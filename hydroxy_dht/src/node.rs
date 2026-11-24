use std::{
    collections::{HashMap, HashSet},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use tokio::{
    net::UdpSocket,
    sync::{Mutex, RwLock, mpsc, oneshot},
};
use tracing::{debug, info, trace, warn};

use crate::{
    errors::DHTError,
    id::{NodeId, NodeInfo, compare_distance, generate_random_id},
    krpc::{KRPCMessage, QueryPayload, ResponsePayload},
    routing::RoutingTable,
    storage::PeerStorage,
    token::TokenManager,
};

pub const DEFAULT_BOOTSTRAP_ROUTERS: &[&str] = &[
    // Router 1 => maintained by BitTorrent, Inc.
    "router.bittorrent.com:6881",
    // Router 2 => maintained by BitTorrent, Inc.
    "router.utorrent.com:6881",
    // Router 3 => maintained by libtorrent project (Arvid Norberg)
    "dht.libtorrent.org:25401",
    // Router 4 => maintained by Transmission Project
    "dht.transmissionbt.com:6881",
    // Router 5 => maintained by BitComet Project
    "router.bitcomet.com:6881",
    // Router 6 => maintained by Vuze Project
    "dht.aelitis.com:6881",
];

const DEFAULT_QUERY_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_CONCURRENT_LOOKUPS: usize = 4;
const LOOKUP_ALPHA: usize = 3;

#[derive(Debug, Clone)]
pub struct DHTConfig {
    pub bind_addr: SocketAddrV4,
    pub local_id: Option<NodeId>,
    pub bootstrap_routers: Vec<String>,
    pub query_timeout: Duration,
}

impl Default for DHTConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0),
            local_id: None,
            bootstrap_routers: DEFAULT_BOOTSTRAP_ROUTERS
                .iter()
                .map(|s| s.to_string())
                .collect(),
            query_timeout: DEFAULT_QUERY_TIMEOUT,
        }
    }
}

pub struct DHTNode {
    inner: Arc<DHTInner>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    recv_handle: Option<tokio::task::JoinHandle<()>>,
}

pub struct DHTInner {
    local_id: NodeId,
    socket: Arc<UdpSocket>,
    routing_table: RwLock<RoutingTable>,
    token_manager: RwLock<TokenManager>,
    peer_storage: RwLock<PeerStorage>,
    pending_queries: Mutex<HashMap<Vec<u8>, (oneshot::Sender<KRPCMessage>, Instant)>>,
    tx_counter: AtomicU32,
    config: DHTConfig,
}

#[derive(Clone)]
pub struct DHTHandle {
    inner: Arc<DHTInner>,
}

impl DHTNode {
    pub async fn start(config: DHTConfig) -> Result<(Self, DHTHandle), DHTError> {
        let local_id = config.local_id.unwrap_or_else(generate_random_id);
        let socket = UdpSocket::bind(config.bind_addr).await?;
        let socket = Arc::new(socket);

        let local_addr = socket.local_addr()?;
        info!(
            "DHT node started on {} with node ID: {:02x?}",
            local_addr,
            &local_id[..4]
        );

        let inner = Arc::new(DHTInner {
            local_id,
            socket: Arc::clone(&socket),
            routing_table: RwLock::new(RoutingTable::new(local_id)),
            token_manager: RwLock::new(TokenManager::new()),
            peer_storage: RwLock::new(PeerStorage::new()),
            pending_queries: Mutex::new(HashMap::new()),
            tx_counter: AtomicU32::new(1),
            config: config.clone(),
        });

        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        // Spawn receive & dispatch loop
        let recv_inner = Arc::clone(&inner);
        let recv_handle = tokio::spawn(async move {
            Self::receive_loop(recv_inner).await;
        });

        // Spawn background maintenance loop
        let maint_inner = Arc::clone(&inner);
        tokio::spawn(async move {
            Self::maintenance_loop(maint_inner, shutdown_rx).await;
        });

        let handle = DHTHandle {
            inner: Arc::clone(&inner),
        };

        // Trigger initial bootstrap in the background
        let boot_handle = handle.clone();
        tokio::spawn(async move {
            boot_handle.bootstrap().await;
        });

        Ok((
            DHTNode {
                inner,
                shutdown_tx: Some(shutdown_tx),
                recv_handle: Some(recv_handle),
            },
            handle,
        ))
    }

    pub fn handle(&self) -> DHTHandle {
        DHTHandle {
            inner: Arc::clone(&self.inner),
        }
    }

    pub fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.recv_handle.take() {
            handle.abort();
        }
    }

    async fn receive_loop(inner: Arc<DHTInner>) {
        let mut buf = [0u8; 4096];

        loop {
            let (len, src_addr) = match inner.socket.recv_from(&mut buf).await {
                Ok(res) => res,
                Err(e) => {
                    trace!("DHT socket recv error: {}", e);
                    break;
                }
            };

            let src_v4 = match src_addr {
                SocketAddr::V4(v4) => v4,
                SocketAddr::V6(_) => continue, // BEP 5 is IPv4
            };

            let msg = match KRPCMessage::from_bytes(&buf[..len]) {
                Ok(m) => m,
                Err(e) => {
                    trace!("Failed to parse KRPC message from {}: {}", src_v4, e);
                    continue;
                }
            };

            match msg {
                KRPCMessage::Query {
                    transaction_id,
                    query,
                } => {
                    let inner_clone = Arc::clone(&inner);
                    tokio::spawn(async move {
                        inner_clone
                            .handle_query(transaction_id, query, src_v4)
                            .await;
                    });
                }
                KRPCMessage::Response {
                    ref transaction_id, ..
                }
                | KRPCMessage::Error {
                    ref transaction_id, ..
                } => {
                    let mut pending = inner.pending_queries.lock().await;
                    if let Some((tx, _)) = pending.remove(transaction_id) {
                        let _ = tx.send(msg);
                    }
                }
            }
        }
    }

    async fn maintenance_loop(inner: Arc<DHTInner>, mut shutdown_rx: oneshot::Receiver<()>) {
        let mut interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                _ = &mut shutdown_rx => {
                    info!("DHT node received shutdown signal");
                    break;
                }
                _ = interval.tick() => {
                    // Prune timed-out pending queries
                    {
                        let mut pending = inner.pending_queries.lock().await;
                        let now = Instant::now();
                        pending.retain(|_, (_, created)| now.duration_since(*created) < inner.config.query_timeout);
                    }

                    // Rotate token secret if needed
                    {
                        let mut tokens = inner.token_manager.write().await;
                        tokens.maybe_rotate();
                    }

                    // Prune expired peers
                    {
                        let mut storage = inner.peer_storage.write().await;
                        storage.prune_expired(Duration::from_secs(30 * 60));
                    }

                    // Refresh stale routing table buckets
                    let stale = {
                        let rt = inner.routing_table.read().await;
                        rt.stale_buckets(Duration::from_secs(15 * 60))
                    };

                    for target in stale {
                        let h = DHTHandle { inner: Arc::clone(&inner) };
                        tokio::spawn(async move {
                            let _ = h.find_closest_nodes(target).await;
                        });
                    }

                    // Re-bootstrap if routing table has very few nodes
                    let good_count = {
                        let rt = inner.routing_table.read().await;
                        rt.good_nodes()
                    };

                    if good_count < 8 {
                        let h = DHTHandle { inner: Arc::clone(&inner) };
                        tokio::spawn(async move {
                            h.bootstrap().await;
                        });
                    }
                }
            }
        }
    }
}

impl DHTHandle {
    pub fn local_id(&self) -> NodeId {
        self.inner.local_id
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.inner.socket.local_addr()
    }

    /// Returns routing table's size, (total_nodes, good_nodes)
    pub async fn routing_table_size(&self) -> (usize, usize) {
        let rt = self.inner.routing_table.read().await;
        (rt.total_nodes(), rt.good_nodes())
    }

    /// Perform a ping query to a target address.
    pub async fn ping(&self, target: SocketAddrV4) -> Result<NodeId, DHTError> {
        let query = QueryPayload::Ping {
            id: self.inner.local_id,
        };
        let resp = self.send_query(target, query).await?;
        match resp {
            ResponsePayload::Ping { id } | ResponsePayload::AnnouncePeer { id } => Ok(id),
            _ => Err(DHTError::Protocol("Unexpected response to ping".into())),
        }
    }

    /// Perform a find_node query to a target node.
    pub async fn find_node(
        &self,
        target_addr: SocketAddrV4,
        target_id: NodeId,
    ) -> Result<Vec<NodeInfo>, DHTError> {
        let query = QueryPayload::FindNode {
            id: self.inner.local_id,
            target: target_id,
        };
        let resp = self.send_query(target_addr, query).await?;
        match resp {
            ResponsePayload::FindNode { nodes, .. } => Ok(nodes),
            _ => Err(DHTError::Protocol(
                "Unexpected response to find_node".into(),
            )),
        }
    }

    /// Send a low level KRPC Query to Node and await its response.
    async fn send_query(
        &self,
        target: SocketAddrV4,
        query: QueryPayload,
    ) -> Result<ResponsePayload, DHTError> {
        let tx_id = self.inner.next_transaction_id();
        let (rx_tx, rx) = oneshot::channel();

        {
            let mut pending = self.inner.pending_queries.lock().await;
            pending.insert(tx_id.clone(), (rx_tx, Instant::now()));
        }

        let msg = KRPCMessage::Query {
            transaction_id: tx_id.clone(),
            query,
        };
        let bytes = msg.to_bytes();

        if let Err(e) = self
            .inner
            .socket
            .send_to(&bytes, SocketAddr::V4(target))
            .await
        {
            let mut pending = self.inner.pending_queries.lock().await;
            pending.remove(&tx_id);
            return Err(DHTError::Io(e));
        }

        let res = tokio::time::timeout(self.inner.config.query_timeout, rx).await;

        match res {
            Ok(Ok(KRPCMessage::Response { response, .. })) => {
                let node_id = match &response {
                    ResponsePayload::Ping { id } => *id,
                    ResponsePayload::FindNode { id, .. } => *id,
                    ResponsePayload::GetPeers { id, .. } => *id,
                    ResponsePayload::AnnouncePeer { id } => *id,
                };
                let mut rt = self.inner.routing_table.write().await;
                rt.record_success(&node_id, target);
                Ok(response)
            }
            Ok(Ok(KRPCMessage::Error { code, message, .. })) => {
                let mut rt = self.inner.routing_table.write().await;
                rt.record_failure(&target);
                Err(DHTError::KrpcError { code, message })
            }
            Ok(Err(_)) => {
                let mut rt = self.inner.routing_table.write().await;
                rt.record_failure(&target);
                Err(DHTError::ChannelClosed)
            }
            Err(_) => {
                let mut pending = self.inner.pending_queries.lock().await;
                pending.remove(&tx_id);
                let mut rt = self.inner.routing_table.write().await;
                rt.record_failure(&target);
                Err(DHTError::Timeout(tx_id))
            }
            _ => Err(DHTError::Protocol("Invalid response message type".into())),
        }
    }

    /// Bootstrap the DHT by resolving well-known routers and populating the routing table.
    pub async fn bootstrap(&self) {
        info!("Starting DHT bootstrap...");
        let mut router_addrs = Vec::new();

        for router in &self.inner.config.bootstrap_routers {
            match tokio::net::lookup_host(router).await {
                Ok(addrs) => {
                    for addr in addrs {
                        if let SocketAddr::V4(v4) = addr {
                            router_addrs.push(v4);
                        }
                    }
                }
                Err(e) => {
                    trace!("Failed to resolve DHT bootstrap router {}: {}", router, e);
                }
            }
        }

        if router_addrs.is_empty() {
            warn!("DHT bootstrap: no router addresses could be resolved");
            return;
        }

        info!(
            "Resolved {} DHT bootstrap router endpoints",
            router_addrs.len()
        );

        // Send initial find_node for our own ID to all bootstrap nodes
        let mut initial_nodes = Vec::new();
        for addr in router_addrs {
            let h = self.clone();
            let my_id = self.inner.local_id;
            if let Ok(nodes) = h.find_node(addr, my_id).await {
                initial_nodes.extend(nodes);
            }
        }

        // Add discovered nodes into routing table
        {
            let mut rt = self.inner.routing_table.write().await;
            for node in &initial_nodes {
                rt.inject_node(*node);
            }
        }

        // Run find_closest_nodes for self to expand neighbor buckets
        self.find_closest_nodes(self.inner.local_id).await;

        let (total, good) = self.routing_table_size().await;
        info!(
            "DHT bootstrap completed: {} nodes discovered ({} good)",
            total, good
        );
    }

    /// Iterative lookup to find closest nodes to `target_id`.
    pub async fn find_closest_nodes(&self, target_id: NodeId) -> Vec<NodeInfo> {
        let initial_nodes = {
            let rt = self.inner.routing_table.read().await;
            rt.closest_nodes(&target_id, 8)
        };

        let mut queried: HashSet<SocketAddrV4> = HashSet::new();
        let mut candidates = initial_nodes;

        for _ in 0..8 {
            candidates.sort_by(|a, b| compare_distance(&target_id, &a.id, &b.id));

            let to_query: Vec<NodeInfo> = candidates
                .iter()
                .filter(|n| !queried.contains(&n.ip_v4_addr))
                .take(LOOKUP_ALPHA)
                .cloned()
                .collect();

            if to_query.is_empty() {
                break;
            }

            let mut tasks = Vec::new();
            for node in to_query {
                queried.insert(node.ip_v4_addr);
                let h = self.clone();
                tasks.push(async move { h.find_node(node.ip_v4_addr, target_id).await });
            }

            let results = futures_util_join_all(tasks).await;
            let mut added_any = false;

            for res in results {
                if let Ok(nodes) = res {
                    let mut rt = self.inner.routing_table.write().await;
                    for node in nodes {
                        rt.inject_node(node);
                        if !queried.contains(&node.ip_v4_addr)
                            && !candidates.iter().any(|c| c.ip_v4_addr == node.ip_v4_addr)
                        {
                            candidates.push(node);
                            added_any = true;
                        }
                    }
                }
            }

            if !added_any {
                break;
            }
        }

        candidates.sort_by(|a, b| compare_distance(&target_id, &a.id, &b.id));
        candidates.truncate(8);
        candidates
    }

    /// Stream peers discovered via DHT for the given `info_hash`.
    /// Also returns a list of (NodeInfo, token) for the closest nodes reached (for subsequent announces).
    pub async fn get_peers(
        &self,
        info_hash: NodeId,
        peer_tx: mpsc::Sender<SocketAddrV4>,
    ) -> Vec<(NodeInfo, Vec<u8>)> {
        let initial_nodes = {
            let rt = self.inner.routing_table.read().await;
            rt.closest_nodes(&info_hash, 8)
        };

        let mut queried: HashSet<SocketAddrV4> = HashSet::new();
        let mut candidates = initial_nodes;
        let mut token_map: HashMap<SocketAddrV4, (NodeInfo, Vec<u8>)> = HashMap::new();
        let mut known_peers: HashSet<SocketAddrV4> = HashSet::new();

        for _ in 0..10 {
            candidates.sort_by(|a, b| compare_distance(&info_hash, &a.id, &b.id));

            let to_query: Vec<NodeInfo> = candidates
                .iter()
                .filter(|n| !queried.contains(&n.ip_v4_addr))
                .take(MAX_CONCURRENT_LOOKUPS)
                .cloned()
                .collect();

            if to_query.is_empty() {
                break;
            }

            let mut tasks = Vec::new();
            for node in to_query {
                queried.insert(node.ip_v4_addr);
                let h = self.clone();
                let my_id = self.inner.local_id;
                tasks.push(async move {
                    let q = QueryPayload::GetPeers {
                        id: my_id,
                        info_hash,
                    };
                    let resp = h.send_query(node.ip_v4_addr, q).await;
                    (node, resp)
                });
            }

            let results = futures_util_join_all(tasks).await;

            for (node, res) in results {
                if let Ok(ResponsePayload::GetPeers {
                    id: resp_id,
                    token,
                    peers,
                    nodes,
                }) = res
                {
                    token_map.insert(
                        node.ip_v4_addr,
                        (NodeInfo::new(resp_id, node.ip_v4_addr), token),
                    );

                    // Send discovered peers
                    for peer in peers {
                        if known_peers.insert(peer) {
                            let _ = peer_tx.send(peer).await;
                        }
                    }

                    // Add discovered closer nodes into candidates
                    let mut rt = self.inner.routing_table.write().await;
                    for closer_node in nodes {
                        rt.inject_node(closer_node);
                        if !queried.contains(&closer_node.ip_v4_addr)
                            && !candidates
                                .iter()
                                .any(|c| c.ip_v4_addr == closer_node.ip_v4_addr)
                        {
                            candidates.push(closer_node);
                        }
                    }
                }
            }
        }

        token_map.into_values().collect()
    }

    /// Announce peer presence for `info_hash` to the closest DHT nodes while streaming discovered peers.
    pub async fn announce_peer_streaming(
        &self,
        info_hash: NodeId,
        port: u16,
        implied_port: bool,
        peer_tx: mpsc::Sender<SocketAddrV4>,
    ) -> usize {
        let nodes_with_tokens = self.get_peers(info_hash, peer_tx).await;

        let mut successful = 0;
        let mut tasks = Vec::new();

        for (node, token) in nodes_with_tokens {
            let h = self.clone();
            let my_id = self.inner.local_id;
            tasks.push(async move {
                let query = QueryPayload::AnnouncePeer {
                    id: my_id,
                    info_hash,
                    port,
                    token,
                    implied_port,
                };
                h.send_query(node.ip_v4_addr, query).await
            });
        }

        let results = futures_util_join_all(tasks).await;
        for res in results {
            if let Ok(ResponsePayload::AnnouncePeer { .. } | ResponsePayload::Ping { .. }) = res {
                successful += 1;
            }
        }

        info!(
            "Announced to {} DHT nodes for {:02x?}",
            successful,
            &info_hash[..4]
        );
        successful
    }

    /// Announce peer presence for `info_hash` to the closest DHT nodes.
    pub async fn announce_peer(&self, info_hash: NodeId, port: u16, implied_port: bool) -> usize {
        let (dummy_tx, _) = mpsc::channel(1);
        self.announce_peer_streaming(info_hash, port, implied_port, dummy_tx)
            .await
    }
}

impl DHTInner {
    async fn handle_query(&self, tx_id: Vec<u8>, query: QueryPayload, from: SocketAddrV4) {
        // Record Node in Routing Table.
        let sender_id = match &query {
            QueryPayload::Ping { id } => *id,
            QueryPayload::FindNode { id, .. } => *id,
            QueryPayload::GetPeers { id, .. } => *id,
            QueryPayload::AnnouncePeer { id, .. } => *id,
        };

        {
            let mut rt = self.routing_table.write().await;
            rt.inject_node(NodeInfo::new(sender_id, from));
        }

        // Response Message
        let res_msg = match query {
            QueryPayload::Ping { .. } => KRPCMessage::Response {
                transaction_id: tx_id,
                response: ResponsePayload::Ping { id: self.local_id },
            },

            QueryPayload::FindNode { target, .. } => {
                let closest = {
                    let rt = self.routing_table.read().await;
                    rt.closest_nodes(&target, 8)
                };

                KRPCMessage::Response {
                    transaction_id: tx_id,
                    response: ResponsePayload::FindNode {
                        id: self.local_id,
                        nodes: closest,
                    },
                }
            }
            QueryPayload::GetPeers { info_hash, .. } => {
                let token = {
                    let tm = self.token_manager.read().await;
                    tm.generate_token(from.ip())
                };

                let peers = {
                    let storage = self.peer_storage.read().await;
                    storage.get_peers(&info_hash, 50)
                };

                let nodes = if peers.is_empty() {
                    let rt = self.routing_table.read().await;
                    rt.closest_nodes(&info_hash, 8)
                } else {
                    Vec::new()
                };

                KRPCMessage::Response {
                    transaction_id: tx_id,
                    response: ResponsePayload::GetPeers {
                        id: self.local_id,
                        token,
                        peers,
                        nodes,
                    },
                }
            }
            QueryPayload::AnnouncePeer {
                info_hash,
                port,
                token,
                implied_port,
                ..
            } => {
                let token_valid = {
                    let tm = self.token_manager.read().await;
                    tm.validate_token(from.ip(), &token)
                };

                if !token_valid {
                    KRPCMessage::Error {
                        transaction_id: tx_id,
                        code: 203,
                        message: "Invalid token".to_string(),
                    }
                } else {
                    let peer_port = if implied_port { from.port() } else { port };
                    let peer_addr = SocketAddrV4::new(*from.ip(), peer_port);

                    {
                        let mut storage = self.peer_storage.write().await;
                        storage.store_peer(info_hash, peer_addr);
                    }

                    debug!(
                        "DHT stored announced peer {} for {:02x?}",
                        peer_addr,
                        &info_hash[..4]
                    );

                    KRPCMessage::Response {
                        transaction_id: tx_id,
                        response: ResponsePayload::AnnouncePeer { id: self.local_id },
                    }
                }
            }
        };

        let bytes = res_msg.to_bytes();
        let _ = self.socket.send_to(&bytes, SocketAddr::V4(from)).await;
    }

    fn next_transaction_id(&self) -> Vec<u8> {
        let val = self.tx_counter.fetch_add(1, Ordering::Relaxed);
        val.to_be_bytes()[2..].to_vec() // 2-byte Transaction ID.
    }
}

// async helper to join all futures
async fn futures_util_join_all<F, T>(futures: Vec<F>) -> Vec<T>
where
    F: std::future::Future<Output = T>,
{
    let mut results = Vec::with_capacity(futures.len());
    for f in futures {
        results.push(f.await);
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn test_two_dht_nodes_ping_and_find_node() {
        let config1 = DHTConfig {
            bind_addr: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
            bootstrap_routers: vec![],
            ..Default::default()
        };
        let (_node1, handle1) = DHTNode::start(config1).await.unwrap();

        let config2 = DHTConfig {
            bind_addr: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
            bootstrap_routers: vec![],
            ..Default::default()
        };
        let (_node2, handle2) = DHTNode::start(config2).await.unwrap();

        let addr1 = match handle1.local_addr().unwrap() {
            SocketAddr::V4(v4) => v4,
            _ => panic!("Expected IPv4"),
        };

        // Node 2 pings Node 1
        let pong_id = handle2.ping(addr1).await.expect("ping should succeed");
        assert_eq!(pong_id, handle1.local_id());

        // Node 2 should now have Node 1 in its routing table
        let (total, _) = handle2.routing_table_size().await;
        assert_eq!(total, 1);

        // Node 2 find_node query to Node 1
        let nodes = handle2
            .find_node(addr1, [99u8; 20])
            .await
            .expect("find_node should succeed");
        // Should return closest nodes
        assert!(nodes.len() <= 8);
    }

    #[tokio::test]
    async fn test_get_peers_and_announce_peer_flow() {
        let config1 = DHTConfig {
            bind_addr: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
            bootstrap_routers: vec![],
            ..Default::default()
        };
        let (_node1, handle1) = DHTNode::start(config1).await.unwrap();

        let config2 = DHTConfig {
            bind_addr: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
            bootstrap_routers: vec![],
            ..Default::default()
        };
        let (_node2, handle2) = DHTNode::start(config2).await.unwrap();

        let addr1 = match handle1.local_addr().unwrap() {
            SocketAddr::V4(v4) => v4,
            _ => panic!("Expected IPv4"),
        };

        let info_hash = [42u8; 20];

        // Seed node 2's routing table with node 1
        handle2.ping(addr1).await.unwrap();

        // Node 2 announces to Node 1
        let announced = handle2.announce_peer(info_hash, 6881, false).await;
        assert!(announced >= 1);

        // Now Node 1 should have the peer in storage
        let (peer_tx, mut peer_rx) = mpsc::channel(10);
        let _ = handle2.get_peers(info_hash, peer_tx).await;

        let peer = tokio::time::timeout(Duration::from_millis(500), peer_rx.recv())
            .await
            .expect("should receive peer")
            .expect("peer channel open");

        assert_eq!(peer.port(), 6881);
    }
}
