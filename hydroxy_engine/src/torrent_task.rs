
use std::{
    collections::{HashSet, VecDeque},
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};

use hydroxy_state::TorrentState;
use hydroxy_storage_io::file_region::FileRegion;
use hydroxy_tracker::{Peer, Tracker, TrackerEvents};
use hydroxy_utils::{
    get_peer_id,
    torrent_info::{OtherInfos, TorrentInfo},
};
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio::task::JoinSet;
use tracing::{info, warn};

use crate::{
    control::{TorrentCommand, TorrentControlRegistry},
    errors::EngineError,
    peer_worker::{PeerState, PeerWorker},
    piece_allocator::PieceAllocator,
    piece_assembler::PieceAssembler,
    stats::{
        stats::{TorrentFileStats, TrackerStats},
        stats_registry::StatsRegistry,
    },
};

const MAX_CONNECTED_PEERS_PER_TORRENT: usize = 30;

pub struct TorrentTask {
    pub root_dir: String,
    pub info_hash: [u8; 20],
    pub category: String,
    pub stats_registry: Arc<StatsRegistry>,
    pub control_registry: Arc<TorrentControlRegistry>,
}

impl TorrentTask {
    pub async fn start(self, torrent_info: TorrentInfo) -> Result<(), EngineError> {
        info!("Torrent Task Started...");

        let info_hash = self.info_hash;
        let info_hash_sha1 = OtherInfos::to_sha1(&info_hash);

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<TorrentCommand>(32);
        self.control_registry.register(&info_hash_sha1, cmd_tx);

        let mut hot_state = TorrentState::load_state(&info_hash_sha1, &torrent_info).await?;

        // filemap
        let file_map = Arc::new(FileRegion::build_file_map(&torrent_info, &self.root_dir));
        let total_torrent_size: u64 = file_map.iter().map(|r| r.length).sum();
        let total_pieces = torrent_info.info.pieces.len() / 20;
        let piece_length = torrent_info.info.piece_length as u32;
        let piece_hashes = Arc::new(torrent_info.info.pieces.clone());

        let mut trackers = hot_state.trackers.clone();
        if trackers.is_empty() && !torrent_info.announce.is_empty() {
            trackers.push(torrent_info.announce.clone());
        }

        let tracker_stats: Vec<TrackerStats> = trackers
            .iter()
            .enumerate()
            .map(|(i, url)| TrackerStats {
                tier: i + 1,
                url: url.clone(),
                status: "Not contacted yet".to_string(),
                peers: 0,
                interval: 0,
                next_announce: 0,
            })
            .collect();

        let file_stats: Vec<TorrentFileStats> = file_map
            .iter()
            .enumerate()
            .map(|(i, region)| {
                let name = std::path::Path::new(&region.path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("file_{}", i));
                TorrentFileStats {
                    id: i,
                    name,
                    path: region.path.clone(),
                    size: region.length,
                    offset: region.offset_in_torrent,
                    progress: 0.0,
                    priority: "Normal".to_string(),
                }
            })
            .collect();

        self.stats_registry.register_torrent(
            &info_hash_sha1,
            &torrent_info.info.name,
            &self.category,
            total_pieces,
            piece_length,
            total_torrent_size,
            tracker_stats,
            file_stats,
        );

        let any_file_exists = file_map.iter().any(|r| std::path::Path::new(&r.path).exists());
        let has_saved_pieces = hot_state.pieces.iter().any(|&b| b != 0);

        // When a new torrent is added (or state had no verified pieces) and files exist on disk, recheck them
        if any_file_exists && !has_saved_pieces {
            self.stats_registry.set_status(&info_hash_sha1, Some("checking".to_string()));
            let recheck = crate::piece_verifier::verify_torrent_data_on_disk(
                &file_map,
                &piece_hashes,
                piece_length,
                total_torrent_size,
                total_pieces,
            )
            .await;

            hot_state.pieces = recheck.bitfield;
            let _ = TorrentState::save_state(hot_state.clone()).await;
            self.stats_registry.set_initial_bitfield(
                &info_hash_sha1,
                &hot_state.pieces,
                recheck.completed_pieces,
                recheck.completed_bytes,
            );
            self.stats_registry.set_status(&info_hash_sha1, None);
        }

        let mut completed_pieces = 0;
        for (i, byte) in hot_state.pieces.iter().enumerate() {
            for bit in 0..8 {
                let piece_idx = i * 8 + (7 - bit);
                if piece_idx < total_pieces && (byte & (1 << bit)) != 0 {
                    completed_pieces += 1;
                }
            }
        }
        self.stats_registry.set_initial_bitfield(
            &info_hash_sha1,
            &hot_state.pieces,
            completed_pieces,
            completed_pieces as u64 * piece_length as u64,
        );

        // Piece Allocator
        let piece_allocator = Arc::new(Mutex::new(PieceAllocator::from_hotstate_bitfield(
            &hot_state.pieces,
            total_pieces,
        )));

        let mut is_paused = hot_state.paused == 1;
        let is_sequential = hot_state.sequential == 1;
        piece_allocator.lock().await.sequential = is_sequential;
        self.stats_registry.set_sequential(&info_hash_sha1, is_sequential);

        if piece_allocator.lock().await.all_complete() {
            info!("All pieces already downloaded — torrent complete");
            self.stats_registry.set_status(&info_hash_sha1, Some("completed".to_string()));
        } else if is_paused {
            self.stats_registry.set_status(&info_hash_sha1, Some("paused".to_string()));
        }

        // shared torrent state
        let torrent_state = Arc::new(Mutex::new(hot_state));

        // Generate one consistent peer_id for this entire torrent session
        let session_peer_id = get_peer_id();

        // Track connected peers to avoid spawning duplicate workers
        let connected_peers: Arc<Mutex<HashSet<(Ipv4Addr, u16)>>> =
            Arc::new(Mutex::new(HashSet::new()));

        // Broadcast channel for notifying peer workers when pieces complete
        let (have_tx, _) = broadcast::channel::<u32>(128);

        // Collect initial peers from all trackers
        let (initial_downloaded, initial_remaining) = {
            let alloc = piece_allocator.lock().await;
            (
                alloc.downloaded_bytes(piece_length, total_torrent_size),
                alloc.remaining_bytes(piece_length, total_torrent_size),
            )
        };

        let initial_peers = Self::announce_to_trackers(
            &trackers,
            &info_hash,
            session_peer_id,
            initial_downloaded,
            0,
            initial_remaining,
            Some(TrackerEvents::Started),
            Some((&self.stats_registry, &info_hash_sha1)),
        )
        .await;
        info!("Found Total Peers : {}", initial_peers.len());

        // Worker supervision via JoinSet
        let mut worker_set: JoinSet<(Ipv4Addr, u16, Result<(), EngineError>)> = JoinSet::new();

        // Queue of discovered peers waiting for an open connection slot
        let mut peer_pool: VecDeque<Peer> = VecDeque::from(initial_peers);

        // Spawn workers up to MAX_CONNECTED_PEERS_PER_TORRENT if not paused and not already complete
        if !is_paused && !piece_allocator.lock().await.all_complete() {
            while let Some(peer) = peer_pool.pop_front() {
                if !Self::spawn_worker_if_new(
                    &mut worker_set,
                    &connected_peers,
                    peer,
                    info_hash,
                    session_peer_id,
                    total_pieces,
                    piece_length,
                    total_torrent_size,
                    &piece_hashes,
                    &piece_allocator,
                    &file_map,
                    &torrent_state,
                    &self.stats_registry,
                    have_tx.clone(),
                )
                .await
                {
                    peer_pool.push_front(peer);
                    break;
                }
            }
        }

        // Channel for the re-announce task to send newly discovered peers
        let (new_peer_tx, mut new_peer_rx) = mpsc::channel::<Vec<Peer>>(16);

        // Spawn periodic re-announce background task
        let re_announce_trackers = trackers.clone();
        let re_announce_info_hash = info_hash;
        let re_announce_info_hash_sha1 = info_hash_sha1.clone();
        let re_announce_stats = Arc::clone(&self.stats_registry);
        let re_announce_allocator = Arc::clone(&piece_allocator);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(120));
            interval.tick().await; // skip the first immediate tick

            loop {
                interval.tick().await;

                let (is_complete, downloaded, remaining) = {
                    let alloc = re_announce_allocator.lock().await;
                    (
                        alloc.all_complete(),
                        alloc.downloaded_bytes(piece_length, total_torrent_size),
                        alloc.remaining_bytes(piece_length, total_torrent_size),
                    )
                };

                if is_complete {
                    info!("All pieces complete — sending completed announce & stopping re-announce");
                    let _ = Self::announce_to_trackers(
                        &re_announce_trackers,
                        &re_announce_info_hash,
                        session_peer_id,
                        downloaded,
                        0,
                        0,
                        Some(TrackerEvents::Completed),
                        Some((&re_announce_stats, &re_announce_info_hash_sha1)),
                    )
                    .await;
                    break;
                }

                info!("Re-announcing to trackers for fresh peers...");
                let peers = Self::announce_to_trackers(
                    &re_announce_trackers,
                    &re_announce_info_hash,
                    session_peer_id,
                    downloaded,
                    0,
                    remaining,
                    None,
                    Some((&re_announce_stats, &re_announce_info_hash_sha1)),
                )
                .await;

                if !peers.is_empty() {
                    info!("Re-announce discovered {} peers", peers.len());
                    if new_peer_tx.send(peers).await.is_err() {
                        break; // receiver dropped, torrent task exited
                    }
                }
            }
        });

        // Main supervision loop: watch for commands, finished workers & new peers
        loop {
            tokio::select! {
                cmd_opt = cmd_rx.recv() => {
                    let cmd = match cmd_opt {
                        Some(c) => c,
                        None => {
                            info!("Command channel closed for torrent {}", info_hash_sha1);
                            break;
                        }
                    };
                    match cmd {
                        TorrentCommand::Remove => {
                            info!("Removing torrent task: {}", info_hash_sha1);
                            worker_set.shutdown().await;
                            connected_peers.lock().await.clear();
                            break;
                        }
                        TorrentCommand::Pause => {
                            info!("Pausing torrent: {}", info_hash_sha1);
                            is_paused = true;
                            worker_set.shutdown().await;
                            connected_peers.lock().await.clear();
                            self.stats_registry.set_status(&info_hash_sha1, Some("paused".to_string()));
                            let mut state = torrent_state.lock().await;
                            state.paused = 1;
                            let _ = TorrentState::save_state(state.clone()).await;
                        }
                        TorrentCommand::SetSequential(val) => {
                            info!("Setting sequential download = {} for torrent {}", val, info_hash_sha1);
                            piece_allocator.lock().await.sequential = val;
                            self.stats_registry.set_sequential(&info_hash_sha1, val);
                            let mut state = torrent_state.lock().await;
                            state.sequential = if val { 1 } else { 0 };
                            let _ = TorrentState::save_state(state.clone()).await;
                        }
                        TorrentCommand::Resume => {
                            info!("Resuming torrent: {}", info_hash_sha1);
                            is_paused = false;
                            let mut state = torrent_state.lock().await;
                            state.paused = 0;
                            let _ = TorrentState::save_state(state.clone()).await;

                            let all_done = piece_allocator.lock().await.all_complete();
                            if all_done {
                                self.stats_registry.set_status(&info_hash_sha1, Some("completed".to_string()));
                            } else {
                                self.stats_registry.set_status(&info_hash_sha1, None);
                                while let Some(peer) = peer_pool.pop_front() {
                                    if !Self::spawn_worker_if_new(
                                        &mut worker_set,
                                        &connected_peers,
                                        peer,
                                        info_hash,
                                        session_peer_id,
                                        total_pieces,
                                        piece_length,
                                        total_torrent_size,
                                        &piece_hashes,
                                        &piece_allocator,
                                        &file_map,
                                        &torrent_state,
                                        &self.stats_registry,
                                        have_tx.clone(),
                                    )
                                    .await
                                    {
                                        peer_pool.push_front(peer);
                                        break;
                                    }
                                }
                            }
                        }
                        TorrentCommand::Recheck => {
                            info!("Force recheck initiated for: {}", info_hash_sha1);
                            worker_set.shutdown().await;
                            connected_peers.lock().await.clear();
                            self.stats_registry.set_status(&info_hash_sha1, Some("checking".to_string()));

                            let recheck = crate::piece_verifier::verify_torrent_data_on_disk(
                                &file_map,
                                &piece_hashes,
                                piece_length,
                                total_torrent_size,
                                total_pieces,
                            ).await;

                            {
                                let mut state = torrent_state.lock().await;
                                state.pieces = recheck.bitfield.clone();
                                let _ = TorrentState::save_state(state.clone()).await;
                            }
                            {
                                let mut alloc = piece_allocator.lock().await;
                                *alloc = PieceAllocator::from_hotstate_bitfield(&recheck.bitfield, total_pieces);
                            }
                            self.stats_registry.set_initial_bitfield(
                                &info_hash_sha1,
                                &recheck.bitfield,
                                recheck.completed_pieces,
                                recheck.completed_bytes,
                            );

                            if recheck.completed_pieces == total_pieces {
                                self.stats_registry.set_status(&info_hash_sha1, Some("completed".to_string()));
                            } else if is_paused {
                                self.stats_registry.set_status(&info_hash_sha1, Some("paused".to_string()));
                            } else {
                                self.stats_registry.set_status(&info_hash_sha1, None);
                                while let Some(peer) = peer_pool.pop_front() {
                                    if !Self::spawn_worker_if_new(
                                        &mut worker_set,
                                        &connected_peers,
                                        peer,
                                        info_hash,
                                        session_peer_id,
                                        total_pieces,
                                        piece_length,
                                        total_torrent_size,
                                        &piece_hashes,
                                        &piece_allocator,
                                        &file_map,
                                        &torrent_state,
                                        &self.stats_registry,
                                        have_tx.clone(),
                                    )
                                    .await
                                    {
                                        peer_pool.push_front(peer);
                                        break;
                                    }
                                }
                            }
                        }
                        TorrentCommand::Reannounce => {
                            info!("Manual reannounce initiated for: {}", info_hash_sha1);
                            let (downloaded, remaining) = {
                                let alloc = piece_allocator.lock().await;
                                (
                                    alloc.downloaded_bytes(piece_length, total_torrent_size),
                                    alloc.remaining_bytes(piece_length, total_torrent_size),
                                )
                            };
                            let peers = Self::announce_to_trackers(
                                &trackers,
                                &info_hash,
                                session_peer_id,
                                downloaded,
                                0,
                                remaining,
                                None,
                                Some((&self.stats_registry, &info_hash_sha1)),
                            )
                            .await;
                            for peer in peers {
                                let is_connected = connected_peers.lock().await.contains(&(peer.ipv4_addr, peer.port));
                                if !is_connected {
                                    peer_pool.push_back(peer);
                                }
                            }
                            if !is_paused && !piece_allocator.lock().await.all_complete() {
                                while let Some(peer) = peer_pool.pop_front() {
                                    if !Self::spawn_worker_if_new(
                                        &mut worker_set,
                                        &connected_peers,
                                        peer,
                                        info_hash,
                                        session_peer_id,
                                        total_pieces,
                                        piece_length,
                                        total_torrent_size,
                                        &piece_hashes,
                                        &piece_allocator,
                                        &file_map,
                                        &torrent_state,
                                        &self.stats_registry,
                                        have_tx.clone(),
                                    )
                                    .await
                                    {
                                        peer_pool.push_front(peer);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                // Handle newly discovered peers from re-announce
                Some(new_peers) = new_peer_rx.recv() => {
                    for peer in new_peers {
                        let is_connected = connected_peers.lock().await.contains(&(peer.ipv4_addr, peer.port));
                        if !is_connected {
                            peer_pool.push_back(peer);
                        }
                    }

                    if !is_paused && !piece_allocator.lock().await.all_complete() {
                        while let Some(peer) = peer_pool.pop_front() {
                            if !Self::spawn_worker_if_new(
                                &mut worker_set,
                                &connected_peers,
                                peer,
                                info_hash,
                                session_peer_id,
                                total_pieces,
                                piece_length,
                                total_torrent_size,
                                &piece_hashes,
                                &piece_allocator,
                                &file_map,
                                &torrent_state,
                                &self.stats_registry,
                                have_tx.clone(),
                            )
                            .await
                            {
                                peer_pool.push_front(peer);
                                break;
                            }
                        }
                    }
                }
                // Handle completed/failed workers
                Some(result) = worker_set.join_next(), if !worker_set.is_empty() => {
                    match result {
                        Ok((ip, port, worker_result)) => {
                            // Remove from connected set so re-announce can
                            // reconnect to this peer later if needed
                            connected_peers.lock().await.remove(&(ip, port));

                            if let Err(e) = worker_result {
                                warn!("Worker {}:{} finished with error: {}", ip, port, e);
                            }

                            if !is_paused && !piece_allocator.lock().await.all_complete() {
                                while let Some(next_peer) = peer_pool.pop_front() {
                                    if Self::spawn_worker_if_new(
                                        &mut worker_set,
                                        &connected_peers,
                                        next_peer,
                                        info_hash,
                                        session_peer_id,
                                        total_pieces,
                                        piece_length,
                                        total_torrent_size,
                                        &piece_hashes,
                                        &piece_allocator,
                                        &file_map,
                                        &torrent_state,
                                        &self.stats_registry,
                                        have_tx.clone(),
                                    )
                                    .await
                                    {
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Worker task panicked: {}", e);
                        }
                    }

                    // Check if all pieces are complete
                    if piece_allocator.lock().await.all_complete() {
                        info!("All pieces downloaded successfully!");
                        self.stats_registry.set_status(&info_hash_sha1, Some("completed".to_string()));
                    }
                }
            }
        }

        self.control_registry.unregister(&info_hash_sha1);

        let (final_downloaded, final_remaining, is_complete) = {
            let alloc = piece_allocator.lock().await;
            (
                alloc.downloaded_bytes(piece_length, total_torrent_size),
                alloc.remaining_bytes(piece_length, total_torrent_size),
                alloc.all_complete(),
            )
        };
        let final_event = if is_complete {
            Some(TrackerEvents::Completed)
        } else {
            Some(TrackerEvents::Stopped)
        };
        let _ = Self::announce_to_trackers(
            &trackers,
            &info_hash,
            session_peer_id,
            final_downloaded,
            0,
            final_remaining,
            final_event,
            Some((&self.stats_registry, &info_hash_sha1)),
        )
        .await;

        Ok(())
    }

    // Contact all trackers and collect peers.
    async fn announce_to_trackers(
        trackers: &[String],
        info_hash: &[u8; 20],
        peer_id: [u8; 20],
        downloaded: u64,
        uploaded: u64,
        remaining_size: u64,
        event: Option<TrackerEvents>,
        stats: Option<(&Arc<StatsRegistry>, &str)>,
    ) -> Vec<Peer> {
        let mut peers: Vec<Peer> = Vec::new();

        for announce_url in trackers {
            if let Some((reg, hash)) = stats {
                reg.update_tracker(hash, announce_url, "Updating...", None, None, None);
            }
            let tracker = Tracker {
                announce_url: announce_url.clone(),
                downloaded: downloaded as i64,
                uploaded: uploaded as i64,
                remaining_size: remaining_size as i64,
                info_hash: *info_hash,
                peer_id,
                port: "9060".into(),
                event,
            };

            match tracker.announce().await {
                Ok(tracker_response) => {
                    let peer_count = tracker_response.ipv4_peers.len();
                    let interval = if tracker_response.interval > 0 {
                        tracker_response.interval as u64
                    } else {
                        120
                    };
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let next_announce = now + interval;

                    if let Some((reg, hash)) = stats {
                        reg.update_tracker(
                            hash,
                            announce_url,
                            "Working",
                            Some(peer_count),
                            Some(interval),
                            Some(next_announce),
                        );
                    }
                    peers.extend(tracker_response.ipv4_peers);
                    info!(
                        "found : {peer_count} peers from tracker -> {} (next announce in {}s)",
                        announce_url, interval
                    );
                }
                Err(e) => {
                    let err_msg = format!("Error: {}", e);
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    if let Some((reg, hash)) = stats {
                        reg.update_tracker(
                            hash,
                            announce_url,
                            &err_msg,
                            Some(0),
                            None,
                            Some(now + 60),
                        );
                    }
                    warn!("ERROR -> {}", e.to_string());
                }
            }
        }

        peers
    }

    // Spawn a worker for a peer only if we don't already have an active
    // connection to that peer (deduplicated by ip:port) and within connection cap.
    async fn spawn_worker_if_new(
        worker_set: &mut JoinSet<(Ipv4Addr, u16, Result<(), EngineError>)>,
        connected_peers: &Arc<Mutex<HashSet<(Ipv4Addr, u16)>>>,
        peer: Peer,
        info_hash: [u8; 20],
        session_peer_id: [u8; 20],
        total_pieces: usize,
        piece_length: u32,
        total_torrent_size: u64,
        piece_hashes: &Arc<Vec<u8>>,
        piece_allocator: &Arc<Mutex<PieceAllocator>>,
        file_map: &Arc<Vec<FileRegion>>,
        torrent_state: &Arc<Mutex<TorrentState>>,
        stats_registry: &Arc<StatsRegistry>,
        have_tx: broadcast::Sender<u32>,
    ) -> bool {
        let key = (peer.ipv4_addr, peer.port);
        {
            let mut connected = connected_peers.lock().await;
            if connected.len() >= MAX_CONNECTED_PEERS_PER_TORRENT {
                return false;
            }
            if !connected.insert(key) {
                return false; // already have an active worker for this peer
            }
        }

        let piece_allocator = Arc::clone(piece_allocator);
        let torrent_state = Arc::clone(torrent_state);
        let file_map = Arc::clone(file_map);
        let stats_registry = Arc::clone(stats_registry);
        let piece_hashes = Arc::clone(piece_hashes);
        let peer_ip = peer.ipv4_addr;
        let peer_port = peer.port;
        let have_rx = have_tx.subscribe();

        worker_set.spawn(async move {
            let worker = PeerWorker {
                ip_addr: IpAddr::from(peer.ipv4_addr),
                port: peer.port,
                info_hash,
                peer_id: session_peer_id,
                peer_state: PeerState {
                    am_choking: true,
                    am_interested: false,
                    peer_choking: true,
                    peer_interested: false,
                    bitfield: vec![],
                },
                total_pieces,
                piece_allocator,
                piece_assembler: PieceAssembler::new(),
                total_torrent_size,
                piece_length,
                piece_hashes,
                file_map,
                torrent_state,
                pending_requests: 0,
                current_piece: None,
                requested_offset: 0,
                stats_registry,
                have_tx,
                have_rx,
            };

            let result = worker.start().await;
            if let Err(ref e) = result {
                warn!(
                    "Peer: {}:{}, error: {}",
                    peer.ipv4_addr,
                    peer.port,
                    e.to_string()
                );
            }

            (peer_ip, peer_port, result)
        });

        true
    }
}

