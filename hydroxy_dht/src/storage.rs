use std::{
    collections::HashMap,
    net::SocketAddr,
    time::{Duration, Instant},
};

use crate::id::NodeId;

const DEFAULT_PEER_EXPIRY: Duration = Duration::from_mins(30);

pub struct PeerStorage {
    peers: HashMap<NodeId, HashMap<SocketAddr, Instant>>,
}

impl PeerStorage {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    /// Store an announced IPv4 or IPv6 peer for `info_hash`.
    pub fn store_peer(&mut self, info_hash: NodeId, peer: SocketAddr) {
        self.peers
            .entry(info_hash)
            .or_default()
            .insert(peer, Instant::now());
    }

    /// Retrieve up to `limit` active peers matching the requested address families (`want_v4` / `want_v6`).
    pub fn get_peers(
        &self,
        info_hash: &NodeId,
        want_v4: bool,
        want_v6: bool,
        limit: usize,
    ) -> Vec<SocketAddr> {
        let entry = match self.peers.get(info_hash) {
            Some(e) => e,
            None => return Vec::new(),
        };

        let now = Instant::now();
        entry
            .iter()
            .filter(|&(addr, &inserted)| {
                now.duration_since(inserted) <= DEFAULT_PEER_EXPIRY
                    && ((want_v4 && addr.is_ipv4()) || (want_v6 && addr.is_ipv6()))
            })
            .take(limit)
            .map(|(&addr, _)| addr)
            .collect()
    }

    /// Clean up peers older than `max_age`.
    pub fn prune_expired(&mut self, max_age: Duration) {
        let now = Instant::now();
        self.peers.retain(|_, map| {
            map.retain(|_, inserted| now.duration_since(*inserted) <= max_age);
            !map.is_empty()
        });
    }

    /// Total number of tracked info_hashes.
    pub fn tracked_hashes_count(&self) -> usize {
        self.peers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};

    #[test]
    fn test_peer_storage_dual_stack_and_pruning() {
        let mut storage = PeerStorage::new();
        let hash = [5u8; 20];
        let peer4 = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 1), 6881));
        let peer6 = SocketAddr::V6(SocketAddrV6::new(
            "2406:9cc0:200:1521::1".parse::<Ipv6Addr>().unwrap(),
            6882,
            0,
            0,
        ));

        storage.store_peer(hash, peer4);
        storage.store_peer(hash, peer6);

        assert_eq!(storage.get_peers(&hash, true, false, 10), vec![peer4]);
        assert_eq!(storage.get_peers(&hash, false, true, 10), vec![peer6]);
        assert_eq!(storage.get_peers(&hash, true, true, 10).len(), 2);

        storage.prune_expired(Duration::from_millis(0));
        assert_eq!(storage.get_peers(&hash, true, true, 10).len(), 0);
    }
}
