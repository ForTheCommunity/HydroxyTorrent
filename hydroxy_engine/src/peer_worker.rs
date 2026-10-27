
use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
    time::Duration,
};

use hydroxy_peer_wire_proto::{handshake::Handshake, message::PeerMessage};
use hydroxy_state::TorrentState;
use hydroxy_storage_io::{file_region::FileRegion, piece_io::PieceIO};
use hydroxy_tracker::Peer;
use tokio::{
    net::{tcp::OwnedReadHalf, TcpStream},
    sync::{broadcast, mpsc, Mutex},
};
use tracing::{info, warn};

use crate::{
    errors::EngineError,
    pex,
    piece_allocator::{PieceAllocator, PieceStatus},
    piece_assembler::PieceAssembler,
    stats::stats::PeerConnectionState,
    stats::stats_registry::StatsRegistry,
};
use hydroxy_utils::{parse_peer_id, torrent_info::OtherInfos};

#[allow(dead_code)]
pub struct PeerState {
    pub am_choking: bool,
    pub am_interested: bool,
    pub peer_choking: bool,
    pub peer_interested: bool,
    pub bitfield: Vec<u8>,
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
    pub file_map: Arc<Vec<FileRegion>>,
    pub torrent_state: Arc<Mutex<TorrentState>>,
    pub pending_requests: u32,
    pub current_piece: Option<u32>,
    pub requested_offset: u32, // next begin offset to request for current piece
    pub stats_registry: Arc<StatsRegistry>,
    pub have_tx: broadcast::Sender<u32>,
    pub have_rx: broadcast::Receiver<u32>,
    pub supports_extensions: bool,
    pub remote_ut_pex_id: Option<u8>,
    pub connected_peers: Arc<Mutex<HashSet<(Ipv4Addr, u16)>>>,
    pub peer_tx: mpsc::Sender<Vec<Peer>>,
    pub last_sent_pex_peers: HashSet<(Ipv4Addr, u16)>,
}

impl PeerWorker {
    pub async fn start(mut self) -> Result<(), EngineError> {
        let mut tcp_stream = TcpStream::connect(format!("{}:{}", self.ip_addr, self.port)).await?;

        let handshake = Handshake::new(self.info_hash, self.peer_id)
            .do_handshake(&mut tcp_stream)
            .await?;

        self.supports_extensions = handshake.supports_extensions();
        self.run_with_stream(tcp_stream, handshake.peer_id).await
    }

    pub async fn start_inbound(
        self,
        tcp_stream: TcpStream,
        remote_peer_id: [u8; 20],
    ) -> Result<(), EngineError> {
        self.run_with_stream(tcp_stream, remote_peer_id).await
    }

    async fn run_with_stream(
        mut self,
        mut tcp_stream: TcpStream,
        remote_peer_id: [u8; 20],
    ) -> Result<(), EngineError> {
        let info_hash_sha1 = OtherInfos::to_sha1(&self.info_hash);

        // If remote peer supports BEP 10 Extension Protocol, send Extended Handshake
        if self.supports_extensions {
            let ext_handshake_payload = pex::build_extended_handshake();
            PeerMessage::Extended {
                id: pex::EXTENSION_HANDSHAKE_ID,
                payload: ext_handshake_payload,
            }
            .write(&mut tcp_stream)
            .await?;
        }

        // Send local bitfield immediately after handshake per BEP 0003
        let our_bitfield = self
            .piece_allocator
            .lock()
            .await
            .to_bitfield(self.total_pieces);
        if !our_bitfield.is_empty() {
            PeerMessage::Bitfield(our_bitfield).write(&mut tcp_stream).await?;
        }

        // Unchoke the remote peer immediately per BEP 0003 so they can request data from us
        self.peer_state.am_choking = false;
        PeerMessage::Unchoke.write(&mut tcp_stream).await?;

        // Express interest if we have missing pieces
        let has_missing = {
            let alloc = self.piece_allocator.lock().await;
            alloc.pieces.iter().any(|p| matches!(p, PieceStatus::Missing))
        };
        if has_missing {
            self.peer_state.am_interested = true;
            PeerMessage::Interested.write(&mut tcp_stream).await?;
        }

        let client_name = parse_peer_id(&remote_peer_id);

        self.stats_registry.peer_connected(
            &info_hash_sha1,
            self.ip_addr,
            self.port,
            client_name,
        );

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
                        if PeerMessage::KeepAlive.write(&mut write_half).await.is_err() {
                            break;
                        }
                    }
                    Ok(completed_piece) = have_rx.recv() => {
                        if PeerMessage::Have(completed_piece).write(&mut write_half).await.is_err() {
                            break;
                        }
                    }
                    msg_opt = out_rx.recv() => {
                        match msg_opt {
                            Some(msg) => {
                                if msg.write(&mut write_half).await.is_err() {
                                    break;
                                }
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        let result = self.run_loop(&mut read_half, &out_tx).await;
        writer_handle.abort();

        self.stats_registry.peer_disconnected(&info_hash_sha1, self.ip_addr, self.port);

        // RAII cleanup: release any in-progress piece back to the pool
        // so other workers can claim it. This runs on any exit path
        // (timeout, error, disconnect, etc.)
        if let Some(piece_idx) = self.current_piece.take() {
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
    ) -> Result<(), EngineError> {
        let mut pex_interval = tokio::time::interval(Duration::from_secs(60));
        pex_interval.tick().await; // skip initial tick

        loop {
            let read_timeout = if self.current_piece.is_some() {
                Duration::from_secs(45)
            } else {
                Duration::from_secs(120)
            };

            tokio::select! {
                read_result = tokio::time::timeout(read_timeout, PeerMessage::read(read_half)) => {
                    match read_result {
                        Ok(msg_result) => {
                            let peer_message = msg_result?;
                            self.handle_message(peer_message, out_tx).await?;
                        }
                        Err(_) => {
                            if let Some(piece_idx) = self.current_piece.take() {
                                warn!(
                                    "Worker {}:{} piece {} request stalled (45s no data) — releasing piece back to pool",
                                    self.ip_addr, self.port, piece_idx
                                );
                                self.piece_allocator
                                    .lock()
                                    .await
                                    .release_piece(piece_idx as usize);
                                self.piece_assembler.clear_piece(piece_idx);
                                self.pending_requests = 0;
                                self.requested_offset = 0;
                                self.try_start_downloading(out_tx).await?;
                            } else {
                                let _ = out_tx.send(PeerMessage::KeepAlive).await;
                            }
                        }
                    }
                }
                _ = pex_interval.tick() => {
                    if let Some(remote_id) = self.remote_ut_pex_id {
                        self.send_pex_update(remote_id, out_tx).await;
                    }
                }
            }
        }
    }

    async fn handle_message(
        &mut self,
        peer_message: PeerMessage,
        out_tx: &mpsc::Sender<PeerMessage>,
    ) -> Result<(), EngineError> {
        let info_hash_sha1 = OtherInfos::to_sha1(&self.info_hash);
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

                // release the in-progress piece so another worker can claim it
                if let Some(piece_idx) = self.current_piece.take() {
                    self.piece_allocator
                        .lock()
                        .await
                        .release_piece(piece_idx as usize);
                    self.piece_assembler.clear_piece(piece_idx);
                    self.pending_requests = 0;
                    self.requested_offset = 0;
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
                self.try_start_downloading(out_tx).await?;
            }
            PeerMessage::Interested => {
                self.peer_state.peer_interested = true;
            }
            PeerMessage::NotInterested => {
                self.peer_state.peer_interested = false;
            }
            PeerMessage::Have(index) => {
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
                    self.peer_state.bitfield.resize((byte + 1).min(max_bytes), 0);
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
                    let is_missing = {
                        let alloc = self.piece_allocator.lock().await;
                        (index as usize) < alloc.pieces.len()
                            && matches!(alloc.pieces[index as usize], PieceStatus::Missing)
                    };
                    if is_missing {
                        self.peer_state.am_interested = true;
                        let _ = out_tx.send(PeerMessage::Interested).await;
                    }
                }

                // If unchoked and idle, start downloading
                if !self.peer_state.peer_choking && self.current_piece.is_none() {
                    self.try_start_downloading(out_tx).await?;
                }
            }
            PeerMessage::Bitfield(mut bitfield) => {
                let expected_bytes = (self.total_pieces + 7) / 8;
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
                    (0..self.total_pieces).any(|i| {
                        PieceAllocator::check_piece(&self.peer_state.bitfield, i)
                            && matches!(piece_allocator.pieces[i], PieceStatus::Missing)
                    })
                };

                if has_useful {
                    self.peer_state.am_interested = true;
                    let _ = out_tx.send(PeerMessage::Interested).await;
                }

                if !self.peer_state.peer_choking && self.current_piece.is_none() {
                    self.try_start_downloading(out_tx).await?;
                }
            }

            PeerMessage::Piece {
                index,
                begin,
                block,
            } => {
                // Only accept blocks for the currently assigned piece within valid torrent bounds
                if self.current_piece != Some(index) || (index as usize) >= self.total_pieces {
                    return Ok(());
                }

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
                    // SHA1 verify
                    let hash_start = index as usize * 20;
                    if hash_start + 20 > self.piece_hashes.len() {
                        return Ok(());
                    }
                    let expected_hash = &self.piece_hashes[hash_start..hash_start + 20];

                    if !PieceAssembler::verify_piece(&assembled, expected_hash) {
                        warn!("Piece {} failed SHA1 — releasing back to pool", index);
                        self.piece_allocator
                            .lock()
                            .await
                            .release_piece(index as usize);
                        self.piece_assembler.clear_piece(index);
                        self.current_piece = None;
                        self.pending_requests = 0;
                        self.requested_offset = 0;
                        self.try_start_downloading(out_tx).await?;
                        return Ok(());
                    }

                    // write to disk without cloning FileRegion vector
                    PieceIO::write_piece_to_disk(
                        &self.file_map,
                        index,
                        self.piece_length,
                        &assembled,
                    )
                    .await?;
                    drop(assembled);

                    info!("Piece {} written to disk", index);
                    self.stats_registry.piece_completed(&info_hash_sha1, self.ip_addr, self.port, index as usize);

                    // update fast-resume state serialized under lock to prevent concurrent state task pileups
                    {
                        let mut state = self.torrent_state.lock().await;
                        state.mark_completed_piece(index as usize);
                        let snapshot = state.clone();
                        if let Err(e) = TorrentState::save_state(snapshot).await {
                            warn!("State save failed after piece {}: {}", index, e);
                        }
                    }

                    // mark complete in allocator
                    self.piece_allocator
                        .lock()
                        .await
                        .mark_complete(index as usize);

                    // Broadcast Have message to all connected peer workers
                    let _ = self.have_tx.send(index);

                    // move to next piece
                    self.current_piece = None;
                    self.pending_requests = 0;
                    self.requested_offset = 0;
                    self.try_start_downloading(out_tx).await?;
                } else {
                    // piece incomplete — keep pipeline full
                    if let Some(piece_idx) = self.current_piece {
                        if !self.peer_state.peer_choking {
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
                // Reject zero-length or oversized block requests (> 128 KB per BEP 0003)
                if length == 0 || length > 128 * 1024 {
                    return Ok(());
                }

                // If we have completed this piece and are not choking the peer, serve only the requested block
                let is_complete = {
                    let allocator = self.piece_allocator.lock().await;
                    (index as usize) < allocator.pieces.len()
                        && matches!(allocator.pieces[index as usize], PieceStatus::Complete)
                };

                if is_complete && !self.peer_state.am_choking {
                    if let Ok(Some(block)) = PieceIO::read_block_from_disk(
                        &self.file_map,
                        index,
                        begin,
                        length,
                        self.piece_length,
                        self.total_torrent_size,
                    )
                    .await
                    {
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
                    }
                }
            }

            PeerMessage::Cancel {
                index,
                begin,
                length,
            } => {
                let _ = (index, begin, length);
            }

            PeerMessage::Extended { id, payload } => {
                if id == pex::EXTENSION_HANDSHAKE_ID {
                    if let Some(remote_id) = pex::parse_remote_ut_pex_id(&payload) {
                        info!(
                            "PEX: Negotiated ut_pex with {}:{} (remote extended ID: {})",
                            self.ip_addr, self.port, remote_id
                        );
                        let was_none = self.remote_ut_pex_id.is_none();
                        self.remote_ut_pex_id = Some(remote_id);
                        if was_none {
                            // Send initial PEX message with currently connected peers
                            self.send_pex_update(remote_id, out_tx).await;
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

    /// Send a PEX update message to the remote peer advertising connected peers.
    async fn send_pex_update(
        &mut self,
        remote_pex_id: u8,
        out_tx: &mpsc::Sender<PeerMessage>,
    ) {
        let current_peers: Vec<(Ipv4Addr, u16)> = {
            let guard = self.connected_peers.lock().await;
            guard.iter().copied().collect()
        };

        let my_ipv4 = match self.ip_addr {
            IpAddr::V4(ipv4) => Some(ipv4),
            _ => None,
        };

        let added: Vec<(Ipv4Addr, u16)> = current_peers
            .iter()
            .filter(|&&(ip, port)| {
                if Some(ip) == my_ipv4 && port == self.port {
                    return false; // Don't advertise peer to itself
                }
                !self.last_sent_pex_peers.contains(&(ip, port))
            })
            .take(50)
            .copied()
            .collect();

        let current_set: HashSet<(Ipv4Addr, u16)> = current_peers.into_iter().collect();
        let dropped: Vec<(Ipv4Addr, u16)> = self
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
        if self.peer_state.peer_choking || self.current_piece.is_some() {
            return Ok(());
        }

        let piece_idx = self.piece_allocator.lock().await.allocate_piece(
            &self.peer_state.bitfield,
            &self.ip_addr,
            self.total_pieces,
        );

        if let Some(piece_idx) = piece_idx {
            self.current_piece = Some(piece_idx as u32);
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
        let max_pending: u32 = 16; // 16 * 16KB = 256KB in-flight pipeline depth

        while self.pending_requests < max_pending && self.requested_offset < actual_len {
            let this_block_len = block_size.min(actual_len - self.requested_offset);

            if out_tx.send(PeerMessage::Request {
                index: piece_idx,
                begin: self.requested_offset,
                length: this_block_len,
            }).await.is_err() {
                break;
            }

            self.requested_offset += this_block_len;
            self.pending_requests += 1;
        }

        Ok(())
    }

    fn calculate_peer_progress(&self) -> f64 {
        if self.total_pieces == 0 {
            return 0.0;
        }
        let count = (0..self.total_pieces)
            .filter(|&i| PieceAllocator::check_piece(&self.peer_state.bitfield, i))
            .count();
        (count as f64 / self.total_pieces as f64) * 100.0
    }
}
