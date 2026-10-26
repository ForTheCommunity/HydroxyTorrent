use anyhow::Result;
use hydroxy_config::HydroxyConfig;
use hydroxy_utils::get_peer_id;
use std::sync::Arc;
use tracing::{info, warn};

use crate::manager::Manager;
use crate::peer_listener::{InboundTorrentRegistry, PeerListener};
use crate::queue::DownloadQueueManager;
use crate::stats::stats_registry::StatsRegistry;

pub mod control;
mod errors;
mod manager;
pub mod peer_listener;
mod peer_worker;
mod piece_allocator;
mod piece_assembler;
pub mod piece_verifier;
pub mod queue;
pub mod remote;
pub mod stats;
mod torrent_task;

use crate::control::TorrentControlRegistry;

pub struct HydroxyEngine {}

impl HydroxyEngine {
    pub async fn start(
        stats_registry: Arc<StatsRegistry>,
        control_registry: Arc<TorrentControlRegistry>,
    ) -> Result<()> {
        let peer_id = get_peer_id();
        info!("Generated Random Peer ID -> {}", str::from_utf8(&peer_id)?);

        let config = match HydroxyConfig::init() {
            Ok(cfg) => cfg,
            Err(e) => {
                warn!("Configuration load warning in HydroxyEngine: {}", e);
                return Err(anyhow::anyhow!("Failed to initialize config: {}", e));
            }
        };

        let listen_address = config.network_config.listen_address;
        let listen_port = config.network_config.listen_port;

        let inbound_registry = Arc::new(InboundTorrentRegistry::new());
        if let Err(e) = PeerListener::start(
            listen_address,
            listen_port,
            Arc::clone(&inbound_registry),
        )
        .await
        {
            warn!(
                "Failed to bind peer incoming listener on {}:{}: {}",
                listen_address, listen_port, e
            );
        }

        let queue_manager = Arc::new(DownloadQueueManager::new(
            config.effective_max_active_downloads(),
        ));

        Manager::start(
            stats_registry,
            control_registry,
            inbound_registry,
            queue_manager,
            listen_port,
        )
        .await?;

        Ok(())
    }
}
