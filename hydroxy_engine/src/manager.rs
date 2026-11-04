use anyhow::Result;
use hydroxy_config::HydroxyConfig;
use hydroxy_storage_io::TorrentDirData;
use hydroxy_utils::torrent_info::TorrentInfo;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::{
    control::TorrentControlRegistry, peer_listener::InboundTorrentRegistry,
    queue::DownloadQueueManager, stats::stats_registry::StatsRegistry, torrent_task::TorrentTask,
};

pub struct Manager {}

impl Manager {
    pub async fn start(
        stats_registry: Arc<StatsRegistry>,
        control_registry: Arc<TorrentControlRegistry>,
        inbound_registry: Arc<InboundTorrentRegistry>,
        queue_manager: Arc<DownloadQueueManager>,
        listen_port: u16,
    ) -> Result<()> {
        info!(target: "major", "Hydroxy Engine Started...");

        let active_torrents = Arc::new(Mutex::new(HashSet::<[u8; 20]>::new()));
        // Cache (modified_time, info_hash) by path so unchanged .torrent files already
        // running in active_torrents are not read into memory and re-parsed every minute.
        let mut known_files: HashMap<PathBuf, (Option<SystemTime>, [u8; 20])> = HashMap::new();

        // Check watch directory periodically every minute
        let mut scan_interval = tokio::time::interval(Duration::from_secs(60));
        let shutdown_notify = control_registry.shutdown_notify();

        loop {
            tokio::select! {
                _ = scan_interval.tick() => {},
                _ = shutdown_notify.notified() => {
                    info!(target: "major", "Watch directory scanner stopped for shutdown");
                    break;
                }
            }

            if control_registry.is_shutting_down() {
                info!(target: "major", "Watch directory scanner stopped for shutdown");
                break;
            }

            let config = match HydroxyConfig::init() {
                Ok(cfg) => cfg,
                Err(e) => {
                    warn!("Failed to load config in watchdir scanner: {}", e);
                    continue;
                }
            };

            queue_manager
                .set_max_active(config.effective_max_active_downloads())
                .await;

            let mut seen_paths: HashSet<PathBuf> = HashSet::new();

            for storage in &config.storage_config {
                let watch_dir = if std::fs::create_dir_all(&storage.watch_directory).is_ok() {
                    storage.watch_directory.clone()
                } else {
                    let local = format!("dottorrents_{}", storage.name);
                    let _ = std::fs::create_dir_all(&local);
                    local
                };

                let download_dir = if std::fs::create_dir_all(&storage.download_directory).is_ok() {
                    storage.download_directory.clone()
                } else {
                    let local = format!("downloads_{}", storage.name);
                    let _ = std::fs::create_dir_all(&local);
                    local
                };

                let max_depth = if storage.watch_directory_max_depth == 0 {
                    3
                } else {
                    storage.watch_directory_max_depth
                };

                let torrents_dir_paths =
                    match TorrentDirData::scan_paths(&watch_dir, &download_dir, max_depth).await {
                        Ok(data) => data,
                        Err(e) => {
                            warn!("Failed to scan watch directory '{}' [storage: {}]: {}", watch_dir, storage.name, e);
                            continue;
                        }
                    };

                let temp_base_dir: Option<String> = storage
                    .temp_directory
                    .as_ref()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());

                for torrent_dir in torrents_dir_paths {
                    for entry in torrent_dir.torrent_entries {
                        seen_paths.insert(entry.path.clone());

                        if let Some((cached_mtime, cached_hash)) = known_files.get(&entry.path) {
                            if *cached_mtime == entry.modified
                                && active_torrents.lock().await.contains(cached_hash)
                            {
                                continue;
                            }
                        }

                        let raw_bytes = match tokio::fs::read(&entry.path).await {
                            Ok(b) => b,
                            Err(e) => {
                                warn!("Failed to read torrent file {:?}: {}", entry.path, e);
                                continue;
                            }
                        };

                        let final_dir = if torrent_dir.dir_name.is_empty() || torrent_dir.dir_name == "." {
                            download_dir.clone()
                        } else {
                            format!("{}/{}", &download_dir, &torrent_dir.dir_name)
                        };

                        let temp_dir = temp_base_dir.as_ref().map(|base| {
                            if torrent_dir.dir_name.is_empty() || torrent_dir.dir_name == "." {
                                base.clone()
                            } else {
                                format!("{}/{}", base, &torrent_dir.dir_name)
                            }
                        });

                        let torrent_info = match TorrentInfo::parse_torrent_file(raw_bytes) {
                            Ok(info) => info,
                            Err(e) => {
                                warn!("Failed to parse torrent file in {}: {}", torrent_dir.dir_name, e);
                                continue;
                            }
                        };

                        let info_hash = torrent_info.other_infos.info_hash;
                        known_files.insert(entry.path, (entry.modified, info_hash));

                        let mut active = active_torrents.lock().await;
                        if active.contains(&info_hash) {
                            continue; // Already has a running TorrentTask
                        }
                        active.insert(info_hash);
                        drop(active);

                        let info_hash_sha1 = hydroxy_utils::torrent_info::OtherInfos::to_sha1(&info_hash);
                        info!(
                            target: "major",
                            "Torrent found in watch dir: '{}' ({}) [storage: '{}', category: '{}']",
                            torrent_info.info.name,
                            info_hash_sha1,
                            storage.name,
                            if torrent_dir.dir_name.is_empty() { "default" } else { &torrent_dir.dir_name }
                        );

                        let category = torrent_dir.dir_name.clone();

                        let torrent_task = TorrentTask {
                            info_hash,
                            storage: storage.name.clone(),
                            final_dir,
                            temp_dir,
                            category,
                            stats_registry: stats_registry.clone(),
                            control_registry: control_registry.clone(),
                            listen_port,
                            inbound_registry: inbound_registry.clone(),
                            queue_manager: queue_manager.clone(),
                        };

                        let active_torrents_ref = active_torrents.clone();
                        // Spawning a torrent task for each newly detected torrent
                        tokio::spawn(async move {
                            let res = torrent_task.start(torrent_info).await;
                            if let Err(e) = res {
                                warn!(target: "major", "Torrent task failed: {}", e);
                            }
                            active_torrents_ref.lock().await.remove(&info_hash);
                        });
                    }
                }
            }

            known_files.retain(|path, _| seen_paths.contains(path));
        }

        Ok(())
    }
}
