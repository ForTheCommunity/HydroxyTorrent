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
        info!("Hydroxy Engine Started...");

        let active_torrents = Arc::new(Mutex::new(HashSet::<[u8; 20]>::new()));
        // Cache (modified_time, info_hash) by path so unchanged .torrent files already
        // running in active_torrents are not read into memory and re-parsed every minute.
        let mut known_files: HashMap<PathBuf, (Option<SystemTime>, [u8; 20])> = HashMap::new();

        // Check watch directory periodically every minute
        let mut scan_interval = tokio::time::interval(Duration::from_secs(60));

        loop {
            scan_interval.tick().await;

            let config = match HydroxyConfig::init() {
                Ok(cfg) => cfg,
                Err(e) => {
                    warn!("Failed to load config in watchdir scanner: {}", e);
                    continue;
                }
            };

            let watch_dir = if std::fs::create_dir_all(&config.storage_config.watch_directory).is_ok() {
                config.storage_config.watch_directory.clone()
            } else {
                let local = "dottorrents".to_string();
                let _ = std::fs::create_dir_all(&local);
                local
            };

            let download_dir = if std::fs::create_dir_all(&config.storage_config.download_directory).is_ok() {
                config.storage_config.download_directory.clone()
            } else {
                let local = "downloads".to_string();
                let _ = std::fs::create_dir_all(&local);
                local
            };

            let max_depth = if config.storage_config.watch_directory_max_depth == 0 {
                3
            } else {
                config.storage_config.watch_directory_max_depth
            };

            let torrents_dir_paths =
                match TorrentDirData::scan_paths(&watch_dir, &download_dir, max_depth).await {
                    Ok(data) => data,
                    Err(e) => {
                        warn!("Failed to scan watch directory '{}': {}", watch_dir, e);
                        continue;
                    }
                };

            queue_manager
                .set_max_active(config.effective_max_active_downloads())
                .await;

            let mut seen_paths: HashSet<PathBuf> = HashSet::new();

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

                    let root_dir = if torrent_dir.dir_name.is_empty() || torrent_dir.dir_name == "." {
                        download_dir.clone()
                    } else {
                        format!("{}/{}", &download_dir, &torrent_dir.dir_name)
                    };

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
                        "New torrent detected in watch dir: '{}' ({})",
                        torrent_info.info.name,
                        info_hash_sha1
                    );

                    let category = torrent_dir.dir_name.clone();

                    let torrent_task = TorrentTask {
                        info_hash,
                        root_dir,
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
                            warn!("Torrent task failed: {}", e);
                        }
                        active_torrents_ref.lock().await.remove(&info_hash);
                    });
                }
            }

            known_files.retain(|path, _| seen_paths.contains(path));
        }
    }
}
