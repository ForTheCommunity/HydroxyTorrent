use std::{
    collections::HashMap,
    net::SocketAddrV4,
    time::{Duration, Instant},
};

use crate::id::NodeId;

const DEFAULT_PEER_EXPIRY: Duration = Duration::from_mins(30);

pub struct PeerStorage {
    peers: HashMap<NodeId, HashMap<SocketAddrV4, Instant>>,
}

impl PeerStorage {
    /// Get a new instance of PeerStorage
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    /// Store a Peer for a InfoHash
    pub fn store_peer(&mut self, info_hash: NodeId, peer: SocketAddrV4) {
        self.peers
            .entry(info_hash)
            .or_default()
            .insert(peer, Instant::now());
    }

    /// Retrieve up to limit active peers for a info_hash
    pub fn get_peers(&self, info_hash: &NodeId, limit: usize) -> Vec<SocketAddrV4> {
        let entry = match self.peers.get(info_hash) {
            Some(e) => e,
            None => return Vec::new(),
        };

        let now = Instant::now();
        entry
            .iter()
            .filter(|&(_, &inserted)| now.duration_since(inserted) <= DEFAULT_PEER_EXPIRY)
            .take(limit)
            .map(|(&addr, _)| addr)
            .collect()
    }

    /// Clean up peers older than Max Age.
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
    use std::net::Ipv4Addr;

    #[test]
    fn test_peer_storage_and_pruning() {
        let mut storage = PeerStorage::new();
        let hash = [5u8; 20];
        let peer1 = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 1), 6881);
        let peer2 = SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 2), 6882);

        storage.store_peer(hash, peer1);
        storage.store_peer(hash, peer2);

        let peers = storage.get_peers(&hash, 10);
        assert_eq!(peers.len(), 2);

        // Pruning with 0 max_age should remove all
        storage.prune_expired(Duration::from_millis(0));
        assert_eq!(storage.get_peers(&hash, 10).len(), 0);
    }
}
