use hydroxy_peer_wire_proto::handshake::Handshake;
use hydroxy_tracker::Peer;
use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Mutex, Semaphore, mpsc},
};
use tracing::{info, warn};

const MAX_CONCURRENT_INBOUND_HANDSHAKES: usize = 64;

pub struct InboundConnection {
    pub stream: TcpStream,
    pub peer_ip: IpAddr,
    pub peer_port: u16,
    pub remote_peer_id: [u8; 20],
    pub supports_extensions: bool,
    pub supports_dht: bool,
    pub supports_fast: bool,
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
        listen_address: IpAddr,
        listen_port: u16,
        registry: Arc<InboundTorrentRegistry>,
    ) -> Result<(), std::io::Error> {
        let handshake_limiter = Arc::new(Semaphore::new(MAX_CONCURRENT_INBOUND_HANDSHAKES));

        // When configured on an unspecified wildcard (`0.0.0.0` or `::`), bind dual-stack
        // so both IPv4 and IPv6 peers can connect inbound regardless of OS `bindv6only` setting.
        if listen_address.is_unspecified() {
            let v6_addr = SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), listen_port);
            let v4_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), listen_port);

            match TcpListener::bind(v6_addr).await {
                Ok(v6_listener) => {
                    info!(target: "major", "Peer inbound dual-stack listener active on {}", v6_addr);
                    Self::spawn_accept_loop(
                        v6_listener,
                        Arc::clone(&registry),
                        Arc::clone(&handshake_limiter),
                    );
                    // On systems with `IPV6_V6ONLY = 1`, also bind 0.0.0.0:port; on Linux (`V6ONLY = 0`),
                    // `[::]:port` already accepts IPv4-mapped connections and this returns AddrInUse.
                    if let Ok(v4_listener) = TcpListener::bind(v4_addr).await {
                        info!(target: "major", "Peer inbound IPv4 listener active on {}", v4_addr);
                        Self::spawn_accept_loop(v4_listener, registry, handshake_limiter);
                    }
                    return Ok(());
                }
                Err(_) => {
                    let v4_listener = TcpListener::bind(v4_addr).await?;
                    info!(target: "major", "Peer inbound IPv4 listener active on {}", v4_addr);
                    Self::spawn_accept_loop(v4_listener, registry, handshake_limiter);
                    return Ok(());
                }
            }
        }

        let bind_addr = SocketAddr::new(listen_address, listen_port);
        let listener = TcpListener::bind(bind_addr).await?;
        info!(target: "major", "Peer inbound listener active on {}", bind_addr);
        Self::spawn_accept_loop(listener, registry, handshake_limiter);

        Ok(())
    }

    fn spawn_accept_loop(
        listener: TcpListener,
        registry: Arc<InboundTorrentRegistry>,
        handshake_limiter: Arc<Semaphore>,
    ) {
        tokio::spawn(async move {
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
                        let peer_ip = Peer::normalize_ip(socket_addr.ip());
                        let peer_port = socket_addr.port();
                        if peer_port == 0 || !Peer::is_valid_peer_ip(peer_ip) {
                            continue;
                        }
                        let registry = Arc::clone(&registry);

                        tokio::spawn(async move {
                            let _permit = permit;
                            // Read 68-byte handshake with 10s timeout
                            let handshake_res =
                                tokio::time::timeout(std::time::Duration::from_secs(10), async {
                                    let mut buf = [0u8; 68];
                                    stream.read_exact(&mut buf).await?;
                                    Handshake::from_bytes(&buf)
                                })
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
                                        supports_dht: incoming.supports_dht(),
                                        supports_fast: incoming.supports_fast(),
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
    }
}
