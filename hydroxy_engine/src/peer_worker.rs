use std::{
    collections::{HashMap, HashSet, VecDeque},
    net::IpAddr,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use hydroxy_dht::DHTHandle;
use hydroxy_peer_wire_proto::{handshake::Handshake, message::PeerMessage};
use hydroxy_state::TorrentState;
use hydroxy_storage_io::{file_region::FileRegion, piece_io::PieceIO};
use hydroxy_tracker::Peer;
use tokio::{
    net::{TcpStream, tcp::OwnedReadHalf},
    sync::{Mutex, RwLock, broadcast, mpsc},
};
use tracing::{info, warn};

use crate::{
    errors::EngineError,
    fast_ext, pex,
    piece_allocator::{PieceAllocator, PieceStatus},
    piece_assembler::PieceAssembler,
    stats::stats::PeerConnectionState,
    stats::stats_registry::StatsRegistry,
};
use hydroxy_utils::{parse_peer_id, torrent_info::OtherInfos};

const PIECE_STALL_TIMEOUT: Duration = Duration::from_secs(20);
const BASE_PIECE_COOLDOWN: Duration = Duration::from_secs(5);
const MAX_PIECE_COOLDOWN: Duration = Duration::from_secs(300);
const MAX_VERIFICATION_ATTEMPTS: usize = 5;
const IDLE_UNCHOKED_POLL_INTERVAL: Duration = Duration::from_secs(1);
const IDLE_CHOKED_WAIT_INTERVAL: Duration = Duration::from_secs(30);
const REJECTED_PIECE_COOLDOWN: Duration = Duration::from_secs(5);
const MAX_CHOKED_REJECTED_REQUESTS: u32 = 256;
const MAX_SUGGESTED_PIECES: usize = 64;

struct PeerSessionGuard {
    stats_registry: Arc<StatsRegistry>,
    info_hash_sha1: String,
    ip_addr: IpAddr,
    port: u16,
    writer_handle: Option<tokio::task::JoinHandle<()>>,
    piece_allocator: Arc<Mutex<PieceAllocator>>,
    active_piece: Arc<AtomicU32>,
}

impl Drop for PeerSessionGuard {
    fn drop(&mut self) {
        if let Some(handle) = self.writer_handle.take() {
            handle.abort();
        }
        self.stats_registry
            .peer_disconnected(&self.info_hash_sha1, self.ip_addr, self.port);

        let piece_idx = self.active_piece.swap(u32::MAX, Ordering::Relaxed);
        if piece_idx != u32::MAX {
            if let Ok(mut alloc) = self.piece_allocator.try_lock() {
                alloc.release_piece(piece_idx as usize);
            } else {
                let alloc_clone = Arc::clone(&self.piece_allocator);
                tokio::spawn(async move {
                    alloc_clone.lock().await.release_piece(piece_idx as usize);
                });
            }
        }
    }
}

#[allow(dead_code)]
pub struct PeerState {
    pub am_choking: bool,
    pub am_interested: bool,
    pub peer_choking: bool,
    pub peer_interested: bool,
    pub bitfield: Vec<u8>,
    pub received_bitfield_state: bool,
    pub peer_has_all: bool,
}

#[allow(dead_code)]
pub struct PeerWorker {
    pub ip_addr: IpAddr,
    pub port: u16,
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
    pub peer_state: PeerState,
    pub total_pieces: usize,
    pub piece_allocator: Arc<Mutex<PieceAllocator>>,
    pub piece_assembler: PieceAssembler,
    pub total_torrent_size: u64,
    pub piece_length: u32,
    pub piece_hashes: Arc<Vec<u8>>,
    pub file_map: Arc<RwLock<Vec<FileRegion>>>,
    pub torrent_state: Arc<Mutex<TorrentState>>,
    pub pending_requests: u32,
    pub current_piece: Option<u32>,
    pub requested_offset: u32, // next begin offset to request for current piece
    pub stats_registry: Arc<StatsRegistry>,
    pub have_tx: broadcast::Sender<u32>,
    pub have_rx: broadcast::Receiver<u32>,
    pub supports_extensions: bool,
    pub supports_dht: bool,
    pub supports_fast: bool,
    pub dht_handle: Option<DHTHandle>,
    pub is_private: bool,
    pub remote_ut_pex_id: Option<u8>,
    pub connected_peers: Arc<Mutex<HashSet<(IpAddr, u16)>>>,
    pub peer_tx: mpsc::Sender<Vec<Peer>>,
    pub last_sent_pex_peers: HashSet<(IpAddr, u16)>,
    pub active_piece_guard: Arc<AtomicU32>,
    pub last_block_time: Option<Instant>,
    pub failed_pieces: HashMap<u32, (usize, Instant)>,
    pub banned_peers: Arc<Mutex<HashSet<IpAddr>>>,
    pub peer_failed_pieces: Arc<Mutex<HashMap<IpAddr, usize>>>,
    pub am_allowed_fast: HashSet<u32>,
    pub peer_allowed_fast: HashSet<u32>,
    pub peer_suggested: VecDeque<u32>,
    pub sent_requests: HashMap<(u32, u32, u32), u32>,
    pub rejected_pieces: HashMap<u32, Instant>,
    pub choked_rejected_count: u32,
}

impl PeerWorker {
    pub fn calculate_piece_backoff(attempts: usize) -> Duration {
        let exp = (attempts.saturating_sub(1) as u32).min(6);
        let secs = BASE_PIECE_COOLDOWN
            .as_secs()
            .saturating_mul(1 << exp)
            .min(MAX_PIECE_COOLDOWN.as_secs());
        Duration::from_secs(secs)
    }

    pub async fn start(mut self) -> Result<(), EngineError> {
        let socket_addr = std::net::SocketAddr::new(self.ip_addr, self.port);
        let mut tcp_stream =
            tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(socket_addr))
                .await
                .map_err(|_| EngineError::Timeout)??;
        let _ = tcp_stream.set_nodelay(true);

        let handshake = tokio::time::timeout(
            Duration::from_secs(5),
            Handshake::new(self.info_hash, self.peer_id).do_handshake(&mut tcp_stream),
        )
        .await
        .map_err(|_| EngineError::Timeout)??;

        self.supports_extensions = handshake.supports_extensions();
        self.supports_dht = handshake.supports_dht();
        self.supports_fast = handshake.supports_fast();
        self.run_with_stream(tcp_stream, handshake.peer_id).await
    }

    pub async fn start_inbound(
        self,
        tcp_stream: TcpStream,
        remote_peer_id: [u8; 20],
    ) -> Result<(), EngineError> {
        let _ = tcp_stream.set_nodelay(true);
        self.run_with_stream(tcp_stream, remote_peer_id).await
    }

    fn set_current_piece(&mut self, piece: Option<u32>) {
        self.current_piece = piece;
        let raw = piece.unwrap_or(u32::MAX);
        self.active_piece_guard.store(raw, Ordering::Relaxed);
    }

    fn can_download(&self) -> bool {
        !self.peer_state.peer_choking || (self.supports_fast && !self.peer_allowed_fast.is_empty())
    }

    fn can_request_piece(&self, piece_idx: u32) -> bool {
        !self.peer_state.peer_choking
            || (self.supports_fast && self.peer_allowed_fast.contains(&piece_idx))
    }

    fn record_sent_request(&mut self, index: u32, begin: u32, length: u32) {
        *self
            .sent_requests
            .entry((index, begin, length))
            .or_insert(0) += 1;
    }

    fn consume_sent_request(&mut self, index: u32, begin: u32, length: u32) -> bool {
        if let std::collections::hash_map::Entry::Occupied(mut entry) =
            self.sent_requests.entry((index, begin, length))
        {
            let count = entry.get_mut();
            if *count > 1 {
                *count -= 1;
            } else {
                entry.remove();
            }
            true
        } else {
            false
        }
    }

    async fn run_with_stream(
        mut self,
        mut tcp_stream: TcpStream,
        remote_peer_id: [u8; 20],
    ) -> Result<(), EngineError> {
        let info_hash_sha1 = OtherInfos::to_sha1(&self.info_hash);

        let (our_bitfield, has_incomplete, completed_count) = {
            let alloc = self.piece_allocator.lock().await;
            (
                alloc.to_bitfield(self.total_pieces),
                alloc.has_incomplete_pieces(),
                alloc.completed_pieces_count(),
            )
        };

        self.peer_state.am_choking = false;
        if has_incomplete {
            self.peer_state.am_interested = true;
        }
        let supports_extensions = self.supports_extensions;
        let supports_dht = self.supports_dht;
        let supports_fast = self.supports_fast;
        let is_private = self.is_private;
        let dht_handle = self.dht_handle.clone();

        let fast_list: Vec<u32> = if supports_fast {
            fast_ext::generate_allowed_fast_set_for_ip(
                self.ip_addr,
                &self.info_hash,
                self.total_pieces as u32,
                fast_ext::DEFAULT_ALLOWED_FAST_K,
            )
        } else {
            Vec::new()
        };
        self.am_allowed_fast = fast_list.iter().copied().collect();
        let sent_have_all = supports_fast && self.total_pieces > 0 && !has_incomplete;

        tokio::time::timeout(Duration::from_secs(10), async {
            // If remote peer supports BEP 10 Extension Protocol and torrent is not private, send Extended Handshake
            if supports_extensions && !is_private {
                let ext_handshake_payload = pex::build_extended_handshake();
                PeerMessage::Extended {
                    id: pex::EXTENSION_HANDSHAKE_ID,
                    payload: ext_handshake_payload,
                }
                .write(&mut tcp_stream)
                .await?;
            }

            // Send initial piece availability per BEP 0003 / BEP 0006 immediately after handshake
            if supports_fast {
                if sent_have_all {
                    PeerMessage::HaveAll.write(&mut tcp_stream).await?;
                } else if completed_count == 0 {
                    PeerMessage::HaveNone.write(&mut tcp_stream).await?;
                } else if !our_bitfield.is_empty() {
                    PeerMessage::Bitfield(our_bitfield)
                        .write(&mut tcp_stream)
                        .await?;
                }
            } else if !our_bitfield.is_empty() {
                PeerMessage::Bitfield(our_bitfield)
                    .write(&mut tcp_stream)
                    .await?;
            }

            // If remote peer supports BEP 5 DHT and torrent is not private, send Port message
            if supports_dht && !is_private {
                if let Some(ref dht) = dht_handle {
                    if let Ok(std::net::SocketAddr::V4(v4)) = dht.local_addr() {
                        PeerMessage::Port(v4.port()).write(&mut tcp_stream).await?;
                    }
                }
            }

            // BEP 0006: Advertise canonical Allowed Fast piece set to the remote peer
            if supports_fast {
                for &fast_piece in &fast_list {
                    PeerMessage::AllowedFast(fast_piece)
                        .write(&mut tcp_stream)
                        .await?;
                }
            }

            // Unchoke the remote peer immediately per BEP 0003 so they can request data from us
            PeerMessage::Unchoke.write(&mut tcp_stream).await?;

            // Express interest if we have incomplete pieces
            if has_incomplete {
                PeerMessage::Interested.write(&mut tcp_stream).await?;
            }

            Ok::<(), EngineError>(())
        })
        .await
        .map_err(|_| EngineError::Timeout)??;

        let client_name = parse_peer_id(&remote_peer_id);

        self.stats_registry
            .peer_connected(&info_hash_sha1, self.ip_addr, self.port, client_name);

        // Split TCP stream into dedicated read and write halves.
        // This ensures PeerMessage::read is NEVER cancelled or interrupted mid-message by
        // outgoing messages or keepalives, preventing stream desynchronization.
        let (mut read_half, mut write_half) = tcp_stream.into_split();
        let (out_tx, mut out_rx) = mpsc::channel::<PeerMessage>(128);
        let mut have_rx = self.have_rx.resubscribe();

        let writer_handle = tokio::spawn(async move {
            let mut keepalive_interval = tokio::time::interval(Duration::from_secs(60));
            keepalive_interval.tick().await;

            loop {
                tokio::select! {
                    _ = keepalive_interval.tick() => {
                        if tokio::time::timeout(
                            Duration::from_secs(15),
                            PeerMessage::KeepAlive.write(&mut write_half),
                        ).await.map_or(true, |r| r.is_err()) {
                            break;
                        }
                    }
                    Ok(completed_piece) = have_rx.recv() => {
                        // BEP 0006: Never send Have messages after HaveAll has been sent
                        if !sent_have_all {
                            if tokio::time::timeout(
                                Duration::from_secs(15),
                                PeerMessage::Have(completed_piece).write(&mut write_half),
                            ).await.map_or(true, |r| r.is_err()) {
                                break;
                            }
                        }
                    }
                    msg_opt = out_rx.recv() => {
                        match msg_opt {
                            Some(msg) => {
                                if tokio::time::timeout(
                                    Duration::from_secs(15),
                                    msg.write(&mut write_half),
                                ).await.map_or(true, |r| r.is_err()) {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        // RAII guard ensures writer_handle is aborted, peer_disconnected is recorded,
        // and any in-flight piece is released back to the allocator even if this task
        // is aborted (e.g. via JoinSet::shutdown on Pause/Recheck) or panics.
        let _session_guard = PeerSessionGuard {
            stats_registry: Arc::clone(&self.stats_registry),
            info_hash_sha1: info_hash_sha1.clone(),
            ip_addr: self.ip_addr,
            port: self.port,
            writer_handle: Some(writer_handle),
            piece_allocator: Arc::clone(&self.piece_allocator),
            active_piece: Arc::clone(&self.active_piece_guard),
        };

        let result = self
            .run_loop(&mut read_half, &out_tx, &info_hash_sha1)
            .await;

        // Cleanly release any in-progress piece back to the pool on normal return
        if let Some(piece_idx) = self.current_piece {
            self.set_current_piece(None);
            self.last_block_time = None;
            self.piece_allocator
                .lock()
                .await
                .release_piece(piece_idx as usize);
            self.piece_assembler.clear_piece(piece_idx);
            warn!(
                "Worker {}:{} exiting — released piece {} back to pool",
                self.ip_addr, self.port, piece_idx
            );
        }
        self.piece_assembler.clear();

        result
    }

    async fn run_loop(
        &mut self,
        read_half: &mut OwnedReadHalf,
        out_tx: &mpsc::Sender<PeerMessage>,
        info_hash_sha1: &str,
    ) -> Result<(), EngineError> {
        let mut last_pex_sent = Instant::now();
        let pex_period = Duration::from_secs(60);

        loop {
            // 1. Check if an active piece request has stalled (no Piece block received within PIECE_STALL_TIMEOUT).
            // Tracking `last_block_time` separately ensures KeepAlive or PEX messages cannot mask a stalled piece request.
            if let Some(piece_idx) = self.current_piece {
                let stalled = self
                    .last_block_time
                    .map_or(true, |t| t.elapsed() >= PIECE_STALL_TIMEOUT);
                if stalled {
                    warn!(
                        "Worker {}:{} piece {} request stalled ({}s no block data) — releasing piece back to pool",
                        self.ip_addr,
                        self.port,
                        piece_idx,
                        PIECE_STALL_TIMEOUT.as_secs()
                    );
                    self.set_current_piece(None);
                    self.last_block_time = None;
                    self.pending_requests = 0;
                    self.requested_offset = 0;
                    self.piece_assembler.clear_piece(piece_idx);
                    self.piece_allocator
                        .lock()
                        .await
                        .release_piece(piece_idx as usize);
                    let entry = self
                        .failed_pieces
                        .entry(piece_idx)
                        .or_insert((0, Instant::now()));
                    entry.0 += 1;
                    entry.1 = Instant::now();
                    let cooldown = Self::calculate_piece_backoff(entry.0);
                    if entry.0 >= MAX_VERIFICATION_ATTEMPTS {
                        warn!(
                            "Piece {} stalled {} times (threshold {} reached) — applying exponential backoff ({:?})",
                            piece_idx, entry.0, MAX_VERIFICATION_ATTEMPTS, cooldown
                        );
                    }
                }
            }

            // 2. If unchoked (or choked with BEP 0006 AllowedFast pieces) and idle, check if a missing piece is available.
            if self.can_download() && self.current_piece.is_none() {
                self.try_start_downloading(out_tx).await?;
            }

            // 3. Wait for incoming data using `peek` so timeouts NEVER cancel `PeerMessage::read` mid-message
            // and never desynchronize the TCP stream.
            let wait_timeout = if self.current_piece.is_some() {
                let elapsed = self
                    .last_block_time
                    .map(|t| t.elapsed())
                    .unwrap_or(Duration::ZERO);
                PIECE_STALL_TIMEOUT
                    .saturating_sub(elapsed)
                    .max(Duration::from_millis(200))
            } else if self.can_download() {
                IDLE_UNCHOKED_POLL_INTERVAL
            } else {
                IDLE_CHOKED_WAIT_INTERVAL
            };

            let mut peek_buf = [0u8; 1];
            match tokio::time::timeout(wait_timeout, read_half.peek(&mut peek_buf)).await {
                Ok(peek_res) => {
                    peek_res?;
                    let peer_message =
                        tokio::time::timeout(Duration::from_secs(30), PeerMessage::read(read_half))
                            .await
                            .map_err(|_| EngineError::Timeout)??;
                    self.handle_message(peer_message, out_tx, info_hash_sha1)
                        .await?;
                }
                Err(_) => {
                    // Timeout on `peek` consumed 0 bytes from the TCP stream.
                    // The top of the loop will now check piece stall / try_start_downloading.
                }
            }

            if last_pex_sent.elapsed() >= pex_period {
                last_pex_sent = Instant::now();
                if let Some(remote_id) = self.remote_ut_pex_id {
                    self.send_pex_update(remote_id, out_tx, info_hash_sha1)
                        .await;
                }
            }
        }
    }

    async fn handle_message(
        &mut self,
        peer_message: PeerMessage,
        out_tx: &mpsc::Sender<PeerMessage>,
        info_hash_sha1: &str,
    ) -> Result<(), EngineError> {
        match peer_message {
            PeerMessage::KeepAlive => {
                // Per spec, KeepAlive is consumed silently.
            }
            PeerMessage::Choke => {
                self.peer_state.peer_choking = true;
                self.stats_registry.update_peer_state(
                    &info_hash_sha1,
                    self.ip_addr,
                    self.port,
                    PeerConnectionState::Choked,
                );

                // BEP 0006: Choke does NOT interrupt Allowed Fast pieces.
                // Only release the in-progress piece if it cannot be requested while choked.
                if let Some(piece_idx) = self.current_piece {
                    if !self.can_request_piece(piece_idx) {
                        self.set_current_piece(None);
                        self.last_block_time = None;
                        self.piece_allocator
                            .lock()
                            .await
                            .release_piece(piece_idx as usize);
                        self.piece_assembler.clear_piece(piece_idx);
                        self.pending_requests = 0;
                        self.requested_offset = 0;

                        // If we have AllowedFast pieces available, transition immediately
                        if self.can_download() {
                            self.try_start_downloading(out_tx).await?;
                        }
                    }
                }
            }
            PeerMessage::Unchoke => {
                self.peer_state.peer_choking = false;
                self.stats_registry.update_peer_state(
                    &info_hash_sha1,
                    self.ip_addr,
                    self.port,
                    PeerConnectionState::Connected,
                );
                if self.current_piece.is_none() {
                    self.try_start_downloading(out_tx).await?;
                }
            }
            PeerMessage::Interested => {
                self.peer_state.peer_interested = true;
            }
            PeerMessage::NotInterested => {
                self.peer_state.peer_interested = false;
            }
            PeerMessage::Have(index) => {
                // BEP 0006: Receiving Have after HaveAll is a protocol violation -> close connection
                if self.supports_fast && self.peer_state.peer_has_all {
                    return Err(EngineError::Error(format!(
                        "Peer {}:{} sent Have({}) after HaveAll (BEP 0006 violation)",
                        self.ip_addr, self.port, index
                    )));
                }
                self.peer_state.received_bitfield_state = true;

                // Guard against out-of-bounds piece index (prevents memory bomb via bitfield.resize)
                if (index as usize) >= self.total_pieces {
                    warn!(
                        "Peer {}:{} sent out-of-range Have({}) (total_pieces={}) — ignoring",
                        self.ip_addr, self.port, index, self.total_pieces
                    );
                    return Ok(());
                }

                // Update piece allocator availability
                {
                    let mut piece_allocator = self.piece_allocator.lock().await;
                    piece_allocator.register_have(index as usize);
                }

                let byte = index as usize / 8;
                let bit = 7 - (index as usize % 8);
                if byte >= self.peer_state.bitfield.len() {
                    let max_bytes = (self.total_pieces + 7) / 8;
                    self.peer_state
                        .bitfield
                        .resize((byte + 1).min(max_bytes), 0);
                }
                if byte < self.peer_state.bitfield.len() {
                    self.peer_state.bitfield[byte] |= 1 << bit;
                }

                let progress = self.calculate_peer_progress();
                self.stats_registry.update_peer_progress(
                    &info_hash_sha1,
                    self.ip_addr,
                    self.port,
                    progress,
                );

                let we_are_complete = {
                    let alloc = self.piece_allocator.lock().await;
                    alloc.all_complete()
                };
                if we_are_complete && progress >= 100.0 {
                    info!(
                        "Worker {}:{} completed all pieces (seeder) — disconnecting to free connection slot",
                        self.ip_addr, self.port
                    );
                    return Ok(());
                }

                if !self.peer_state.am_interested {
                    let is_incomplete = {
                        let alloc = self.piece_allocator.lock().await;
                        (index as usize) < alloc.pieces.len()
                            && !matches!(alloc.pieces[index as usize], PieceStatus::Complete)
                    };
                    if is_incomplete {
                        self.peer_state.am_interested = true;
                        let _ = out_tx.send(PeerMessage::Interested).await;
                    }
                }

                // If unchoked (or allowed-fast) and idle, start downloading
                if self.can_download() && self.current_piece.is_none() {
                    self.try_start_downloading(out_tx).await?;
                }
            }
            PeerMessage::Bitfield(mut bitfield) => {
                let expected_bytes = (self.total_pieces + 7) / 8;
                if self.supports_fast {
                    if self.peer_state.received_bitfield_state {
                        return Err(EngineError::Error(format!(
                            "Peer {}:{} sent duplicate Bitfield/availability message (BEP 0006 violation)",
                            self.ip_addr, self.port
                        )));
                    }
                    if bitfield.len() != expected_bytes {
                        return Err(EngineError::Error(format!(
                            "Peer {}:{} sent invalid Bitfield length {} (expected {})",
                            self.ip_addr,
                            self.port,
                            bitfield.len(),
                            expected_bytes
                        )));
                    }
                }
                self.peer_state.received_bitfield_state = true;

                if bitfield.len() > expected_bytes {
                    bitfield.truncate(expected_bytes);
                    bitfield.shrink_to_fit();
                }

                {
                    let mut piece_allocator = self.piece_allocator.lock().await;
                    piece_allocator.register_bitfield(&bitfield, self.total_pieces);
                }

                self.peer_state.bitfield = bitfield;

                let progress = self.calculate_peer_progress();
                self.stats_registry.update_peer_progress(
                    &info_hash_sha1,
                    self.ip_addr,
                    self.port,
                    progress,
                );

                let we_are_complete = {
                    let alloc = self.piece_allocator.lock().await;
                    alloc.all_complete()
                };
                if we_are_complete && progress >= 100.0 {
                    info!(
                        "Worker {}:{} is also a seeder (100% complete) — disconnecting to free connection slot",
                        self.ip_addr, self.port
                    );
                    return Ok(());
                }

                let has_useful = {
                    let piece_allocator = self.piece_allocator.lock().await;
                    piece_allocator
                        .peer_has_incomplete_piece(&self.peer_state.bitfield, self.total_pieces)
                };

                if has_useful && !self.peer_state.am_interested {
                    self.peer_state.am_interested = true;
                    let _ = out_tx.send(PeerMessage::Interested).await;
                }

                if self.can_download() && self.current_piece.is_none() {
                    self.try_start_downloading(out_tx).await?;
                }
            }

            PeerMessage::HaveAll => {
                if !self.supports_fast {
                    return Err(EngineError::Error(format!(
                        "Peer {}:{} sent HaveAll without Fast Extension negotiated",
                        self.ip_addr, self.port
                    )));
                }
                if self.peer_state.received_bitfield_state {
                    return Err(EngineError::Error(format!(
                        "Peer {}:{} sent duplicate HaveAll/Bitfield message",
                        self.ip_addr, self.port
                    )));
                }
                self.peer_state.received_bitfield_state = true;
                self.peer_state.peer_has_all = true;

                let bitfield = PieceAllocator::build_full_bitfield(self.total_pieces);
                {
                    let mut piece_allocator = self.piece_allocator.lock().await;
                    piece_allocator.register_bitfield(&bitfield, self.total_pieces);
                }
                self.peer_state.bitfield = bitfield;

                self.stats_registry.update_peer_progress(
                    &info_hash_sha1,
                    self.ip_addr,
                    self.port,
                    100.0,
                );

                let has_incomplete = {
                    let alloc = self.piece_allocator.lock().await;
                    if alloc.all_complete() {
                        info!(
                            "Worker {}:{} is also a seeder (HaveAll) — disconnecting to free connection slot",
                            self.ip_addr, self.port
                        );
                        return Ok(());
                    }
                    alloc.has_incomplete_pieces()
                };

                if has_incomplete && !self.peer_state.am_interested {
                    self.peer_state.am_interested = true;
                    let _ = out_tx.send(PeerMessage::Interested).await;
                }

                if self.can_download() && self.current_piece.is_none() {
                    self.try_start_downloading(out_tx).await?;
                }
            }

            PeerMessage::HaveNone => {
                if !self.supports_fast {
                    return Err(EngineError::Error(format!(
                        "Peer {}:{} sent HaveNone without Fast Extension negotiated",
                        self.ip_addr, self.port
                    )));
                }
                if self.peer_state.received_bitfield_state {
                    return Err(EngineError::Error(format!(
                        "Peer {}:{} sent duplicate HaveNone/Bitfield message",
                        self.ip_addr, self.port
                    )));
                }
                self.peer_state.received_bitfield_state = true;
                self.peer_state.bitfield = vec![0u8; (self.total_pieces + 7) / 8];
                self.stats_registry.update_peer_progress(
                    &info_hash_sha1,
                    self.ip_addr,
                    self.port,
                    0.0,
                );
            }

            PeerMessage::SuggestPiece(index) => {
                if !self.supports_fast {
                    return Err(EngineError::Error(format!(
                        "Peer {}:{} sent SuggestPiece without Fast Extension negotiated",
                        self.ip_addr, self.port
                    )));
                }
                if (index as usize) >= self.total_pieces {
                    warn!(
                        "Peer {}:{} sent out-of-range SuggestPiece({}) (total_pieces={}) — ignoring",
                        self.ip_addr, self.port, index, self.total_pieces
                    );
                    return Ok(());
                }
                if !self.peer_suggested.contains(&index) {
                    if self.peer_suggested.len() >= MAX_SUGGESTED_PIECES {
                        self.peer_suggested.pop_front();
                    }
                    self.peer_suggested.push_back(index);
                }
                if self.can_download() && self.current_piece.is_none() {
                    self.try_start_downloading(out_tx).await?;
                }
            }

            PeerMessage::AllowedFast(index) => {
                if !self.supports_fast {
                    return Err(EngineError::Error(format!(
                        "Peer {}:{} sent AllowedFast without Fast Extension negotiated",
                        self.ip_addr, self.port
                    )));
                }
                if (index as usize) >= self.total_pieces {
                    warn!(
                        "Peer {}:{} sent out-of-range AllowedFast({}) (total_pieces={}) — ignoring",
                        self.ip_addr, self.port, index, self.total_pieces
                    );
                    return Ok(());
                }
                self.peer_allowed_fast.insert(index);
                if self.current_piece.is_none() {
                    self.try_start_downloading(out_tx).await?;
                }
            }

            PeerMessage::RejectRequest {
                index,
                begin,
                length,
            } => {
                if !self.supports_fast {
                    return Err(EngineError::Error(format!(
                        "Peer {}:{} sent RejectRequest without Fast Extension negotiated",
                        self.ip_addr, self.port
                    )));
                }
                if !self.consume_sent_request(index, begin, length) {
                    return Err(EngineError::Error(format!(
                        "Peer {}:{} sent RejectRequest for unrequested block ({}, {}, {})",
                        self.ip_addr, self.port, index, begin, length
                    )));
                }

                if self.current_piece == Some(index) {
                    self.set_current_piece(None);
                    self.last_block_time = None;
                    self.pending_requests = 0;
                    self.requested_offset = 0;
                    self.piece_assembler.clear_piece(index);
                    self.piece_allocator
                        .lock()
                        .await
                        .release_piece(index as usize);
                    self.rejected_pieces.insert(index, Instant::now());

                    if self.can_download() {
                        self.try_start_downloading(out_tx).await?;
                    }
                }
            }

            PeerMessage::Piece {
                index,
                begin,
                block,
            } => {
                // BEP 0006: Receiving a Piece that was never requested MUST close the connection
                let was_requested = self.consume_sent_request(index, begin, block.len() as u32);
                if self.supports_fast && !was_requested {
                    return Err(EngineError::Error(format!(
                        "Peer {}:{} sent unrequested Piece block ({}, {}, {}) (BEP 0006 violation)",
                        self.ip_addr,
                        self.port,
                        index,
                        begin,
                        block.len()
                    )));
                }

                // Only accept blocks for the currently assigned piece within valid torrent bounds
                if self.current_piece != Some(index) || (index as usize) >= self.total_pieces {
                    return Ok(());
                }

                self.last_block_time = Some(Instant::now());
                self.pending_requests = self.pending_requests.saturating_sub(1);
                self.stats_registry.record_download(
                    &info_hash_sha1,
                    self.ip_addr,
                    self.port,
                    block.len() as u64,
                );

                let actual_len = PieceAssembler::actual_piece_length(
                    index,
                    self.total_pieces as u32,
                    self.piece_length,
                    self.total_torrent_size,
                );

                if let Some(assembled) = self
                    .piece_assembler
                    .add_block(index, begin, block, actual_len)
                {
                    // SHA1 verify on blocking pool so multi-MB hashing never stalls core network threads
                    let hash_start = index as usize * 20;
                    if hash_start + 20 > self.piece_hashes.len() {
                        return Ok(());
                    }
                    let mut expected_hash = [0u8; 20];
                    expected_hash.copy_from_slice(&self.piece_hashes[hash_start..hash_start + 20]);

                    let (is_valid, assembled) = tokio::task::spawn_blocking(move || {
                        let valid = PieceAssembler::verify_piece(&assembled, &expected_hash);
                        (valid, assembled)
                    })
                    .await
                    .map_err(|e| EngineError::Error(format!("SHA1 task join error: {}", e)))?;

                    if !is_valid {
                        // 1. Record failed piece attempt and backoff
                        let entry = self
                            .failed_pieces
                            .entry(index)
                            .or_insert((0, Instant::now()));
                        entry.0 += 1;
                        entry.1 = Instant::now();
                        let attempts = entry.0;
                        let cooldown = Self::calculate_piece_backoff(attempts);

                        // 2. Track failed piece counts per ip_addr
                        let total_peer_failures = {
                            let mut failures = self.peer_failed_pieces.lock().await;
                            let count = failures.entry(self.ip_addr).or_insert(0);
                            *count += 1;
                            *count
                        };

                        // 3. Ban peer from reconnecting for this session
                        {
                            let mut banned = self.banned_peers.lock().await;
                            banned.insert(self.ip_addr);
                        }

                        warn!(
                            "Piece {} failed SHA1 verification (attempt {}/{}, cooldown {:?}) from peer {}:{}. Peer penalized (total failures: {}), banned from session, and disconnected.",
                            index,
                            attempts,
                            MAX_VERIFICATION_ATTEMPTS,
                            cooldown,
                            self.ip_addr,
                            self.port,
                            total_peer_failures
                        );

                        // 4. Clean up state and release piece back to allocator
                        self.set_current_piece(None);
                        self.last_block_time = None;
                        self.pending_requests = 0;
                        self.requested_offset = 0;
                        self.piece_assembler.clear_piece(index);
                        self.piece_allocator
                            .lock()
                            .await
                            .release_piece(index as usize);

                        // 5. Terminate worker to immediately disconnect this peer
                        return Err(EngineError::CorruptData(format!(
                            "Peer {}:{} disconnected and banned: SHA1 verification failed on piece {}",
                            self.ip_addr, self.port, index
                        )));
                    }

                    self.failed_pieces.remove(&index);

                    // write to disk without cloning FileRegion vector
                    {
                        let map = self.file_map.read().await;
                        PieceIO::write_piece_to_disk(&*map, index, self.piece_length, &assembled)
                            .await?;
                    }
                    drop(assembled);

                    info!("Piece {} written to disk", index);
                    self.stats_registry.piece_completed(
                        &info_hash_sha1,
                        self.ip_addr,
                        self.port,
                        index as usize,
                    );

                    // update hot_state serialized under lock to prevent concurrent state task pileups
                    {
                        let snap = self.stats_registry.snapshot_torrent(&info_hash_sha1);
                        let mut state = self.torrent_state.lock().await;
                        state.mark_completed_piece(index as usize);
                        if let Some(s) = snap {
                            state.total_downloaded = state.total_downloaded.max(s.downloaded_bytes);
                            state.total_uploaded = state.total_uploaded.max(s.uploaded_bytes);
                        }
                        let snapshot = state.clone();
                        if let Err(e) = TorrentState::save_state(snapshot).await {
                            warn!("State save failed after piece {}: {}", index, e);
                        }
                    }

                    // mark complete in allocator
                    self.set_current_piece(None);
                    self.last_block_time = None;
                    self.piece_allocator
                        .lock()
                        .await
                        .mark_complete(index as usize);

                    // Broadcast Have message to all connected peer workers
                    let _ = self.have_tx.send(index);

                    // move to next piece
                    self.pending_requests = 0;
                    self.requested_offset = 0;
                    self.try_start_downloading(out_tx).await?;
                } else {
                    // piece incomplete — keep pipeline full
                    if let Some(piece_idx) = self.current_piece {
                        if self.can_request_piece(piece_idx) {
                            self.fill_pipeline(piece_idx, out_tx).await?;
                        }
                    }
                }
            }

            PeerMessage::Request {
                index,
                begin,
                length,
            } => {
                // Reject zero-length, oversized (> 128 KB per BEP 0003), or out-of-bounds piece requests
                if length == 0 || length > 128 * 1024 || (index as usize) >= self.total_pieces {
                    if self.supports_fast {
                        let _ = out_tx
                            .send(PeerMessage::RejectRequest {
                                index,
                                begin,
                                length,
                            })
                            .await;
                    }
                    return Ok(());
                }

                // BEP 0006: Serve requests if unchoked OR if the piece is in our Allowed Fast set for this peer
                let may_serve = !self.peer_state.am_choking
                    || (self.supports_fast && self.am_allowed_fast.contains(&index));

                if !may_serve {
                    if self.supports_fast {
                        self.choked_rejected_count = self.choked_rejected_count.saturating_add(1);
                        if self.choked_rejected_count > MAX_CHOKED_REJECTED_REQUESTS {
                            return Err(EngineError::Error(format!(
                                "Peer {}:{} sent excessive requests ({}) while choked",
                                self.ip_addr, self.port, self.choked_rejected_count
                            )));
                        }
                        let _ = out_tx
                            .send(PeerMessage::RejectRequest {
                                index,
                                begin,
                                length,
                            })
                            .await;
                    }
                    return Ok(());
                }

                let is_complete = {
                    let allocator = self.piece_allocator.lock().await;
                    (index as usize) < allocator.pieces.len()
                        && matches!(allocator.pieces[index as usize], PieceStatus::Complete)
                };

                if is_complete {
                    let block_opt = {
                        let map = self.file_map.read().await;
                        PieceIO::read_block_from_disk(
                            &*map,
                            index,
                            begin,
                            length,
                            self.piece_length,
                            self.total_torrent_size,
                        )
                        .await
                    };
                    if let Ok(Some(block)) = block_opt {
                        let block_len = block.len() as u64;
                        if out_tx
                            .send(PeerMessage::Piece {
                                index,
                                begin,
                                block,
                            })
                            .await
                            .is_ok()
                        {
                            self.stats_registry.record_upload(
                                &info_hash_sha1,
                                self.ip_addr,
                                self.port,
                                block_len,
                            );
                        }
                    } else if self.supports_fast {
                        let _ = out_tx
                            .send(PeerMessage::RejectRequest {
                                index,
                                begin,
                                length,
                            })
                            .await;
                    }
                } else if self.supports_fast {
                    let _ = out_tx
                        .send(PeerMessage::RejectRequest {
                            index,
                            begin,
                            length,
                        })
                        .await;
                }
            }

            PeerMessage::Cancel {
                index,
                begin,
                length,
            } => {
                let _ = (index, begin, length);
            }

            PeerMessage::Port(port) => {
                if !self.is_private && port != 0 {
                    if let Some(ref dht) = self.dht_handle {
                        if let IpAddr::V4(ipv4) = self.ip_addr {
                            let target = std::net::SocketAddrV4::new(ipv4, port);
                            let dht_clone = dht.clone();
                            tokio::spawn(async move {
                                let _ = dht_clone.ping(target).await;
                            });
                        }
                    }
                }
            }

            PeerMessage::Extended { id, payload } => {
                if self.is_private {
                    return Ok(());
                }
                if id == pex::EXTENSION_HANDSHAKE_ID {
                    if let Some(client_ver) = pex::parse_remote_client_version(&payload) {
                        self.stats_registry.update_peer_client(
                            info_hash_sha1,
                            self.ip_addr,
                            self.port,
                            client_ver,
                        );
                    }
                    if let Some(remote_id) = pex::parse_remote_ut_pex_id(&payload) {
                        info!(
                            "PEX: Negotiated ut_pex with {}:{} (remote extended ID: {})",
                            self.ip_addr, self.port, remote_id
                        );
                        let was_none = self.remote_ut_pex_id.is_none();
                        self.remote_ut_pex_id = Some(remote_id);
                        if was_none {
                            // Send initial PEX message with currently connected peers
                            self.send_pex_update(remote_id, out_tx, info_hash_sha1)
                                .await;
                        }
                    }
                } else if Some(id) == self.remote_ut_pex_id || id == pex::UT_PEX_EXT_ID {
                    let discovered_peers = pex::parse_pex_added_peers(&payload);
                    if !discovered_peers.is_empty() {
                        info!(
                            "PEX: Discovered {} peer(s) from {}:{}",
                            discovered_peers.len(),
                            self.ip_addr,
                            self.port
                        );
                        let _ = self.peer_tx.send(discovered_peers).await;
                    }
                }
            }
        }

        Ok(())
    }

    /// Send a PEX update message to the remote peer advertising handshaked connected peers (IPv4 + IPv6).
    async fn send_pex_update(
        &mut self,
        remote_pex_id: u8,
        out_tx: &mpsc::Sender<PeerMessage>,
        info_hash_sha1: &str,
    ) {
        if self.is_private {
            return;
        }
        let current_peers: Vec<(IpAddr, u16)> =
            self.stats_registry.active_peer_endpoints(info_hash_sha1);

        let added: Vec<(IpAddr, u16)> = current_peers
            .iter()
            .filter(|&&(ip, port)| {
                if ip == self.ip_addr && port == self.port {
                    return false; // Don't advertise peer to itself
                }
                !self.last_sent_pex_peers.contains(&(ip, port))
            })
            .take(50)
            .copied()
            .collect();

        let current_set: HashSet<(IpAddr, u16)> = current_peers.into_iter().collect();
        let dropped: Vec<(IpAddr, u16)> = self
            .last_sent_pex_peers
            .iter()
            .filter(|&peer| !current_set.contains(peer))
            .take(50)
            .copied()
            .collect();

        if !added.is_empty() || !dropped.is_empty() {
            for &peer in &added {
                self.last_sent_pex_peers.insert(peer);
            }
            for &peer in &dropped {
                self.last_sent_pex_peers.remove(&peer);
            }

            let pex_payload = pex::build_pex_message(&added, &dropped);
            let _ = out_tx
                .send(PeerMessage::Extended {
                    id: remote_pex_id,
                    payload: pex_payload,
                })
                .await;
        }
    }

    /// Attempt to allocate a new piece and begin downloading it.
    async fn try_start_downloading(
        &mut self,
        out_tx: &mpsc::Sender<PeerMessage>,
    ) -> Result<(), EngineError> {
        if !self.can_download() || self.current_piece.is_some() {
            return Ok(());
        }

        let mut exclude_pieces = HashSet::new();
        for (&piece_idx, &(attempts, ts)) in &self.failed_pieces {
            let cooldown = Self::calculate_piece_backoff(attempts);
            if ts.elapsed() < cooldown {
                exclude_pieces.insert(piece_idx as usize);
            }
        }
        self.rejected_pieces
            .retain(|_, ts| ts.elapsed() < REJECTED_PIECE_COOLDOWN);
        for &piece_idx in self.rejected_pieces.keys() {
            exclude_pieces.insert(piece_idx as usize);
        }

        let piece_idx = {
            let mut alloc = self.piece_allocator.lock().await;
            let mut picked: Option<usize> = None;

            // BEP 0006: Prefer suggested pieces from this peer (when not in strict sequential mode)
            if !alloc.sequential && !self.peer_suggested.is_empty() {
                self.peer_suggested.retain(|&s| {
                    let idx = s as usize;
                    idx < alloc.pieces.len() && !matches!(alloc.pieces[idx], PieceStatus::Complete)
                });
                for &s in &self.peer_suggested {
                    let idx = s as usize;
                    if exclude_pieces.contains(&idx) || !self.can_request_piece(s) {
                        continue;
                    }
                    if alloc.try_allocate_specific(
                        idx,
                        &self.peer_state.bitfield,
                        &self.ip_addr,
                        self.total_pieces,
                    ) {
                        picked = Some(idx);
                        break;
                    }
                }
                if let Some(chosen) = picked {
                    self.peer_suggested.retain(|&s| s as usize != chosen);
                }
            }

            if picked.is_none() {
                if !self.peer_state.peer_choking {
                    picked = alloc.allocate_piece_excluding(
                        &self.peer_state.bitfield,
                        &self.ip_addr,
                        self.total_pieces,
                        &exclude_pieces,
                    );
                } else if self.supports_fast && !self.peer_allowed_fast.is_empty() {
                    picked = alloc.allocate_from_allowed(
                        &self.peer_state.bitfield,
                        &self.ip_addr,
                        self.total_pieces,
                        &self.peer_allowed_fast,
                        &exclude_pieces,
                    );
                }
            }

            picked
        };

        if let Some(piece_idx) = piece_idx {
            self.set_current_piece(Some(piece_idx as u32));
            self.last_block_time = Some(Instant::now());
            self.pending_requests = 0;
            self.requested_offset = 0;
            self.fill_pipeline(piece_idx as u32, out_tx).await?;
        }

        Ok(())
    }

    async fn fill_pipeline(
        &mut self,
        piece_idx: u32,
        out_tx: &mpsc::Sender<PeerMessage>,
    ) -> Result<(), EngineError> {
        let actual_len = PieceAssembler::actual_piece_length(
            piece_idx,
            self.total_pieces as u32,
            self.piece_length,
            self.total_torrent_size,
        );

        let block_size: u32 = 16_384;
        let max_pending: u32 = 32; // 32 * 16KB = 512KB in-flight pipeline depth

        while self.pending_requests < max_pending && self.requested_offset < actual_len {
            let this_block_len = block_size.min(actual_len - self.requested_offset);
            let begin = self.requested_offset;

            if out_tx
                .send(PeerMessage::Request {
                    index: piece_idx,
                    begin,
                    length: this_block_len,
                })
                .await
                .is_err()
            {
                break;
            }

            self.record_sent_request(piece_idx, begin, this_block_len);
            self.requested_offset += this_block_len;
            self.pending_requests += 1;
        }

        Ok(())
    }

    fn calculate_peer_progress(&self) -> f64 {
        if self.total_pieces == 0 {
            return 0.0;
        }
        let count: usize = self
            .peer_state
            .bitfield
            .iter()
            .map(|b| b.count_ones() as usize)
            .sum::<usize>()
            .min(self.total_pieces);
        (count as f64 / self.total_pieces as f64) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_calculate_piece_backoff_exponential() {
        assert_eq!(
            PeerWorker::calculate_piece_backoff(0),
            Duration::from_secs(5)
        );
        assert_eq!(
            PeerWorker::calculate_piece_backoff(1),
            Duration::from_secs(5)
        );
        assert_eq!(
            PeerWorker::calculate_piece_backoff(2),
            Duration::from_secs(10)
        );
        assert_eq!(
            PeerWorker::calculate_piece_backoff(3),
            Duration::from_secs(20)
        );
        assert_eq!(
            PeerWorker::calculate_piece_backoff(4),
            Duration::from_secs(40)
        );
        assert_eq!(
            PeerWorker::calculate_piece_backoff(5),
            Duration::from_secs(80)
        );
        assert_eq!(
            PeerWorker::calculate_piece_backoff(6),
            Duration::from_secs(160)
        );
        assert_eq!(
            PeerWorker::calculate_piece_backoff(7),
            Duration::from_secs(300)
        ); // capped
        assert_eq!(
            PeerWorker::calculate_piece_backoff(10),
            Duration::from_secs(300)
        ); // capped
    }

    fn make_test_worker(supports_fast: bool, total_pieces: usize) -> PeerWorker {
        let (have_tx, have_rx) = broadcast::channel(16);
        let (peer_tx, _peer_rx) = mpsc::channel(16);
        let state = TorrentState {
            info_hash_v1: "aa".repeat(20),
            version: 1,
            added_time: 0,
            total_downloaded: 0,
            total_uploaded: 0,
            paused: 0,
            sequential: 0,
            pieces: vec![0u8; (total_pieces + 7) / 8],
            trackers: Vec::new(),
        };
        PeerWorker {
            ip_addr: IpAddr::V4(Ipv4Addr::new(80, 4, 4, 200)),
            port: 6881,
            info_hash: [0xAA; 20],
            peer_id: [0x11; 20],
            peer_state: PeerState {
                am_choking: false,
                am_interested: false,
                peer_choking: true,
                peer_interested: false,
                bitfield: vec![0u8; (total_pieces + 7) / 8],
                received_bitfield_state: false,
                peer_has_all: false,
            },
            total_pieces,
            piece_allocator: Arc::new(Mutex::new(PieceAllocator::new(total_pieces))),
            piece_assembler: PieceAssembler::new(),
            total_torrent_size: (total_pieces as u64) * 16_384,
            piece_length: 16_384,
            piece_hashes: Arc::new(vec![0u8; total_pieces * 20]),
            file_map: Arc::new(RwLock::new(Vec::new())),
            torrent_state: Arc::new(Mutex::new(state)),
            pending_requests: 0,
            current_piece: None,
            requested_offset: 0,
            stats_registry: Arc::new(StatsRegistry::new()),
            have_tx,
            have_rx,
            supports_extensions: false,
            supports_dht: false,
            supports_fast,
            dht_handle: None,
            is_private: false,
            remote_ut_pex_id: None,
            connected_peers: Arc::new(Mutex::new(HashSet::new())),
            peer_tx,
            last_sent_pex_peers: HashSet::new(),
            active_piece_guard: Arc::new(AtomicU32::new(u32::MAX)),
            last_block_time: None,
            failed_pieces: HashMap::new(),
            banned_peers: Arc::new(Mutex::new(HashSet::new())),
            peer_failed_pieces: Arc::new(Mutex::new(HashMap::new())),
            am_allowed_fast: HashSet::new(),
            peer_allowed_fast: HashSet::new(),
            peer_suggested: VecDeque::new(),
            sent_requests: HashMap::new(),
            rejected_pieces: HashMap::new(),
            choked_rejected_count: 0,
        }
    }

    #[tokio::test]
    async fn test_bep0006_have_all_allowed_fast_while_choked_and_reject() {
        let mut worker = make_test_worker(true, 10);
        let (out_tx, mut out_rx) = mpsc::channel(64);
        let ih_hex = "aa".repeat(20);

        // 1. Peer sends HaveAll while choking us -> sends Interested, still choked so no Request yet
        worker
            .handle_message(PeerMessage::HaveAll, &out_tx, &ih_hex)
            .await
            .unwrap();
        assert!(worker.peer_state.peer_has_all);
        assert!(worker.peer_state.am_interested);
        assert_eq!(out_rx.try_recv().unwrap(), PeerMessage::Interested);
        assert!(worker.current_piece.is_none());

        // 2. Peer sends AllowedFast(4) -> even though peer_choking == true, we immediately request piece 4!
        worker
            .handle_message(PeerMessage::AllowedFast(4), &out_tx, &ih_hex)
            .await
            .unwrap();
        assert_eq!(worker.current_piece, Some(4));
        assert_eq!(
            out_rx.try_recv().unwrap(),
            PeerMessage::Request {
                index: 4,
                begin: 0,
                length: 16_384,
            }
        );

        // 3. Receiving Choke does NOT interrupt AllowedFast piece 4!
        worker
            .handle_message(PeerMessage::Choke, &out_tx, &ih_hex)
            .await
            .unwrap();
        assert_eq!(worker.current_piece, Some(4));

        // 4. Peer rejects our request -> releases piece 4 back to allocator cleanly
        worker
            .handle_message(
                PeerMessage::RejectRequest {
                    index: 4,
                    begin: 0,
                    length: 16_384,
                },
                &out_tx,
                &ih_hex,
            )
            .await
            .unwrap();
        assert!(worker.current_piece.is_none());
        assert_eq!(
            worker.piece_allocator.lock().await.pieces[4],
            PieceStatus::Missing
        );

        // 5. Duplicate/unrequested RejectRequest MUST close connection per BEP 0006
        assert!(
            worker
                .handle_message(
                    PeerMessage::RejectRequest {
                        index: 4,
                        begin: 0,
                        length: 16_384,
                    },
                    &out_tx,
                    &ih_hex,
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_bep0006_suggest_piece_and_protocol_violations() {
        let mut worker = make_test_worker(true, 8);
        let (out_tx, mut out_rx) = mpsc::channel(64);
        let ih_hex = "aa".repeat(20);

        // Peer sends HaveAll + SuggestPiece(5) + Unchoke -> prefers suggested piece 5 over rarest index 0
        worker
            .handle_message(PeerMessage::HaveAll, &out_tx, &ih_hex)
            .await
            .unwrap();
        let _ = out_rx.try_recv(); // Interested
        worker
            .handle_message(PeerMessage::SuggestPiece(5), &out_tx, &ih_hex)
            .await
            .unwrap();
        worker
            .handle_message(PeerMessage::Unchoke, &out_tx, &ih_hex)
            .await
            .unwrap();
        assert_eq!(worker.current_piece, Some(5));
        assert_eq!(
            out_rx.try_recv().unwrap(),
            PeerMessage::Request {
                index: 5,
                begin: 0,
                length: 16_384,
            }
        );

        // Receiving Have after HaveAll MUST close connection per BEP 0006
        assert!(
            worker
                .handle_message(PeerMessage::Have(2), &out_tx, &ih_hex)
                .await
                .is_err()
        );

        // Receiving unrequested Piece MUST close connection per BEP 0006
        assert!(
            worker
                .handle_message(
                    PeerMessage::Piece {
                        index: 1,
                        begin: 0,
                        block: vec![0u8; 16_384],
                    },
                    &out_tx,
                    &ih_hex,
                )
                .await
                .is_err()
        );

        // Choked incoming Request for non-AllowedFast piece sends RejectRequest under BEP 0006
        worker.peer_state.am_choking = true;
        worker
            .handle_message(
                PeerMessage::Request {
                    index: 2,
                    begin: 0,
                    length: 16_384,
                },
                &out_tx,
                &ih_hex,
            )
            .await
            .unwrap();
        assert_eq!(
            out_rx.try_recv().unwrap(),
            PeerMessage::RejectRequest {
                index: 2,
                begin: 0,
                length: 16_384,
            }
        );

        // Receiving any BEP 0006 message when Fast Extension was NOT negotiated MUST close connection
        let mut legacy_worker = make_test_worker(false, 8);
        assert!(
            legacy_worker
                .handle_message(PeerMessage::HaveNone, &out_tx, &ih_hex)
                .await
                .is_err()
        );
        assert!(
            legacy_worker
                .handle_message(PeerMessage::AllowedFast(0), &out_tx, &ih_hex)
                .await
                .is_err()
        );
    }
}
