
use std::{
    collections::{HashMap, HashSet, VecDeque},
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
use tokio::sync::{Mutex, Semaphore, broadcast, mpsc};
use tokio::task::JoinSet;
use tracing::{info, warn};

use crate::{
    control::{TorrentCommand, TorrentControlRegistry},
    errors::EngineError,
    peer_listener::{InboundConnection, InboundTorrentRegistry},
    peer_worker::{PeerState, PeerWorker},
    piece_allocator::PieceAllocator,
    piece_assembler::PieceAssembler,
    queue::DownloadQueueManager,
    stats::{
        stats::{TorrentFileStats, TrackerStats},
        stats_registry::StatsRegistry,
    },
};

const MAX_CONNECTED_PEERS_PER_TORRENT: usize = 50;
const MAX_PEER_POOL_SIZE: usize = 500;
const MAX_FAILED_PEERS_CACHE: usize = 1000;
const FAILED_PEER_COOLDOWN: std::time::Duration = std::time::Duration::from_secs(180);
const MAX_CONCURRENT_TRACKER_ANNOUNCES_PER_TORRENT: usize = 8;

#[derive(PartialEq, Eq)]
enum SpawnPeerOutcome {
    Spawned,
    AlreadyConnected,
    AtCapacity,
}

pub struct TorrentTask {
    pub root_dir: String,
    pub info_hash: [u8; 20],
    pub category: String,
    pub stats_registry: Arc<StatsRegistry>,
    pub control_registry: Arc<TorrentControlRegistry>,
    pub listen_port: u16,
    pub inbound_registry: Arc<InboundTorrentRegistry>,
    pub queue_manager: Arc<DownloadQueueManager>,
}

impl TorrentTask {
    pub async fn start(self, torrent_info: TorrentInfo) -> Result<(), EngineError> {
        let info_hash = self.info_hash;
        let info_hash_sha1 = OtherInfos::to_sha1(&info_hash);
        info!(
            target: "major",
            "Torrent task started: '{}' ({})",
            torrent_info.info.name,
            info_hash_sha1
        );

        let (cmd_tx, mut cmd_rx) = mpsc::channel::<TorrentCommand>(32);
        self.control_registry.register(&info_hash_sha1, cmd_tx);

        let mut hot_state = TorrentState::load_state(&info_hash_sha1, &torrent_info).await?;

        // filemap
        let file_map = Arc::new(FileRegion::build_file_map(&torrent_info, &self.root_dir));
        let total_torrent_size: u64 = file_map.iter().map(|r| r.length).sum();
        let total_pieces = torrent_info.info.pieces.len() / 20;
        let piece_length = torrent_info.info.piece_length as u32;
        let torrent_name = torrent_info.info.name;
        let torrent_announce = torrent_info.announce;
        let piece_hashes = Arc::new(torrent_info.info.pieces);

        let mut trackers = Vec::new();
        let mut seen_trackers = HashSet::new();
        for url in hot_state.trackers.iter() {
            let trimmed = url.trim();
            if !trimmed.is_empty() && seen_trackers.insert(trimmed.to_string()) {
                trackers.push(trimmed.to_string());
            }
        }
        if trackers.is_empty() && !torrent_announce.trim().is_empty() {
            trackers.push(torrent_announce.trim().to_string());
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

        let root_dir_prefix = format!("{}/", self.root_dir.trim_end_matches('/'));
        let file_stats: Vec<TorrentFileStats> = file_map
            .iter()
            .enumerate()
            .map(|(i, region)| {
                let name = std::path::Path::new(&region.path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("file_{}", i));
                let rel_path = region
                    .path
                    .strip_prefix(&root_dir_prefix)
                    .or_else(|| region.path.strip_prefix(&self.root_dir))
                    .unwrap_or(&region.path)
                    .trim_start_matches('/')
                    .to_string();
                TorrentFileStats {
                    id: i,
                    name,
                    path: rel_path,
                    size: region.length,
                    offset: region.offset_in_torrent,
                    progress: 0.0,
                    priority: "Normal".to_string(),
                }
            })
            .collect();

        self.stats_registry.register_torrent(
            &info_hash_sha1,
            &torrent_name,
            &self.category,
            total_pieces,
            piece_length,
            total_torrent_size,
            tracker_stats,
            file_stats,
        );
        drop(torrent_name);

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
            hot_state.total_downloaded = recheck.completed_bytes;
            let _ = TorrentState::save_state(hot_state.clone()).await;
            self.stats_registry.set_initial_bitfield(
                &info_hash_sha1,
                &hot_state.pieces,
                recheck.completed_pieces,
                recheck.completed_bytes,
            );
            self.stats_registry.set_status(&info_hash_sha1, None);
        }

        // Piece Allocator
        let piece_allocator = Arc::new(Mutex::new(PieceAllocator::from_hotstate_bitfield(
            &hot_state.pieces,
            total_pieces,
        )));

        let (completed_pieces, completed_bytes) = {
            let alloc = piece_allocator.lock().await;
            (
                alloc.completed_pieces_count(),
                alloc.downloaded_bytes(piece_length, total_torrent_size),
            )
        };

        let initial_downloaded_bytes = hot_state
            .total_downloaded
            .max(completed_bytes)
            .min(total_torrent_size);
        let initial_uploaded_bytes = hot_state.total_uploaded;

        if hot_state.total_downloaded != initial_downloaded_bytes {
            hot_state.total_downloaded = initial_downloaded_bytes;
            let _ = TorrentState::save_state(hot_state.clone()).await;
        }

        self.stats_registry.set_initial_bitfield(
            &info_hash_sha1,
            &hot_state.pieces,
            completed_pieces,
            initial_downloaded_bytes,
        );
        self.stats_registry
            .set_uploaded_bytes(&info_hash_sha1, initial_uploaded_bytes);

        let mut is_paused = hot_state.paused == 1;
        let is_sequential = hot_state.sequential == 1;
        piece_allocator.lock().await.sequential = is_sequential;
        self.stats_registry.set_sequential(&info_hash_sha1, is_sequential);

        let all_done = piece_allocator.lock().await.all_complete();
        let mut has_download_slot = false;
        let is_active_for_announce = Arc::new(std::sync::atomic::AtomicBool::new(false));

        let (mut slot_rx, is_active_now) = if all_done {
            info!(
                target: "major",
                "Torrent complete and seeding: ({})",
                info_hash_sha1
            );
            self.stats_registry.set_status(&info_hash_sha1, Some("seeding".to_string()));
            is_active_for_announce.store(true, std::sync::atomic::Ordering::Relaxed);
            let (_, rx) = mpsc::channel(1);
            (rx, false)
        } else if is_paused {
            self.stats_registry.set_status(&info_hash_sha1, Some("paused".to_string()));
            let (_, rx) = mpsc::channel(1);
            (rx, false)
        } else {
            let (active, rx) = self.queue_manager.acquire_download_slot(info_hash).await;
            if active {
                has_download_slot = true;
                is_active_for_announce.store(true, std::sync::atomic::Ordering::Relaxed);
                self.stats_registry.set_status(&info_hash_sha1, None); // "downloading"
            } else {
                has_download_slot = false;
                self.stats_registry.set_status(&info_hash_sha1, Some("queued".to_string()));
            }
            (rx, active)
        };

        // shared torrent state
        let torrent_state = Arc::new(Mutex::new(hot_state));

        // Generate one consistent peer_id for this entire torrent session
        let session_peer_id = get_peer_id();

        // Register with engine-wide inbound listener registry so incoming peers can connect
        let (inbound_tx, mut inbound_rx) = mpsc::channel::<InboundConnection>(32);
        self.inbound_registry
            .register(info_hash, session_peer_id, inbound_tx)
            .await;

        // Track connected peers to avoid spawning duplicate workers
        let connected_peers: Arc<Mutex<HashSet<(Ipv4Addr, u16)>>> =
            Arc::new(Mutex::new(HashSet::new()));

        // Broadcast channel for notifying peer workers when pieces complete
        let (have_tx, _) = broadcast::channel::<u32>(128);

        // Worker supervision via JoinSet
        let mut worker_set: JoinSet<(Ipv4Addr, u16, Result<(), EngineError>)> = JoinSet::new();

        // Queue of discovered peers waiting for an open connection slot
        let mut peer_pool: VecDeque<Peer> = VecDeque::new();
        // Cooldown map for unreachable/failed peers so PEX does not endlessly re-dial dead IPs
        let mut failed_peers: HashMap<(Ipv4Addr, u16), std::time::Instant> = HashMap::new();

        // Channel for announce tasks to stream newly discovered peers directly to the main loop
        let (new_peer_tx, mut new_peer_rx) = mpsc::channel::<Vec<Peer>>(64);

        let (initial_downloaded, initial_remaining) = {
            let alloc = piece_allocator.lock().await;
            (
                alloc.downloaded_bytes(piece_length, total_torrent_size),
                alloc.remaining_bytes(piece_length, total_torrent_size),
            )
        };

        // Announce to all trackers concurrently in parallel, streaming peers as each tracker responds
        if !is_paused && (all_done || is_active_now) && !trackers.is_empty() {
            let init_trackers = trackers.clone();
            let init_stats = Arc::clone(&self.stats_registry);
            let init_hash_sha1 = info_hash_sha1.clone();
            let init_peer_tx = new_peer_tx.clone();
            let port = self.listen_port;
            tokio::spawn(async move {
                Self::announce_to_trackers_stream(
                    &init_trackers,
                    &info_hash,
                    session_peer_id,
                    port,
                    initial_downloaded,
                    initial_uploaded_bytes,
                    initial_remaining,
                    Some(TrackerEvents::Started),
                    Some((&init_stats, &init_hash_sha1)),
                    &init_peer_tx,
                )
                .await;
            });
        }

        // Spawn periodic re-announce background task
        let re_announce_trackers = trackers.clone();
        let re_announce_info_hash = info_hash;
        let re_announce_info_hash_sha1 = info_hash_sha1.clone();
        let re_announce_stats = Arc::clone(&self.stats_registry);
        let re_announce_allocator = Arc::clone(&piece_allocator);
        let re_announce_peer_tx = new_peer_tx.clone();
        let re_announce_port = self.listen_port;
        let initially_complete = all_done;
        let re_announce_active = Arc::clone(&is_active_for_announce);

        let re_announce_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(120));
            interval.tick().await; // skip the first immediate tick

            let mut completed_sent = initially_complete;

            loop {
                interval.tick().await;

                if !re_announce_active.load(std::sync::atomic::Ordering::Relaxed) {
                    continue; // Skip announcing while queued or paused
                }

                let (is_complete, downloaded, remaining) = {
                    let alloc = re_announce_allocator.lock().await;
                    (
                        alloc.all_complete(),
                        alloc.downloaded_bytes(piece_length, total_torrent_size),
                        alloc.remaining_bytes(piece_length, total_torrent_size),
                    )
                };

                let uploaded = re_announce_stats
                    .snapshot_torrent(&re_announce_info_hash_sha1)
                    .map(|s| s.uploaded_bytes)
                    .unwrap_or(0);

                let event = if is_complete && !completed_sent {
                    completed_sent = true;
                    info!("All pieces complete — sending completed announce to trackers");
                    Some(TrackerEvents::Completed)
                } else {
                    None
                };

                let due_trackers: Vec<String> = if event.is_some() {
                    re_announce_trackers.clone()
                } else {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let tracker_snaps =
                        re_announce_stats.snapshot_trackers(&re_announce_info_hash_sha1);
                    re_announce_trackers
                        .iter()
                        .filter(|url| {
                            tracker_snaps
                                .iter()
                                .find(|t| &t.url == *url)
                                .map(|t| t.next_announce <= now)
                                .unwrap_or(true)
                        })
                        .cloned()
                        .collect()
                };

                if !due_trackers.is_empty() {
                    info!(
                        "Re-announcing to {}/{} due trackers (seeding={}) for fresh peers...",
                        due_trackers.len(),
                        re_announce_trackers.len(),
                        is_complete
                    );
                    Self::announce_to_trackers_stream(
                        &due_trackers,
                        &re_announce_info_hash,
                        session_peer_id,
                        re_announce_port,
                        downloaded,
                        uploaded,
                        remaining,
                        event,
                        Some((&re_announce_stats, &re_announce_info_hash_sha1)),
                        &re_announce_peer_tx,
                    )
                    .await;
                }
            }
        });

        // Inactivity / stall tracking for queue quota management
        let mut zero_speed_since: Option<std::time::Instant> = None;
        let mut is_stalled = false;
        let mut stall_check_interval = tokio::time::interval(std::time::Duration::from_secs(5));
        stall_check_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Main supervision loop: watch for commands, finished workers & new peers
        loop {
            tokio::select! {
                _ = stall_check_interval.tick() => {
                    // Periodically persist updated uploaded/downloaded byte totals to .hot_state (especially while seeding)
                    if let Some(snap) = self.stats_registry.snapshot_torrent(&info_hash_sha1) {
                        let mut state = torrent_state.lock().await;
                        let new_dl = state.total_downloaded.max(snap.downloaded_bytes).min(total_torrent_size);
                        let new_ul = state.total_uploaded.max(snap.uploaded_bytes);
                        if new_dl != state.total_downloaded || new_ul != state.total_uploaded {
                            state.total_downloaded = new_dl;
                            state.total_uploaded = new_ul;
                            let snapshot = state.clone();
                            drop(state);
                            let _ = TorrentState::save_state(snapshot).await;
                        }
                    }

                    // Only evaluate stall for actively downloading torrents that have not completed
                    if has_download_slot && !is_paused {
                        let is_done = piece_allocator.lock().await.all_complete();
                        if !is_done {
                            let speed = self.stats_registry
                                .snapshot_torrent(&info_hash_sha1)
                                .map(|s| s.download_speed)
                                .unwrap_or(0.0);

                            if speed <= 10.0 {
                                match zero_speed_since {
                                    None => {
                                        zero_speed_since = Some(std::time::Instant::now());
                                    }
                                    Some(since) => {
                                        // After 60 seconds of 0 B/s, mark status as stalled in registry
                                        if since.elapsed() >= std::time::Duration::from_secs(60) {
                                            if !is_stalled {
                                                is_stalled = true;
                                                self.stats_registry.set_status(&info_hash_sha1, Some("stalled".to_string()));
                                                info!(
                                                    "Torrent {} is stalled (0 B/s for >60s)",
                                                    info_hash_sha1
                                                );
                                            }

                                            // If other torrents are waiting in queue, yield active slot to rotate and fill download quota
                                            if self.queue_manager.has_waiting().await {
                                                info!(
                                                    "Torrent {} yielding active slot to waiting queue for rotation",
                                                    info_hash_sha1
                                                );
                                                has_download_slot = false;
                                                zero_speed_since = None;
                                                is_active_for_announce.store(false, std::sync::atomic::Ordering::Relaxed);
                                                worker_set.shutdown().await;
                                                piece_allocator.lock().await.reset_in_progress();
                                                connected_peers.lock().await.clear();
                                                self.stats_registry.clear_peers(&info_hash_sha1);

                                                slot_rx = self.queue_manager.yield_stalled_slot(info_hash).await;
                                            }
                                        }
                                    }
                                }
                            } else {
                                zero_speed_since = None;
                                if is_stalled {
                                    is_stalled = false;
                                    self.stats_registry.set_status(&info_hash_sha1, None);
                                    info!(
                                        "Torrent {} resumed active downloading ({:.1} B/s)",
                                        info_hash_sha1, speed
                                    );
                                }
                            }
                        }
                    }
                }
                cmd_opt = cmd_rx.recv() => {
                    let cmd = match cmd_opt {
                        Some(c) => c,
                        None => {
                            info!("Command channel closed for torrent {}", info_hash_sha1);
                            break;
                        }
                    };
                    match cmd {
                        TorrentCommand::Remove | TorrentCommand::Shutdown => {
                            info!(target: "major", "Stopping torrent task ({:?}): {}", cmd, info_hash_sha1);
                            is_active_for_announce.store(false, std::sync::atomic::Ordering::Relaxed);
                            re_announce_handle.abort();
                            worker_set.shutdown().await;
                            piece_allocator.lock().await.reset_in_progress();
                            connected_peers.lock().await.clear();
                            self.stats_registry.clear_peers(&info_hash_sha1);
                            peer_pool.clear();
                            peer_pool.shrink_to_fit();
                            self.inbound_registry.unregister(&info_hash).await;
                            self.queue_manager.release_download_slot(&info_hash).await;
                            if cmd == TorrentCommand::Shutdown {
                                if let Some(snap) = self.stats_registry.snapshot_torrent(&info_hash_sha1) {
                                    let mut state = torrent_state.lock().await;
                                    state.total_downloaded = state.total_downloaded.max(snap.downloaded_bytes).min(total_torrent_size);
                                    state.total_uploaded = state.total_uploaded.max(snap.uploaded_bytes);
                                    let _ = TorrentState::save_state(state.clone()).await;
                                }
                            }
                            break;
                        }
                        TorrentCommand::Pause => {
                            info!(target: "major", "Torrent paused: {}", info_hash_sha1);
                            is_paused = true;
                            has_download_slot = false;
                            is_stalled = false;
                            zero_speed_since = None;
                            is_active_for_announce.store(false, std::sync::atomic::Ordering::Relaxed);
                            self.queue_manager.release_download_slot(&info_hash).await;
                            worker_set.shutdown().await;
                            piece_allocator.lock().await.reset_in_progress();
                            connected_peers.lock().await.clear();
                            self.stats_registry.clear_peers(&info_hash_sha1);
                            self.stats_registry.set_status(&info_hash_sha1, Some("paused".to_string()));
                            let snap = self.stats_registry.snapshot_torrent(&info_hash_sha1);
                            let mut state = torrent_state.lock().await;
                            state.paused = 1;
                            if let Some(ref s) = snap {
                                state.total_downloaded = state.total_downloaded.max(s.downloaded_bytes).min(total_torrent_size);
                                state.total_uploaded = state.total_uploaded.max(s.uploaded_bytes);
                            }
                            let ul = state.total_uploaded;
                            let _ = TorrentState::save_state(state.clone()).await;

                            if !trackers.is_empty() {
                                let (dl, rem) = {
                                    let alloc = piece_allocator.lock().await;
                                    (
                                        alloc.downloaded_bytes(piece_length, total_torrent_size),
                                        alloc.remaining_bytes(piece_length, total_torrent_size),
                                    )
                                };
                                let pause_trackers = trackers.clone();
                                let pause_port = self.listen_port;
                                tokio::spawn(async move {
                                    let _ = Self::announce_to_trackers(
                                        &pause_trackers,
                                        &info_hash,
                                        session_peer_id,
                                        pause_port,
                                        dl,
                                        ul,
                                        rem,
                                        Some(TrackerEvents::Stopped),
                                        None,
                                    )
                                    .await;
                                });
                            }
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
                            info!(target: "major", "Torrent resumed: {}", info_hash_sha1);
                            is_paused = false;
                            is_stalled = false;
                            zero_speed_since = None;
                            let mut state = torrent_state.lock().await;
                            state.paused = 0;
                            let _ = TorrentState::save_state(state.clone()).await;
                            drop(state);

                            // Ensure any stale Downloading piece states are reset to Missing on resume
                            piece_allocator.lock().await.reset_in_progress();

                            let all_done = piece_allocator.lock().await.all_complete();
                            if all_done {
                                is_active_for_announce.store(true, std::sync::atomic::Ordering::Relaxed);
                                self.stats_registry.set_status(&info_hash_sha1, Some("seeding".to_string()));
                            } else {
                                let (active, rx) = self.queue_manager.acquire_download_slot(info_hash).await;
                                slot_rx = rx;
                                if active {
                                    has_download_slot = true;
                                    is_active_for_announce.store(true, std::sync::atomic::Ordering::Relaxed);
                                    self.stats_registry.set_status(&info_hash_sha1, None);
                                } else {
                                    has_download_slot = false;
                                    is_active_for_announce.store(false, std::sync::atomic::Ordering::Relaxed);
                                    self.stats_registry.set_status(&info_hash_sha1, Some("queued".to_string()));
                                }
                            }

                            let can_run = has_download_slot || all_done;
                            if can_run {
                                while let Some(peer) = peer_pool.pop_front() {
                                    match Self::spawn_worker_if_new(
                                        &mut worker_set,
                                        &connected_peers,
                                        peer.clone(),
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
                                        new_peer_tx.clone(),
                                    )
                                    .await
                                    {
                                        SpawnPeerOutcome::Spawned | SpawnPeerOutcome::AlreadyConnected => {}
                                        SpawnPeerOutcome::AtCapacity => {
                                            peer_pool.push_front(peer);
                                            break;
                                        }
                                    }
                                }

                                if worker_set.len() < MAX_CONNECTED_PEERS_PER_TORRENT / 2 && !trackers.is_empty() {
                                    let (dl, rem) = {
                                        let alloc = piece_allocator.lock().await;
                                        (
                                            alloc.downloaded_bytes(piece_length, total_torrent_size),
                                            alloc.remaining_bytes(piece_length, total_torrent_size),
                                        )
                                    };
                                    let resume_trackers = trackers.clone();
                                    let resume_stats = Arc::clone(&self.stats_registry);
                                    let resume_hash_sha1 = info_hash_sha1.clone();
                                    let resume_peer_tx = new_peer_tx.clone();
                                    let resume_port = self.listen_port;
                                    tokio::spawn(async move {
                                        Self::announce_to_trackers_stream(
                                            &resume_trackers,
                                            &info_hash,
                                            session_peer_id,
                                            resume_port,
                                            dl,
                                            0,
                                            rem,
                                            None,
                                            Some((&resume_stats, &resume_hash_sha1)),
                                            &resume_peer_tx,
                                        )
                                        .await;
                                    });
                                }
                            }
                        }
                        TorrentCommand::Recheck => {
                            info!(target: "major", "Force recheck initiated for: {}", info_hash_sha1);
                            worker_set.shutdown().await;
                            piece_allocator.lock().await.reset_in_progress();
                            connected_peers.lock().await.clear();
                            self.stats_registry.clear_peers(&info_hash_sha1);
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
                                state.total_downloaded = recheck.completed_bytes;
                                let _ = TorrentState::save_state(state.clone()).await;
                            }
                            {
                                let mut alloc = piece_allocator.lock().await;
                                let seq = alloc.sequential;
                                *alloc = PieceAllocator::from_hotstate_bitfield(&recheck.bitfield, total_pieces);
                                alloc.sequential = seq;
                            }
                            self.stats_registry.set_initial_bitfield(
                                &info_hash_sha1,
                                &recheck.bitfield,
                                recheck.completed_pieces,
                                recheck.completed_bytes,
                            );

                            if recheck.completed_pieces == total_pieces {
                                if has_download_slot {
                                    has_download_slot = false;
                                    self.queue_manager.release_download_slot(&info_hash).await;
                                }
                                is_active_for_announce.store(true, std::sync::atomic::Ordering::Relaxed);
                                self.stats_registry.set_status(&info_hash_sha1, Some("seeding".to_string()));
                            } else if is_paused {
                                self.stats_registry.set_status(&info_hash_sha1, Some("paused".to_string()));
                            } else if has_download_slot {
                                self.stats_registry.set_status(&info_hash_sha1, None);
                            } else {
                                self.stats_registry.set_status(&info_hash_sha1, Some("queued".to_string()));
                            }

                            let can_run = has_download_slot || recheck.completed_pieces == total_pieces;
                            if !is_paused && can_run {
                                while let Some(peer) = peer_pool.pop_front() {
                                    match Self::spawn_worker_if_new(
                                        &mut worker_set,
                                        &connected_peers,
                                        peer.clone(),
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
                                        new_peer_tx.clone(),
                                    )
                                    .await
                                    {
                                        SpawnPeerOutcome::Spawned | SpawnPeerOutcome::AlreadyConnected => {}
                                        SpawnPeerOutcome::AtCapacity => {
                                            peer_pool.push_front(peer);
                                            break;
                                        }
                                    }
                                }

                                if peer_pool.is_empty() && worker_set.is_empty() && !trackers.is_empty() {
                                    let (dl, rem) = {
                                        let alloc = piece_allocator.lock().await;
                                        (
                                            alloc.downloaded_bytes(piece_length, total_torrent_size),
                                            alloc.remaining_bytes(piece_length, total_torrent_size),
                                        )
                                    };
                                    let resume_trackers = trackers.clone();
                                    let resume_stats = Arc::clone(&self.stats_registry);
                                    let resume_hash_sha1 = info_hash_sha1.clone();
                                    let resume_peer_tx = new_peer_tx.clone();
                                    let resume_port = self.listen_port;
                                    tokio::spawn(async move {
                                        Self::announce_to_trackers_stream(
                                            &resume_trackers,
                                            &info_hash,
                                            session_peer_id,
                                            resume_port,
                                            dl,
                                            0,
                                            rem,
                                            None,
                                            Some((&resume_stats, &resume_hash_sha1)),
                                            &resume_peer_tx,
                                        )
                                        .await;
                                    });
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
                            let uploaded = self.stats_registry
                                .snapshot_torrent(&info_hash_sha1)
                                .map(|s| s.uploaded_bytes)
                                .unwrap_or(0);

                            if !trackers.is_empty() && (has_download_slot || piece_allocator.lock().await.all_complete()) {
                                let re_trackers = trackers.clone();
                                let re_stats = Arc::clone(&self.stats_registry);
                                let re_hash_sha1 = info_hash_sha1.clone();
                                let re_peer_tx = new_peer_tx.clone();
                                let re_port = self.listen_port;
                                tokio::spawn(async move {
                                    Self::announce_to_trackers_stream(
                                        &re_trackers,
                                        &info_hash,
                                        session_peer_id,
                                        re_port,
                                        downloaded,
                                        uploaded,
                                        remaining,
                                        None,
                                        Some((&re_stats, &re_hash_sha1)),
                                        &re_peer_tx,
                                    )
                                    .await;
                                });
                            }
                        }
                    }
                }
                // Handle queue promotion to active downloading
                Some(()) = slot_rx.recv(), if !has_download_slot && !is_paused => {
                    info!("Torrent {} promoted from queue to active downloading", info_hash_sha1);
                    has_download_slot = true;
                    is_stalled = false;
                    zero_speed_since = None;
                    is_active_for_announce.store(true, std::sync::atomic::Ordering::Relaxed);
                    self.stats_registry.set_status(&info_hash_sha1, None);

                    let (dl, rem) = {
                        let alloc = piece_allocator.lock().await;
                        (
                            alloc.downloaded_bytes(piece_length, total_torrent_size),
                            alloc.remaining_bytes(piece_length, total_torrent_size),
                        )
                    };

                    while let Some(peer) = peer_pool.pop_front() {
                        match Self::spawn_worker_if_new(
                            &mut worker_set,
                            &connected_peers,
                            peer.clone(),
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
                            new_peer_tx.clone(),
                        )
                        .await
                        {
                            SpawnPeerOutcome::Spawned | SpawnPeerOutcome::AlreadyConnected => {}
                            SpawnPeerOutcome::AtCapacity => {
                                peer_pool.push_front(peer);
                                break;
                            }
                        }
                    }

                    if !trackers.is_empty() {
                        let resume_trackers = trackers.clone();
                        let resume_stats = Arc::clone(&self.stats_registry);
                        let resume_hash_sha1 = info_hash_sha1.clone();
                        let resume_peer_tx = new_peer_tx.clone();
                        let resume_port = self.listen_port;
                        tokio::spawn(async move {
                            Self::announce_to_trackers_stream(
                                &resume_trackers,
                                &info_hash,
                                session_peer_id,
                                resume_port,
                                dl,
                                0,
                                rem,
                                Some(TrackerEvents::Started),
                                Some((&resume_stats, &resume_hash_sha1)),
                                &resume_peer_tx,
                            )
                            .await;
                        });
                    }
                }
                // Handle inbound connections from listening port
                Some(inbound) = inbound_rx.recv() => {
                    let can_connect = has_download_slot || piece_allocator.lock().await.all_complete();
                    if !is_paused && can_connect {
                        Self::spawn_inbound_worker(
                            &mut worker_set,
                            &connected_peers,
                            inbound,
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
                            new_peer_tx.clone(),
                        )
                        .await;
                    }
                }
                // Handle newly discovered peers from re-announce
                Some(new_peers) = new_peer_rx.recv() => {
                    {
                        let now = std::time::Instant::now();
                        failed_peers.retain(|_, ts| now.duration_since(*ts) < FAILED_PEER_COOLDOWN);
                        let connected = connected_peers.lock().await;
                        for peer in new_peers {
                            if peer_pool.len() >= MAX_PEER_POOL_SIZE {
                                break;
                            }
                            let key = (peer.ipv4_addr, peer.port);
                            if !connected.contains(&key)
                                && !failed_peers.contains_key(&key)
                                && !peer_pool.iter().any(|p| p.ipv4_addr == peer.ipv4_addr && p.port == peer.port)
                            {
                                peer_pool.push_back(peer);
                            }
                        }
                    }

                    let can_connect = has_download_slot || piece_allocator.lock().await.all_complete();
                    if !is_paused && can_connect {
                        while let Some(peer) = peer_pool.pop_front() {
                            match Self::spawn_worker_if_new(
                                &mut worker_set,
                                &connected_peers,
                                peer.clone(),
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
                                new_peer_tx.clone(),
                            )
                            .await
                            {
                                SpawnPeerOutcome::Spawned | SpawnPeerOutcome::AlreadyConnected => {}
                                SpawnPeerOutcome::AtCapacity => {
                                    peer_pool.push_front(peer);
                                    break;
                                }
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
                                if failed_peers.len() >= MAX_FAILED_PEERS_CACHE {
                                    let now = std::time::Instant::now();
                                    failed_peers.retain(|_, ts| now.duration_since(*ts) < FAILED_PEER_COOLDOWN);
                                }
                                if failed_peers.len() < MAX_FAILED_PEERS_CACHE {
                                    failed_peers.insert((ip, port), std::time::Instant::now());
                                }
                            }

                            let can_connect = has_download_slot || piece_allocator.lock().await.all_complete();
                            if !is_paused && can_connect {
                                while let Some(next_peer) = peer_pool.pop_front() {
                                    match Self::spawn_worker_if_new(
                                        &mut worker_set,
                                        &connected_peers,
                                        next_peer.clone(),
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
                                        new_peer_tx.clone(),
                                    )
                                    .await
                                    {
                                        SpawnPeerOutcome::Spawned | SpawnPeerOutcome::AlreadyConnected => {}
                                        SpawnPeerOutcome::AtCapacity => {
                                            peer_pool.push_front(next_peer);
                                            break;
                                        }
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
                        if has_download_slot {
                            has_download_slot = false;
                            is_stalled = false;
                            zero_speed_since = None;
                            self.queue_manager.release_download_slot(&info_hash).await;
                        }
                        self.stats_registry.set_initial_progress(&info_hash_sha1, total_pieces, total_torrent_size);
                        let current_status = self.stats_registry.snapshot_torrent(&info_hash_sha1).and_then(|s| s.custom_status);
                        if current_status.as_deref() != Some("seeding") {
                            info!(
                                target: "major",
                                "Torrent download complete — now seeding ({})",
                                info_hash_sha1
                            );
                            self.stats_registry.set_status(&info_hash_sha1, Some("seeding".to_string()));
                        }
                    }
                }
            }
        }

        is_active_for_announce.store(false, std::sync::atomic::Ordering::Relaxed);
        re_announce_handle.abort();
        self.inbound_registry.unregister(&info_hash).await;
        self.control_registry.unregister(&info_hash_sha1);
        self.queue_manager.release_download_slot(&info_hash).await;

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
            self.listen_port,
            final_downloaded,
            0,
            final_remaining,
            final_event,
            Some((&self.stats_registry, &info_hash_sha1)),
        )
        .await;

        Ok(())
    }

    // Contact all trackers concurrently and collect peers.
    async fn announce_to_trackers(
        trackers: &[String],
        info_hash: &[u8; 20],
        peer_id: [u8; 20],
        port: u16,
        downloaded: u64,
        uploaded: u64,
        remaining_size: u64,
        event: Option<TrackerEvents>,
        stats: Option<(&Arc<StatsRegistry>, &str)>,
    ) -> Vec<Peer> {
        let (peers_tx, mut peers_rx) = mpsc::channel::<Vec<Peer>>(trackers.len().max(1));

        Self::announce_to_trackers_stream(
            trackers,
            info_hash,
            peer_id,
            port,
            downloaded,
            uploaded,
            remaining_size,
            event,
            stats,
            &peers_tx,
        )
        .await;

        drop(peers_tx);

        let mut all_peers = Vec::new();
        while let Some(peers) = peers_rx.recv().await {
            all_peers.extend(peers);
        }

        all_peers
    }

    // Announce to all trackers concurrently in parallel, streaming discovered peers as each responds.
    async fn announce_to_trackers_stream(
        trackers: &[String],
        info_hash: &[u8; 20],
        peer_id: [u8; 20],
        port: u16,
        downloaded: u64,
        uploaded: u64,
        remaining_size: u64,
        event: Option<TrackerEvents>,
        stats: Option<(&Arc<StatsRegistry>, &str)>,
        peer_tx: &mpsc::Sender<Vec<Peer>>,
    ) {
        if trackers.is_empty() {
            return;
        }

        // 1. Immediately mark ALL trackers as "Updating..." in stats registry.
        // This ensures the Web-UI shows all trackers updating in parallel rather than inactive.
        if let Some((reg, hash)) = stats {
            for announce_url in trackers {
                reg.update_tracker(hash, announce_url, "Updating...", None, None, None);
            }
        }

        // 2. Launch concurrent announce tasks for all trackers with per-torrent bounded concurrency
        // so one torrent's long tracker list never blocks other torrents from announcing immediately.
        let mut join_set = JoinSet::new();
        let sem = Arc::new(Semaphore::new(MAX_CONCURRENT_TRACKER_ANNOUNCES_PER_TORRENT));

        for announce_url in trackers {
            let announce_url = announce_url.clone();
            let info_hash = *info_hash;
            let stats_opt = stats.map(|(reg, hash)| (Arc::clone(reg), hash.to_string()));
            let peer_tx = peer_tx.clone();
            let sem = Arc::clone(&sem);

            join_set.spawn(async move {
                let _permit = sem.acquire_owned().await.ok();
                let tracker = Tracker {
                    announce_url: announce_url.clone(),
                    downloaded: downloaded as i64,
                    uploaded: uploaded as i64,
                    remaining_size: remaining_size as i64,
                    info_hash,
                    peer_id,
                    port: port.to_string(),
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

                        if let Some((reg, hash)) = stats_opt.as_ref() {
                            reg.update_tracker(
                                hash,
                                &announce_url,
                                "Working",
                                Some(peer_count),
                                Some(interval),
                                Some(next_announce),
                            );
                        }
                        info!(
                            "found : {peer_count} peers from tracker -> {} (next announce in {}s)",
                            announce_url, interval
                        );
                        if !tracker_response.ipv4_peers.is_empty() {
                            let _ = peer_tx.send(tracker_response.ipv4_peers).await;
                        }
                    }
                    Err(e) => {
                        let err_msg = format!("Error: {}", e);
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        if let Some((reg, hash)) = stats_opt.as_ref() {
                            reg.update_tracker(
                                hash,
                                &announce_url,
                                &err_msg,
                                Some(0),
                                None,
                                Some(now + 300),
                            );
                        }
                        warn!("ERROR -> {} on tracker: {}", e, announce_url);
                    }
                }
            });
        }

        while join_set.join_next().await.is_some() {}
    }

    // Spawn a worker for an inbound connection accepted on our peer listening port.
    async fn spawn_inbound_worker(
        worker_set: &mut JoinSet<(Ipv4Addr, u16, Result<(), EngineError>)>,
        connected_peers: &Arc<Mutex<HashSet<(Ipv4Addr, u16)>>>,
        inbound: InboundConnection,
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
        peer_tx: mpsc::Sender<Vec<Peer>>,
    ) -> bool {
        let key = (inbound.peer_ip, inbound.peer_port);
        {
            let mut connected = connected_peers.lock().await;
            if connected.len() >= MAX_CONNECTED_PEERS_PER_TORRENT {
                return false;
            }
            if !connected.insert(key) {
                return false;
            }
        }

        let piece_allocator = Arc::clone(piece_allocator);
        let torrent_state = Arc::clone(torrent_state);
        let file_map = Arc::clone(file_map);
        let stats_registry = Arc::clone(stats_registry);
        let piece_hashes = Arc::clone(piece_hashes);
        let peer_ip = inbound.peer_ip;
        let peer_port = inbound.peer_port;
        let have_rx = have_tx.subscribe();
        let stream = inbound.stream;
        let remote_peer_id = inbound.remote_peer_id;
        let supports_extensions = inbound.supports_extensions;
        let connected_peers_clone = Arc::clone(connected_peers);

        worker_set.spawn(async move {
            let worker = PeerWorker {
                ip_addr: IpAddr::from(peer_ip),
                port: peer_port,
                info_hash,
                peer_id: session_peer_id,
                peer_state: PeerState {
                    am_choking: false,
                    am_interested: false,
                    peer_choking: true,
                    peer_interested: false,
                    bitfield: vec![0u8; (total_pieces + 7) / 8],
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
                supports_extensions,
                remote_ut_pex_id: None,
                connected_peers: connected_peers_clone,
                peer_tx,
                last_sent_pex_peers: HashSet::new(),
                active_piece_guard: Arc::new(std::sync::atomic::AtomicU32::new(u32::MAX)),
                last_block_time: None,
                last_stalled_piece: None,
            };

            let result = worker.start_inbound(stream, remote_peer_id).await;
            if let Err(ref e) = result {
                warn!(
                    "Inbound peer: {}:{}, error: {}",
                    peer_ip,
                    peer_port,
                    e.to_string()
                );
            }

            (peer_ip, peer_port, result)
        });

        true
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
        peer_tx: mpsc::Sender<Vec<Peer>>,
    ) -> SpawnPeerOutcome {
        let key = (peer.ipv4_addr, peer.port);
        {
            let mut connected = connected_peers.lock().await;
            if connected.contains(&key) {
                return SpawnPeerOutcome::AlreadyConnected;
            }
            if connected.len() >= MAX_CONNECTED_PEERS_PER_TORRENT {
                return SpawnPeerOutcome::AtCapacity;
            }
            connected.insert(key);
        }

        let piece_allocator = Arc::clone(piece_allocator);
        let torrent_state = Arc::clone(torrent_state);
        let file_map = Arc::clone(file_map);
        let stats_registry = Arc::clone(stats_registry);
        let piece_hashes = Arc::clone(piece_hashes);
        let peer_ip = peer.ipv4_addr;
        let peer_port = peer.port;
        let have_rx = have_tx.subscribe();
        let connected_peers_clone = Arc::clone(connected_peers);

        worker_set.spawn(async move {
            let worker = PeerWorker {
                ip_addr: IpAddr::from(peer.ipv4_addr),
                port: peer.port,
                info_hash,
                peer_id: session_peer_id,
                peer_state: PeerState {
                    am_choking: false,
                    am_interested: false,
                    peer_choking: true,
                    peer_interested: false,
                    bitfield: vec![0u8; (total_pieces + 7) / 8],
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
                supports_extensions: false, // will be discovered in handshake
                remote_ut_pex_id: None,
                connected_peers: connected_peers_clone,
                peer_tx,
                last_sent_pex_peers: HashSet::new(),
                active_piece_guard: Arc::new(std::sync::atomic::AtomicU32::new(u32::MAX)),
                last_block_time: None,
                last_stalled_piece: None,
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

        SpawnPeerOutcome::Spawned
    }
}

