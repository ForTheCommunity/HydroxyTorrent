use std::{
    collections::{HashMap, HashSet},
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
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
    krpc::{KRPCMessage, QueryPayload, ResponsePayload, WantFlags},
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
    pub bind_addr_v6: Option<SocketAddrV6>,
    pub enable_ipv6: bool,
    pub local_id: Option<NodeId>,
    pub bootstrap_routers: Vec<String>,
    pub query_timeout: Duration,
}

impl Default for DHTConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0),
            bind_addr_v6: None,
            enable_ipv6: true,
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
    recv_handles: Vec<tokio::task::JoinHandle<()>>,
}

pub struct DHTInner {
    local_id: NodeId,
    socket_v4: Option<Arc<UdpSocket>>,
    socket_v6: Option<Arc<UdpSocket>>,
    v6_is_dual_stack: bool,
    routing_table_v4: RwLock<RoutingTable>,
    routing_table_v6: RwLock<RoutingTable>,
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

        let mut socket_v4: Option<Arc<UdpSocket>> = None;
        let mut socket_v6: Option<Arc<UdpSocket>> = None;
        let mut v6_is_dual_stack = false;

        if config.enable_ipv6 {
            if let Some(explicit_v6) = config.bind_addr_v6 {
                let s6 = UdpSocket::bind(explicit_v6).await?;
                socket_v6 = Some(Arc::new(s6));
                if !config.bind_addr.ip().is_unspecified() || config.bind_addr.port() != 0 {
                    if let Ok(s4) = UdpSocket::bind(config.bind_addr).await {
                        socket_v4 = Some(Arc::new(s4));
                    }
                }
            } else if config.bind_addr.ip().is_unspecified() {
                // Wildcard dual-stack bind (works on IPv4-only, Dual-Stack, and IPv6-only hosts)
                let target_v6 =
                    SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, config.bind_addr.port(), 0, 0);
                match UdpSocket::bind(target_v6).await {
                    Ok(s6) => {
                        let bound_port = s6.local_addr()?.port();
                        socket_v6 = Some(Arc::new(s6));
                        let target_v4 = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, bound_port);
                        match UdpSocket::bind(target_v4).await {
                            Ok(s4) => {
                                socket_v4 = Some(Arc::new(s4));
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
                                // Linux `IPV6_V6ONLY = 0`: `[::]:port` is already dual-stack
                                v6_is_dual_stack = true;
                            }
                            Err(_) => {
                                // Pure IPv6-only system where IPv4 is unavailable
                            }
                        }
                    }
                    Err(_) => {
                        // IPv6 disabled in kernel: fall back to IPv4 socket
                        let s4 = UdpSocket::bind(config.bind_addr).await?;
                        socket_v4 = Some(Arc::new(s4));
                    }
                }
            } else {
                // Explicit non-wildcard IPv4 bind (e.g., `127.0.0.1:0` in local tests)
                let s4 = UdpSocket::bind(config.bind_addr).await?;
                socket_v4 = Some(Arc::new(s4));
            }
        } else {
            let s4 = UdpSocket::bind(config.bind_addr).await?;
            socket_v4 = Some(Arc::new(s4));
        }

        if socket_v4.is_none() && socket_v6.is_none() {
            return Err(DHTError::Io(std::io::Error::new(
                std::io::ErrorKind::AddrNotAvailable,
                "Failed to bind any UDP socket for DHT",
            )));
        }

        let inner = Arc::new(DHTInner {
            local_id,
            socket_v4: socket_v4.clone(),
            socket_v6: socket_v6.clone(),
            v6_is_dual_stack,
            routing_table_v4: RwLock::new(RoutingTable::new(local_id)),
            routing_table_v6: RwLock::new(RoutingTable::new(local_id)),
            token_manager: RwLock::new(TokenManager::new()),
            peer_storage: RwLock::new(PeerStorage::new()),
            pending_queries: Mutex::new(HashMap::new()),
            tx_counter: AtomicU32::new(1),
            config: config.clone(),
        });

        if let Ok(local_addr) = inner.primary_local_addr() {
            info!(
                "DHT node started on {} (dual_stack_v6={}) with node ID: {:02x?}",
                local_addr,
                socket_v6.is_some(),
                &local_id[..4]
            );
        }

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let mut recv_handles = Vec::new();

        if let Some(s4) = socket_v4 {
            let recv_inner = Arc::clone(&inner);
            recv_handles.push(tokio::spawn(async move {
                Self::receive_loop(recv_inner, s4).await;
            }));
        }
        if let Some(s6) = socket_v6 {
            let recv_inner = Arc::clone(&inner);
            recv_handles.push(tokio::spawn(async move {
                Self::receive_loop(recv_inner, s6).await;
            }));
        }

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
                recv_handles,
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
        for handle in self.recv_handles.drain(..) {
            handle.abort();
        }
    }

    async fn receive_loop(inner: Arc<DHTInner>, sock: Arc<UdpSocket>) {
        let mut buf = [0u8; 4096];

        loop {
            let (len, raw_src) = match sock.recv_from(&mut buf).await {
                Ok(res) => res,
                Err(e) => {
                    trace!("DHT socket recv error: {}", e);
                    break;
                }
            };

            let src_addr = match raw_src {
                SocketAddr::V4(v4) => SocketAddr::V4(v4),
                SocketAddr::V6(v6) => match v6.ip().to_ipv4_mapped() {
                    Some(mapped_v4) => SocketAddr::V4(SocketAddrV4::new(mapped_v4, v6.port())),
                    None => SocketAddr::V6(v6),
                },
            };

            let msg = match KRPCMessage::from_bytes(&buf[..len]) {
                Ok(m) => m,
                Err(e) => {
                    trace!("Failed to parse KRPC message from {}: {}", src_addr, e);
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
                            .handle_query(transaction_id, query, src_addr)
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
        interval.tick().await; // skip immediate tick at t=0s since start() spawns initial bootstrap

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

                    // Refresh stale buckets in both IPv4 and IPv6 routing tables
                    let stale_v4 = inner.routing_table_v4.read().await.stale_buckets(Duration::from_secs(15 * 60));
                    for target in stale_v4 {
                        let h = DHTHandle { inner: Arc::clone(&inner) };
                        tokio::spawn(async move {
                            let _ = h.find_closest_nodes_for_family(target, false).await;
                        });
                    }

                    let stale_v6 = inner.routing_table_v6.read().await.stale_buckets(Duration::from_secs(15 * 60));
                    for target in stale_v6 {
                        let h = DHTHandle { inner: Arc::clone(&inner) };
                        tokio::spawn(async move {
                            let _ = h.find_closest_nodes_for_family(target, true).await;
                        });
                    }

                    // Re-bootstrap if combined good node count is low
                    let good_count = inner.routing_table_v4.read().await.good_nodes()
                        + inner.routing_table_v6.read().await.good_nodes();

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
        self.inner.primary_local_addr()
    }

    /// Returns total and good node counts combined across IPv4 and IPv6 Kademlia routing tables.
    pub async fn routing_table_size(&self) -> (usize, usize) {
        let (t4, g4) = {
            let rt4 = self.inner.routing_table_v4.read().await;
            (rt4.total_nodes(), rt4.good_nodes())
        };
        let (t6, g6) = {
            let rt6 = self.inner.routing_table_v6.read().await;
            (rt6.total_nodes(), rt6.good_nodes())
        };
        (t4 + t6, g4 + g6)
    }

    /// Perform a ping query to a target IPv4 or IPv6 DHT node.
    pub async fn ping(&self, target: SocketAddr) -> Result<NodeId, DHTError> {
        let query = QueryPayload::Ping {
            id: self.inner.local_id,
        };
        let resp = self.send_query(target, query).await?;
        match resp {
            ResponsePayload::Ping { id } | ResponsePayload::AnnouncePeer { id } => Ok(id),
            _ => Err(DHTError::Protocol("Unexpected response to ping".into())),
        }
    }

    /// Perform a `find_node` query to a target node requesting the specified address families (`want`).
    pub async fn find_node_ext(
        &self,
        target_addr: SocketAddr,
        target_id: NodeId,
        want: WantFlags,
    ) -> Result<(Vec<NodeInfo>, Vec<NodeInfo>), DHTError> {
        let query = QueryPayload::FindNode {
            id: self.inner.local_id,
            target: target_id,
            want,
        };
        let resp = self.send_query(target_addr, query).await?;
        match resp {
            ResponsePayload::FindNode { nodes, nodes6, .. } => Ok((nodes, nodes6)),
            _ => Err(DHTError::Protocol(
                "Unexpected response to find_node".into(),
            )),
        }
    }

    /// Perform a `find_node` query returning all discovered nodes matching the target's family or requested `want`.
    pub async fn find_node(
        &self,
        target_addr: SocketAddr,
        target_id: NodeId,
    ) -> Result<Vec<NodeInfo>, DHTError> {
        let want = self.inner.default_want_flags();
        let (mut n4, mut n6) = self.find_node_ext(target_addr, target_id, want).await?;
        n4.append(&mut n6);
        Ok(n4)
    }

    /// Send a low-level KRPC query to a node (IPv4 or IPv6) and await its response.
    async fn send_query(
        &self,
        target: SocketAddr,
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

        if let Err(e) = self.inner.send_packet(&bytes, target).await {
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
                self.inner.record_node_success(&node_id, target).await;
                Ok(response)
            }
            Ok(Ok(KRPCMessage::Error { code, message, .. })) => {
                self.inner.record_node_failure(&target).await;
                Err(DHTError::KrpcError { code, message })
            }
            Ok(Err(_)) => {
                self.inner.record_node_failure(&target).await;
                Err(DHTError::ChannelClosed)
            }
            Err(_) => {
                let mut pending = self.inner.pending_queries.lock().await;
                pending.remove(&tx_id);
                self.inner.record_node_failure(&target).await;
                Err(DHTError::Timeout(tx_id))
            }
            _ => Err(DHTError::Protocol("Invalid response message type".into())),
        }
    }

    /// Bootstrap the DHT (both IPv4 and IPv6 tables when available) using BEP 0005 + BEP 0032 `want`.
    pub async fn bootstrap(&self) {
        info!("Starting DHT bootstrap...");
        let mut router_addrs = Vec::new();

        for router in &self.inner.config.bootstrap_routers {
            match tokio::net::lookup_host(router).await {
                Ok(addrs) => {
                    for addr in addrs {
                        if (addr.is_ipv4() && self.inner.can_send_v4())
                            || (addr.is_ipv6() && self.inner.can_send_v6())
                        {
                            router_addrs.push(addr);
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

        let my_id = self.inner.local_id;
        let want = self.inner.default_want_flags();
        let mut boot_tasks = Vec::with_capacity(router_addrs.len());
        for addr in router_addrs {
            let h = self.clone();
            boot_tasks.push(async move { h.find_node_ext(addr, my_id, want).await });
        }

        let results = futures_util_join_all(boot_tasks).await;
        for res in results {
            if let Ok((nodes4, nodes6)) = res {
                for node in nodes4.into_iter().chain(nodes6.into_iter()) {
                    self.inner.inject_discovered_node(node).await;
                }
            }
        }

        // Expand neighbor buckets in both IPv4 and IPv6 routing tables concurrently
        tokio::join!(
            self.find_closest_nodes_for_family(my_id, false),
            self.find_closest_nodes_for_family(my_id, true),
        );

        let (total, good) = self.routing_table_size().await;
        info!(
            "DHT bootstrap completed: {} nodes discovered ({} good)",
            total, good
        );
    }

    /// Iterative Kademlia lookup across both IPv4 and IPv6 routing tables.
    pub async fn find_closest_nodes(&self, target_id: NodeId) -> Vec<NodeInfo> {
        let (mut v4, mut v6) = tokio::join!(
            self.find_closest_nodes_for_family(target_id, false),
            self.find_closest_nodes_for_family(target_id, true),
        );
        v4.append(&mut v6);
        v4
    }

    async fn find_closest_nodes_for_family(
        &self,
        target_id: NodeId,
        ipv6_family: bool,
    ) -> Vec<NodeInfo> {
        if ipv6_family && !self.inner.can_send_v6() {
            return Vec::new();
        }
        if !ipv6_family && !self.inner.can_send_v4() {
            return Vec::new();
        }

        let initial_nodes = if ipv6_family {
            self.inner
                .routing_table_v6
                .read()
                .await
                .closest_nodes(&target_id, 8)
        } else {
            self.inner
                .routing_table_v4
                .read()
                .await
                .closest_nodes(&target_id, 8)
        };

        if initial_nodes.is_empty() {
            return Vec::new();
        }

        let want = self.inner.default_want_flags();
        let mut queried: HashSet<SocketAddr> = HashSet::new();
        let mut candidates = initial_nodes;

        for _ in 0..8 {
            candidates.sort_by(|a, b| compare_distance(&target_id, &a.id, &b.id));

            let to_query: Vec<NodeInfo> = candidates
                .iter()
                .filter(|n| !queried.contains(&n.addr))
                .take(LOOKUP_ALPHA)
                .copied()
                .collect();

            if to_query.is_empty() {
                break;
            }

            let mut tasks = Vec::new();
            for node in to_query {
                queried.insert(node.addr);
                let h = self.clone();
                tasks.push(async move { h.find_node_ext(node.addr, target_id, want).await });
            }

            let results = futures_util_join_all(tasks).await;
            let mut added_any = false;

            for res in results {
                if let Ok((nodes4, nodes6)) = res {
                    for node in nodes4.into_iter().chain(nodes6.into_iter()) {
                        if node.id == self.inner.local_id {
                            continue;
                        }
                        self.inner.inject_discovered_node(node).await;
                        if node.addr.is_ipv6() == ipv6_family
                            && !queried.contains(&node.addr)
                            && !candidates.iter().any(|c| c.addr == node.addr)
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

    /// Stream IPv4 (`6-byte`) and IPv6 (`18-byte`) peers discovered via DHT for `info_hash` (BEP 0005 + BEP 0032).
    pub async fn get_peers(
        &self,
        info_hash: NodeId,
        peer_tx: mpsc::Sender<SocketAddr>,
    ) -> Vec<(NodeInfo, Vec<u8>)> {
        let shared_known = Arc::new(Mutex::new(HashSet::<SocketAddr>::new()));
        let (mut tokens4, mut tokens6) = tokio::join!(
            self.get_peers_for_family(info_hash, false, peer_tx.clone(), Arc::clone(&shared_known)),
            self.get_peers_for_family(info_hash, true, peer_tx, shared_known),
        );
        tokens4.append(&mut tokens6);
        tokens4
    }

    async fn get_peers_for_family(
        &self,
        info_hash: NodeId,
        ipv6_family: bool,
        peer_tx: mpsc::Sender<SocketAddr>,
        known_peers: Arc<Mutex<HashSet<SocketAddr>>>,
    ) -> Vec<(NodeInfo, Vec<u8>)> {
        if ipv6_family && !self.inner.can_send_v6() {
            return Vec::new();
        }
        if !ipv6_family && !self.inner.can_send_v4() {
            return Vec::new();
        }

        let initial_nodes = if ipv6_family {
            self.inner
                .routing_table_v6
                .read()
                .await
                .closest_nodes(&info_hash, 8)
        } else {
            self.inner
                .routing_table_v4
                .read()
                .await
                .closest_nodes(&info_hash, 8)
        };

        if initial_nodes.is_empty() {
            return Vec::new();
        }

        let want = self.inner.default_want_flags();
        let mut queried: HashSet<SocketAddr> = HashSet::new();
        let mut candidates = initial_nodes;
        let mut token_map: HashMap<SocketAddr, (NodeInfo, Vec<u8>)> = HashMap::new();

        for _ in 0..10 {
            candidates.sort_by(|a, b| compare_distance(&info_hash, &a.id, &b.id));

            let to_query: Vec<NodeInfo> = candidates
                .iter()
                .filter(|n| !queried.contains(&n.addr))
                .take(MAX_CONCURRENT_LOOKUPS)
                .copied()
                .collect();

            if to_query.is_empty() {
                break;
            }

            let mut tasks = Vec::new();
            for node in to_query {
                queried.insert(node.addr);
                let h = self.clone();
                let my_id = self.inner.local_id;
                tasks.push(async move {
                    let q = QueryPayload::GetPeers {
                        id: my_id,
                        info_hash,
                        want,
                    };
                    let resp = h.send_query(node.addr, q).await;
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
                    nodes6,
                }) = res
                {
                    token_map.insert(node.addr, (NodeInfo::new(resp_id, node.addr), token));

                    for peer in peers {
                        let is_new = known_peers.lock().await.insert(peer);
                        if is_new {
                            let _ = peer_tx.send(peer).await;
                        }
                    }

                    for closer_node in nodes.into_iter().chain(nodes6.into_iter()) {
                        if closer_node.id == self.inner.local_id {
                            continue;
                        }
                        self.inner.inject_discovered_node(closer_node).await;
                        if closer_node.addr.is_ipv6() == ipv6_family
                            && !queried.contains(&closer_node.addr)
                            && !candidates.iter().any(|c| c.addr == closer_node.addr)
                        {
                            candidates.push(closer_node);
                        }
                    }
                }
            }
        }

        token_map.into_values().collect()
    }

    /// Announce peer presence for `info_hash` to the closest DHT nodes (IPv4 and/or IPv6) while streaming discovered peers.
    pub async fn announce_peer_streaming(
        &self,
        info_hash: NodeId,
        port: u16,
        implied_port: bool,
        peer_tx: mpsc::Sender<SocketAddr>,
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
                h.send_query(node.addr, query).await
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
    fn can_send_v4(&self) -> bool {
        self.socket_v4.is_some() || (self.v6_is_dual_stack && self.socket_v6.is_some())
    }

    fn can_send_v6(&self) -> bool {
        self.socket_v6.is_some()
    }

    fn default_want_flags(&self) -> WantFlags {
        WantFlags {
            n4: self.can_send_v4(),
            n6: self.can_send_v6(),
        }
    }

    fn primary_local_addr(&self) -> std::io::Result<SocketAddr> {
        if let Some(ref s4) = self.socket_v4 {
            return s4.local_addr();
        }
        if let Some(ref s6) = self.socket_v6 {
            return s6.local_addr();
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "No DHT UDP socket bound",
        ))
    }

    async fn send_packet(&self, bytes: &[u8], target: SocketAddr) -> std::io::Result<usize> {
        match target {
            SocketAddr::V4(v4) => {
                if let Some(ref s4) = self.socket_v4 {
                    s4.send_to(bytes, SocketAddr::V4(v4)).await
                } else if self.v6_is_dual_stack {
                    if let Some(ref s6) = self.socket_v6 {
                        let mapped = SocketAddr::V6(SocketAddrV6::new(
                            v4.ip().to_ipv6_mapped(),
                            v4.port(),
                            0,
                            0,
                        ));
                        s6.send_to(bytes, mapped).await
                    } else {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::AddrNotAvailable,
                            "No IPv4 DHT socket",
                        ))
                    }
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::AddrNotAvailable,
                        "No IPv4 DHT socket",
                    ))
                }
            }
            SocketAddr::V6(v6) => {
                if let Some(ref s6) = self.socket_v6 {
                    s6.send_to(bytes, SocketAddr::V6(v6)).await
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::AddrNotAvailable,
                        "No IPv6 DHT socket",
                    ))
                }
            }
        }
    }

    async fn inject_discovered_node(&self, node: NodeInfo) {
        if node.id == self.local_id {
            return;
        }
        if node.addr.is_ipv6() {
            if self.can_send_v6() {
                self.routing_table_v6.write().await.inject_node(node);
            }
        } else if self.can_send_v4() {
            self.routing_table_v4.write().await.inject_node(node);
        }
    }

    async fn record_node_success(&self, id: &NodeId, addr: SocketAddr) {
        if addr.is_ipv6() {
            self.routing_table_v6.write().await.record_success(id, addr);
        } else {
            self.routing_table_v4.write().await.record_success(id, addr);
        }
    }

    async fn record_node_failure(&self, addr: &SocketAddr) {
        if addr.is_ipv6() {
            self.routing_table_v6.write().await.record_failure(addr);
        } else {
            self.routing_table_v4.write().await.record_failure(addr);
        }
    }

    async fn handle_query(&self, tx_id: Vec<u8>, query: QueryPayload, from: SocketAddr) {
        let sender_id = match &query {
            QueryPayload::Ping { id } => *id,
            QueryPayload::FindNode { id, .. } => *id,
            QueryPayload::GetPeers { id, .. } => *id,
            QueryPayload::AnnouncePeer { id, .. } => *id,
        };

        self.inject_discovered_node(NodeInfo::new(sender_id, from))
            .await;

        let res_msg = match query {
            QueryPayload::Ping { .. } => KRPCMessage::Response {
                transaction_id: tx_id,
                response: ResponsePayload::Ping { id: self.local_id },
            },

            QueryPayload::FindNode { target, want, .. } => {
                let (want_n4, want_n6) = want.effective_for_source(&from);
                let nodes = if want_n4 {
                    self.routing_table_v4.read().await.closest_nodes(&target, 8)
                } else {
                    Vec::new()
                };
                let nodes6 = if want_n6 {
                    self.routing_table_v6.read().await.closest_nodes(&target, 8)
                } else {
                    Vec::new()
                };

                KRPCMessage::Response {
                    transaction_id: tx_id,
                    response: ResponsePayload::FindNode {
                        id: self.local_id,
                        nodes,
                        nodes6,
                    },
                }
            }
            QueryPayload::GetPeers {
                info_hash, want, ..
            } => {
                let (want_n4, want_n6) = want.effective_for_source(&from);
                let token = {
                    let tm = self.token_manager.read().await;
                    tm.generate_token(&from.ip())
                };

                let peers = {
                    let storage = self.peer_storage.read().await;
                    storage.get_peers(&info_hash, want_n4, want_n6, 50)
                };

                // Per BEP 0032: include closest routing nodes when `want` flags are requested or no peers stored
                let include_nodes = peers.is_empty() || want.n4 || want.n6;
                let nodes = if include_nodes && want_n4 {
                    self.routing_table_v4
                        .read()
                        .await
                        .closest_nodes(&info_hash, 8)
                } else {
                    Vec::new()
                };
                let nodes6 = if include_nodes && want_n6 {
                    self.routing_table_v6
                        .read()
                        .await
                        .closest_nodes(&info_hash, 8)
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
                        nodes6,
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
                    tm.validate_token(&from.ip(), &token)
                };

                if !token_valid {
                    KRPCMessage::Error {
                        transaction_id: tx_id,
                        code: 203,
                        message: "Invalid token".to_string(),
                    }
                } else {
                    let peer_port = if implied_port { from.port() } else { port };
                    let peer_addr = SocketAddr::new(from.ip(), peer_port);

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
        let _ = self.send_packet(&bytes, from).await;
    }

    fn next_transaction_id(&self) -> Vec<u8> {
        let val = self.tx_counter.fetch_add(1, Ordering::Relaxed);
        val.to_be_bytes()[2..].to_vec() // 2-byte Transaction ID.
    }
}

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

    #[tokio::test]
    async fn test_two_dht_nodes_ping_and_find_node_v4() {
        let config1 = DHTConfig {
            bind_addr: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
            enable_ipv6: false,
            bootstrap_routers: vec![],
            ..Default::default()
        };
        let (node1, handle1) = DHTNode::start(config1).await.unwrap();

        let config2 = DHTConfig {
            bind_addr: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
            enable_ipv6: false,
            bootstrap_routers: vec![],
            ..Default::default()
        };
        let (node2, handle2) = DHTNode::start(config2).await.unwrap();

        let addr1 = handle1.local_addr().unwrap();

        let pong_id = handle2.ping(addr1).await.expect("ping should succeed");
        assert_eq!(pong_id, handle1.local_id());

        let (total, _) = handle2.routing_table_size().await;
        assert_eq!(total, 1);

        let nodes = handle2
            .find_node(addr1, [99u8; 20])
            .await
            .expect("find_node should succeed");
        assert!(nodes.len() <= 8);

        node1.shutdown();
        node2.shutdown();
    }

    #[tokio::test]
    async fn test_get_peers_and_announce_peer_flow_v4() {
        let config1 = DHTConfig {
            bind_addr: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
            enable_ipv6: false,
            bootstrap_routers: vec![],
            ..Default::default()
        };
        let (node1, handle1) = DHTNode::start(config1).await.unwrap();

        let config2 = DHTConfig {
            bind_addr: SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0),
            enable_ipv6: false,
            bootstrap_routers: vec![],
            ..Default::default()
        };
        let (node2, handle2) = DHTNode::start(config2).await.unwrap();

        let addr1 = handle1.local_addr().unwrap();
        let info_hash = [42u8; 20];

        handle2.ping(addr1).await.unwrap();

        let announced = handle2.announce_peer(info_hash, 6881, false).await;
        assert!(announced >= 1);

        let (peer_tx, mut peer_rx) = mpsc::channel(10);
        let _ = handle2.get_peers(info_hash, peer_tx).await;

        let peer = tokio::time::timeout(Duration::from_millis(500), peer_rx.recv())
            .await
            .expect("should receive peer")
            .expect("peer channel open");

        assert_eq!(peer.port(), 6881);
        node1.shutdown();
        node2.shutdown();
    }

    #[tokio::test]
    async fn test_bep0032_ipv6_only_dht_nodes_announce_and_get_peers() {
        let config1 = DHTConfig {
            bind_addr: SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0),
            bind_addr_v6: Some(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 0, 0, 0)),
            enable_ipv6: true,
            bootstrap_routers: vec![],
            ..Default::default()
        };
        let (node1, handle1) = DHTNode::start(config1).await.unwrap();

        let config2 = DHTConfig {
            bind_addr: SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0),
            bind_addr_v6: Some(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 0, 0, 0)),
            enable_ipv6: true,
            bootstrap_routers: vec![],
            ..Default::default()
        };
        let (node2, handle2) = DHTNode::start(config2).await.unwrap();

        let addr1 = handle1.local_addr().unwrap();
        assert!(addr1.is_ipv6());

        // IPv6 Ping -> populates IPv6 routing table (`nodes6`)
        let pong_id = handle2.ping(addr1).await.expect("IPv6 ping should succeed");
        assert_eq!(pong_id, handle1.local_id());
        assert_eq!(handle2.routing_table_size().await.0, 1);

        // IPv6 AnnouncePeer + GetPeers -> returns 18-byte `[::1]:51413` peer in `values`
        let info_hash = [0x77u8; 20];
        let announced = handle2.announce_peer(info_hash, 51413, false).await;
        assert_eq!(announced, 1);

        let (peer_tx, mut peer_rx) = mpsc::channel(10);
        let _ = handle2.get_peers(info_hash, peer_tx).await;

        let peer = tokio::time::timeout(Duration::from_millis(500), peer_rx.recv())
            .await
            .expect("should receive IPv6 peer")
            .expect("peer channel open");

        assert_eq!(
            peer,
            SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::LOCALHOST, 51413, 0, 0))
        );

        node1.shutdown();
        node2.shutdown();
    }
}
