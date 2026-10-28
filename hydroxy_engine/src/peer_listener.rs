use std::{collections::HashMap, net::Ipv4Addr, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, Mutex, Semaphore},
};
use tracing::{info, warn};
use hydroxy_peer_wire_proto::handshake::Handshake;

const MAX_CONCURRENT_INBOUND_HANDSHAKES: usize = 64;

pub struct InboundConnection {
    pub stream: TcpStream,
    pub peer_ip: Ipv4Addr,
    pub peer_port: u16,
    pub remote_peer_id: [u8; 20],
    pub supports_extensions: bool,
}

pub struct InboundTorrentRegistration {
    pub session_peer_id: [u8; 20],
    pub sender: mpsc::Sender<InboundConnection>,
}

#[derive(Clone, Default)]
pub struct InboundTorrentRegistry {
    torrents: Arc<Mutex<HashMap<[u8; 20], InboundTorrentRegistration>>>,
}

impl InboundTorrentRegistry {
    pub fn new() -> Self {
        Self {
            torrents: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn register(
        &self,
        info_hash: [u8; 20],
        session_peer_id: [u8; 20],
        sender: mpsc::Sender<InboundConnection>,
    ) {
        self.torrents.lock().await.insert(
            info_hash,
            InboundTorrentRegistration {
                session_peer_id,
                sender,
            },
        );
    }

    pub async fn unregister(&self, info_hash: &[u8; 20]) {
        self.torrents.lock().await.remove(info_hash);
    }

    pub async fn get(&self, info_hash: &[u8; 20]) -> Option<InboundTorrentRegistration> {
        let lock = self.torrents.lock().await;
        lock.get(info_hash).map(|r| InboundTorrentRegistration {
            session_peer_id: r.session_peer_id,
            sender: r.sender.clone(),
        })
    }
}

pub struct PeerListener;

impl PeerListener {
    pub async fn start(
        listen_address: std::net::IpAddr,
        listen_port: u16,
        registry: Arc<InboundTorrentRegistry>,
    ) -> Result<(), std::io::Error> {
        let bind_addr = std::net::SocketAddr::new(listen_address, listen_port);
        let listener = TcpListener::bind(bind_addr).await?;
        info!(target: "major", "Peer inbound listener active on {}", bind_addr);

        tokio::spawn(async move {
            let handshake_limiter = Arc::new(Semaphore::new(MAX_CONCURRENT_INBOUND_HANDSHAKES));
            loop {
                match listener.accept().await {
                    Ok((mut stream, socket_addr)) => {
                        let permit = match Arc::clone(&handshake_limiter).try_acquire_owned() {
                            Ok(permit) => permit,
                            Err(_) => {
                                // Drop connection immediately if handshake concurrency cap is reached
                                continue;
                            }
                        };
                        let peer_ip = match socket_addr.ip() {
                            std::net::IpAddr::V4(ipv4) => ipv4,
                            std::net::IpAddr::V6(ipv6) => {
                                if let Some(ipv4) = ipv6.to_ipv4_mapped() {
                                    ipv4
                                } else {
                                    continue;
                                }
                            }
                        };
                        let peer_port = socket_addr.port();
                        let registry = Arc::clone(&registry);

                        tokio::spawn(async move {
                            let _permit = permit;
                            // Read 68-byte handshake with 10s timeout
                            let handshake_res = tokio::time::timeout(
                                std::time::Duration::from_secs(10),
                                async {
                                    let mut buf = [0u8; 68];
                                    stream.read_exact(&mut buf).await?;
                                    Handshake::from_bytes(&buf)
                                },
                            )
                            .await;

                            let incoming = match handshake_res {
                                Ok(Ok(h)) => h,
                                _ => return,
                            };

                            if let Some(registration) = registry.get(&incoming.info_hash).await {
                                let our_handshake = Handshake::new(
                                    incoming.info_hash,
                                    registration.session_peer_id,
                                );
                                if stream.write_all(&our_handshake.to_bytes()).await.is_err() {
                                    return;
                                }

                                let _ = tokio::time::timeout(
                                    std::time::Duration::from_secs(5),
                                    registration.sender.send(InboundConnection {
                                        stream,
                                        peer_ip,
                                        peer_port,
                                        remote_peer_id: incoming.peer_id,
                                        supports_extensions: incoming.supports_extensions(),
                                    }),
                                )
                                .await;
                            }
                        });
                    }
                    Err(e) => {
                        warn!("Error accepting peer connection: {}", e);
                    }
                }
            }
        });

        Ok(())
    }
}
