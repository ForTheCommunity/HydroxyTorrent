
use std::{net::IpAddr, sync::Arc, time::Duration};

use hydroxy_peer_wire_proto::{handshake::Handshake, message::PeerMessage};
use hydroxy_state::TorrentState;
use hydroxy_storage_io::{file_region::FileRegion, piece_io::PieceIO};
use tokio::{net::TcpStream, sync::Mutex, sync::broadcast};
use tracing::{info, warn};

use crate::{
    errors::EngineError,
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
}

impl PeerWorker {
    pub async fn start(mut self) -> Result<(), EngineError> {
        let info_hash_sha1 = OtherInfos::to_sha1(&self.info_hash);
        let mut tcp_stream = TcpStream::connect(format!("{}:{}", self.ip_addr, self.port)).await?;

        let handshake = Handshake::new(self.info_hash, self.peer_id)
            .do_handshake(&mut tcp_stream)
            .await?;

        // Send local bitfield immediately after handshake per BEP 0003
        let our_bitfield = self
            .piece_allocator
            .lock()
            .await
            .to_bitfield(self.total_pieces);
        if !our_bitfield.is_empty() {
            PeerMessage::Bitfield(our_bitfield).write(&mut tcp_stream).await?;
        }

        let client_name = parse_peer_id(&handshake.peer_id);

        self.stats_registry.peer_connected(
            &info_hash_sha1,
            self.ip_addr,
            self.port,
            client_name,
        );

        let result = self.run_loop(&mut tcp_stream).await;

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

    async fn run_loop(&mut self, tcp_stream: &mut TcpStream) -> Result<(), EngineError> {
        // send a KeepAlive every 90 seconds to prevent
        // remote peers from dropping the connection due to inactivity.
        let mut keepalive_interval = tokio::time::interval(Duration::from_secs(90));
        keepalive_interval.tick().await; // consume the first immediate tick

        loop {
            let read_timeout = if self.current_piece.is_some() {
                Duration::from_secs(10)
            } else {
                Duration::from_secs(120)
            };

            tokio::select! {
                // Branch 1: Proactive keepalive heartbeat
                _ = keepalive_interval.tick() => {
                    PeerMessage::KeepAlive.write(tcp_stream).await?;
                }
                // Branch 2: Have broadcast from completed piece
                Ok(completed_piece) = self.have_rx.recv() => {
                    PeerMessage::Have(completed_piece).write(tcp_stream).await?;
                }
                // Branch 3: Read next peer message (with 10s stall or 120s idle timeout)
                result = tokio::time::timeout(
                    read_timeout,
                    PeerMessage::read(tcp_stream),
                ) => {
                    match result {
                        Ok(msg_result) => {
                            let peer_message = msg_result?;
                            self.handle_message(peer_message, tcp_stream).await?;
                        }
                        Err(_) => {
                            if let Some(piece_idx) = self.current_piece.take() {
                                warn!(
                                    "Worker {}:{} piece {} request stalled (10s no data) — releasing piece back to pool",
                                    self.ip_addr, self.port, piece_idx
                                );
                                self.piece_allocator
                                    .lock()
                                    .await
                                    .release_piece(piece_idx as usize);
                                self.piece_assembler.clear_piece(piece_idx);
                                self.pending_requests = 0;
                                self.requested_offset = 0;
                                self.try_start_downloading(tcp_stream).await?;
                            } else {
                                PeerMessage::KeepAlive.write(tcp_stream).await?;
                            }
                        }
                    }
                }
            }
        }
    }

    async fn handle_message(
        &mut self,
        peer_message: PeerMessage,
        tcp_stream: &mut TcpStream,
    ) -> Result<(), EngineError> {
        let info_hash_sha1 = OtherInfos::to_sha1(&self.info_hash);
        match peer_message {
            PeerMessage::KeepAlive => {
                // Per spec, KeepAlive is just consumed silently.
                // Do NOT echo it back — that creates an infinite ping-pong loop.
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
                self.try_start_downloading(tcp_stream).await?;
            }
            PeerMessage::Interested => {
                self.peer_state.peer_interested = true;
            }
            PeerMessage::NotInterested => {
                self.peer_state.peer_interested = false;
            }
            PeerMessage::Have(index) => {
                // Update our local bitfield so we know they have this new piece
                {
                    let mut piece_allocator = self.piece_allocator.lock().await;
                    piece_allocator.register_have(index as usize);
                } // lock dropped here

                let byte = index as usize / 8;
                let bit = 7 - (index as usize % 8);
                if let Some(b) = self.peer_state.bitfield.get_mut(byte) {
                    *b |= 1 << bit;
                }

                if !self.peer_state.am_interested {
                    let is_missing = matches!(
                        self.piece_allocator.lock().await.pieces[index as usize],
                        PieceStatus::Missing
                    );
                    if is_missing {
                        self.peer_state.am_interested = true;
                        PeerMessage::Interested.write(tcp_stream).await?;
                    }
                }

                // If we're unchoked and idle (no current piece), try to
                // start downloading now that the peer advertised a new piece.
                if !self.peer_state.peer_choking && self.current_piece.is_none() {
                    self.try_start_downloading(tcp_stream).await?;
                }
            }
            PeerMessage::Bitfield(bitfield) => {
                // registering piece availablity
                // rarest Piece First Strategy
                {
                    let mut piece_allocator = self.piece_allocator.lock().await;
                    piece_allocator.register_bitfield(&bitfield, self.total_pieces);
                }

                // Store on peer state for future allocate_piece calls
                self.peer_state.bitfield = bitfield;

                // Express interest if they have anything we need
                let has_usefull = {
                    let piece_allocator = self.piece_allocator.lock().await;
                    (0..self.total_pieces).any(|i| {
                        PieceAllocator::check_piece(&self.peer_state.bitfield, i)
                            && matches!(piece_allocator.pieces[i], PieceStatus::Missing)
                    })
                };

                if has_usefull {
                    self.peer_state.am_interested = true;
                    PeerMessage::Interested.write(tcp_stream).await?;
                }
            }

            PeerMessage::Piece {
                index,
                begin,
                block,
            } => {
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
                    let expected_hash = &self.piece_hashes[hash_start..hash_start + 20];

                    if !PieceAssembler::verify_piece(&assembled, expected_hash) {
                        warn!("Piece {} failed SHA1 — releasing back to pool", index);
                        self.piece_allocator
                            .lock()
                            .await
                            .release_piece(index as usize);
                        self.piece_assembler.clear_piece(index);
                        // Reset state and try to get a new piece
                        self.current_piece = None;
                        self.pending_requests = 0;
                        self.requested_offset = 0;
                        self.try_start_downloading(tcp_stream).await?;
                        return Ok(());
                    }

                    // write to disk
                    PieceIO {
                        data: assembled,
                        piece_index: index,
                        piece_length: self.piece_length,
                        file_map: (*self.file_map).clone(),
                    }
                    .write_to_disk()
                    .await?;

                    info!("Piece {} written to disk", index);
                    self.stats_registry.piece_completed(&info_hash_sha1, self.ip_addr, self.port, index as usize);

                    // update fast-resume state
                    {
                        let mut state = self.torrent_state.lock().await;
                        state.mark_completed_piece(index as usize);
                        let snapshot = state.clone();
                        drop(state);
                        tokio::spawn(async move {
                            if let Err(e) = TorrentState::save_state(snapshot).await {
                                warn!("State save failed after piece {}: {}", index, e);
                            }
                        });
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
                    self.try_start_downloading(tcp_stream).await?;
                } else {
                    // piece incomplete — keep pipeline full
                    if let Some(piece_idx) = self.current_piece {
                        if !self.peer_state.peer_choking {
                            self.fill_pipeline(piece_idx, tcp_stream).await?;
                        }
                    }
                }
            }


            PeerMessage::Request {
                index,
                begin,
                length,
            } => {
                let _ = (index, begin, length);
                // TODO: implement uploading when seeding is supported
            }

            PeerMessage::Cancel {
                index,
                begin,
                length,
            } => {
                let _ = (index, begin, length);
                // end-game cancellation not implemented yet
            }
        }

        Ok(())
    }

    /// Attempt to allocate a new piece and begin downloading it.
    /// Called when unchoked, after completing a piece, or when a `Have`
    /// message arrives while idle.
    async fn try_start_downloading(
        &mut self,
        tcp_stream: &mut TcpStream,
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
            self.fill_pipeline(piece_idx as u32, tcp_stream).await?;
        }

        Ok(())
    }

    async fn fill_pipeline(
        &mut self,
        piece_idx: u32,
        tcp_stream: &mut TcpStream,
    ) -> Result<(), EngineError> {
        let actual_len = PieceAssembler::actual_piece_length(
            piece_idx,
            self.total_pieces as u32,
            self.piece_length,
            self.total_torrent_size,
        );

        let block_size: u32 = 16_384;

        while self.pending_requests < 5 && self.requested_offset < actual_len {
            let this_block_len = block_size.min(actual_len - self.requested_offset);

            PeerMessage::Request {
                index: piece_idx,
                begin: self.requested_offset,
                length: this_block_len,
            }
            .write(tcp_stream)
            .await?;

            self.requested_offset += this_block_len;
            self.pending_requests += 1;
        }

        Ok(())
    }
}
