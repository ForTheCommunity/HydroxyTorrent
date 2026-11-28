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
pub mod fast_ext;
pub mod log_broadcast;
mod manager;
pub mod peer_listener;
mod peer_worker;
pub mod pex;
mod piece_allocator;
mod piece_assembler;
pub mod piece_verifier;
pub mod queue;
pub mod remote;
pub mod stats;
mod torrent_task;
pub mod webseed;

use crate::control::TorrentControlRegistry;

pub struct HydroxyEngine {}

impl HydroxyEngine {
    pub async fn start(
        stats_registry: Arc<StatsRegistry>,
        control_registry: Arc<TorrentControlRegistry>,
    ) -> Result<()> {
        let peer_id = get_peer_id();
        info!(target: "minor", "Generated Random Peer ID -> {}", str::from_utf8(&peer_id)?);

        let config = match HydroxyConfig::init() {
            Ok(cfg) => cfg,
            Err(e) => {
                warn!(target: "major", "Configuration load warning in HydroxyEngine: {}", e);
                return Err(anyhow::anyhow!("Failed to initialize config: {}", e));
            }
        };

        let listen_address = config.network_config.listen_address;
        let listen_port = config.network_config.listen_port;

        let inbound_registry = Arc::new(InboundTorrentRegistry::new());
        if let Err(e) =
            PeerListener::start(listen_address, listen_port, Arc::clone(&inbound_registry)).await
        {
            warn!(
                target: "major",
                "Failed to bind peer incoming listener on {}:{}: {}",
                listen_address, listen_port, e
            );
        }

        let queue_manager = Arc::new(DownloadQueueManager::with_config(
            config.effective_max_active_downloads(),
            config.limitations.dont_count_slow_torrents,
            config.limitations.slow_dl_rate_threshold_bytes(),
            std::time::Duration::from_secs(config.limitations.slow_torrent_inactive_timer_secs),
        ));

        let dht_handle = if config.network_config.dht_enabled {
            let dht_port = config.network_config.dht_port;
            let (bind_v4_ip, bind_addr_v6) = match listen_address {
                std::net::IpAddr::V4(v4) => (v4, None),
                std::net::IpAddr::V6(v6) if v6.is_unspecified() => {
                    (std::net::Ipv4Addr::UNSPECIFIED, None)
                }
                std::net::IpAddr::V6(v6) => (
                    std::net::Ipv4Addr::UNSPECIFIED,
                    Some(std::net::SocketAddrV6::new(v6, dht_port, 0, 0)),
                ),
            };

            let bootstrap_routers = if config.network_config.dht_bootstrap_routers.is_empty() {
                hydroxy_dht::DEFAULT_BOOTSTRAP_ROUTERS
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            } else {
                config.network_config.dht_bootstrap_routers.clone()
            };

            let dht_cfg = hydroxy_dht::DHTConfig {
                bind_addr: std::net::SocketAddrV4::new(bind_v4_ip, dht_port),
                bind_addr_v6,
                enable_ipv6: listen_address.is_unspecified() || listen_address.is_ipv6(),
                bootstrap_routers,
                ..Default::default()
            };

            match hydroxy_dht::DHTNode::start(dht_cfg).await {
                Ok((node, handle)) => {
                    let actual_port = handle.local_addr().map(|a| a.port()).unwrap_or(dht_port);
                    stats_registry.set_dht_stats(true, actual_port, 0, 0);

                    let dht_stats_reg = Arc::clone(&stats_registry);
                    let dht_stats_handle = handle.clone();
                    let shutdown_notify = control_registry.shutdown_notify();
                    tokio::spawn(async move {
                        let mut interval =
                            tokio::time::interval(std::time::Duration::from_secs(1));
                        loop {
                            tokio::select! {
                                _ = shutdown_notify.notified() => {
                                    node.shutdown();
                                    break;
                                }
                                _ = interval.tick() => {
                                    let (total, good) = dht_stats_handle.routing_table_size().await;
                                    dht_stats_reg.set_dht_stats(true, actual_port, total, good);
                                }
                            }
                        }
                    });
                    info!(
                        target: "major",
                        "DHT node started on UDP port {}",
                        actual_port
                    );
                    Some(handle)
                }
                Err(e) => {
                    stats_registry.set_dht_stats(false, 0, 0, 0);
                    warn!(
                        target: "major",
                        "Failed to start DHT node on UDP port {}: {}",
                        dht_port, e
                    );
                    None
                }
            }
        } else {
            stats_registry.set_dht_stats(false, 0, 0, 0);
            info!(target: "major", "DHT trackerless peer discovery disabled by configuration");
            None
        };

        Manager::start(
            stats_registry,
            control_registry,
            inbound_registry,
            queue_manager,
            listen_port,
            dht_handle,
        )
        .await?;

        Ok(())
    }
}
