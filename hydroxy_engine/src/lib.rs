use anyhow::Result;
use hydroxy_utils::get_peer_id;
use std::sync::Arc;
use tracing::info;

use crate::manager::Manager;
use crate::stats::stats_registry::StatsRegistry;

pub mod control;
mod errors;
mod manager;
mod peer_worker;
mod piece_allocator;
mod piece_assembler;
pub mod piece_verifier;
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

        Manager::start(stats_registry, control_registry).await?;

        Ok(())
    }
}
