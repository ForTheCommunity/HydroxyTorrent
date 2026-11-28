use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

use crate::id::{NodeId, NodeInfo, compare_distance};

pub const BUCKET_SIZE: usize = 8;
const MAX_FAILURES: u32 = 2;
const QUESTIONABLE_TIMEOUT: Duration = Duration::from_mins(15);

#[derive(Debug, Clone)]
pub struct NodeEntry {
    pub node_info: NodeInfo,
    // Timestamp of last successfull contact.
    pub last_seen: Instant,
    // Number of consecutive failed/timed-out queries..
    pub fail_count: u32,
}

impl NodeEntry {
    pub fn new(node_info: NodeInfo) -> Self {
        Self {
            node_info,
            last_seen: Instant::now(),
            fail_count: 0,
        }
    }

    pub fn is_bad(&self) -> bool {
        self.fail_count >= MAX_FAILURES
    }

    pub fn is_good(&self) -> bool {
        !self.is_bad() && self.last_seen.elapsed() < QUESTIONABLE_TIMEOUT
    }
}

// A BitTorrent DHT NodeId is 160 bits long (20 bytes).
// The entire DHT keyspace consists of all possible 2^160 IDs.
//
// The prefix: Vec<bool> represents a binary bit prefix that defines which slice of the 160-bit keyspace this bucket
// is responsible for.
//
// • prefix: vec![] (length 0): Matches all IDs (the entire 2¹⁶⁰ keyspace).
// • prefix: vec![false] (i.e. bit 0): Matches all IDs whose very first bit is 0 (the lower half of the keyspace, [0,
// 2¹⁵⁹ - 1]).
// • prefix: vec![true] (i.e. bit 1): Matches all IDs whose very first bit is 1 (the upper half, [2¹⁵⁹, 2¹⁶⁰ - 1]).
// • prefix: vec![true, false] (bits 10): Matches IDs starting with 10....
//
// Every node stored in self.nodes must have an ID whose first bits match prefix.
// ──────
// 2. How prefix changes during Bucket Splitting
//
// When the routing table is created:
// 1. It starts with one bucket whose prefix is empty: prefix = []. It accepts any node ID.
// 2. Once that bucket fills with 8 nodes and needs to split, it is replaced by two new buckets:
//     • Bucket 1 gets prefix = [false] (representing bit 0).
//     • Bucket 2 gets prefix = [true] (representing bit 1).
// 3. If the bucket matching our own local ID fills up again, it splits further:
//     • For example, [true] splits into [true, false] and [true, true].

#[derive(Debug, Clone)]
pub struct KBucket {
    // prefix
    pub prefix: Vec<bool>,
    pub nodes: Vec<NodeEntry>,
    pub last_updated: Instant,
}

impl KBucket {
    pub fn new(prefix: Vec<bool>) -> Self {
        Self {
            prefix,
            nodes: Vec::with_capacity(BUCKET_SIZE),
            last_updated: Instant::now(),
        }
    }

    pub fn matches(&self, id: &NodeId) -> bool {
        for (i, &bit) in self.prefix.iter().enumerate() {
            let byte_idx = i / 8;
            let bit_idx = 7 - (i % 8);
            let id_bit = (id[byte_idx] >> bit_idx) & 1 == 1;
            if id_bit != bit {
                return false;
            }
        }
        true
    }
}

pub struct RoutingTable {
    /// Node ID of ourself
    self_id: NodeId,
    k_buckets: Vec<KBucket>,
}

impl RoutingTable {
    pub fn new(self_id: NodeId) -> Self {
        Self {
            self_id,
            k_buckets: vec![KBucket::new(Vec::new())],
        }
    }

    pub fn self_id(&self) -> &NodeId {
        &self.self_id
    }

    /// Insert or update a Node in the Routing Table.
    pub fn inject_node(&mut self, node_info: NodeInfo) -> bool {
        if node_info.id == self.self_id {
            // we shouldn't insert our own Node into our Routing Table.
            return false;
        }

        let bucket_index = match self.find_bucket_index(&node_info.id) {
            Some(index) => index,
            None => return false,
        };

        // If the	node is	already	present, its IP/port is updated,
        // its fail_count is reset to 0 (it is healthy again),
        // last_seen is updated to now, and the bucket's timestamp is refreshed.
        if let Some(entry) = self.k_buckets[bucket_index]
            .nodes
            .iter_mut()
            .find(|n| n.node_info.id == node_info.id)
        {
            entry.node_info.addr = node_info.addr;
            entry.last_seen = Instant::now();
            entry.fail_count = 0;
            self.k_buckets[bucket_index].last_updated = Instant::now();
            return true;
        }

        // if bucket is not full then inject/insert the node.
        if self.k_buckets[bucket_index].nodes.len() < BUCKET_SIZE {
            self.k_buckets[bucket_index]
                .nodes
                .push(NodeEntry::new(node_info));
            self.k_buckets[bucket_index].last_updated = Instant::now();
            return true;
        }

        // if bucket is full then check if we split it or not...
        let can_split = self.k_buckets[bucket_index].matches(&self.self_id)
            && self.k_buckets[bucket_index].prefix.len() < 160;

        if can_split {
            self.split_bucket(bucket_index);
            return self.inject_node(node_info);
        }

        // Otherwise, replace a known bad node if one exists
        if let Some(bad_pos) = self.k_buckets[bucket_index]
            .nodes
            .iter()
            .position(|n| n.is_bad())
        {
            self.k_buckets[bucket_index].nodes[bad_pos] = NodeEntry::new(node_info);
            self.k_buckets[bucket_index].last_updated = Instant::now();
            return true;
        }

        false
    }

    /// Record a successful response from a node.
    pub fn record_success(&mut self, id: &NodeId, addr: SocketAddr) {
        self.inject_node(NodeInfo::new(*id, addr));
    }

    /// Record a failure (timeout / bad packet) from a node address.
    pub fn record_failure(&mut self, addr: &SocketAddr) {
        for bucket in &mut self.k_buckets {
            for entry in &mut bucket.nodes {
                if entry.node_info.addr == *addr {
                    entry.fail_count = entry.fail_count.saturating_add(1);
                    return;
                }
            }
        }
    }

    /// Get the `count` closest nodes to the target ID, sorted by distance.
    pub fn closest_nodes(&self, target: &NodeId, count: usize) -> Vec<NodeInfo> {
        let mut all_nodes: Vec<NodeInfo> = self
            .k_buckets
            .iter()
            .flat_map(|b| b.nodes.iter())
            .filter(|n| !n.is_bad())
            .map(|n| n.node_info)
            .collect();

        all_nodes.sort_by(|a, b| compare_distance(target, &a.id, &b.id));

        if all_nodes.len() > count {
            all_nodes.truncate(count);
        }
        all_nodes
    }

    /// Total number of nodes in the routing table.
    pub fn total_nodes(&self) -> usize {
        self.k_buckets.iter().map(|b| b.nodes.len()).sum()
    }

    /// Total number of good nodes in the routing table.
    pub fn good_nodes(&self) -> usize {
        self.k_buckets
            .iter()
            .flat_map(|b| b.nodes.iter())
            .filter(|n| n.is_good())
            .count()
    }

    /// Return all good/questionable nodes currently in the routing table.
    pub fn all_nodes(&self) -> Vec<NodeInfo> {
        self.k_buckets
            .iter()
            .flat_map(|b| b.nodes.iter())
            .filter(|n| !n.is_bad())
            .map(|n| n.node_info)
            .collect()
    }

    /// Find IDs of buckets that have not been updated within `threshold` (for refreshing).
    pub fn stale_buckets(&self, threshold: Duration) -> Vec<NodeId> {
        let mut refresh_ids = Vec::new();
        for bucket in &self.k_buckets {
            if bucket.last_updated.elapsed() >= threshold {
                // Generate a sample ID with this bucket's prefix
                let mut id = [0u8; 20];
                for (i, &bit) in bucket.prefix.iter().enumerate() {
                    let byte_idx = i / 8;
                    let bit_idx = 7 - (i % 8);
                    if bit {
                        id[byte_idx] |= 1 << bit_idx;
                    }
                }
                refresh_ids.push(id);
            }
        }
        refresh_ids
    }

    fn find_bucket_index(&self, id: &NodeId) -> Option<usize> {
        self.k_buckets.iter().position(|b| b.matches(id))
    }

    fn split_bucket(&mut self, idx: usize) {
        let old_bucket = self.k_buckets.remove(idx);
        let mut prefix_0 = old_bucket.prefix.clone();
        prefix_0.push(false);

        let mut prefix_1 = old_bucket.prefix;
        prefix_1.push(true);

        let mut bucket_0 = KBucket::new(prefix_0);
        let mut bucket_1 = KBucket::new(prefix_1);

        for node in old_bucket.nodes {
            if bucket_0.matches(&node.node_info.id) {
                bucket_0.nodes.push(node);
            } else {
                bucket_1.nodes.push(node);
            }
        }

        self.k_buckets.insert(idx, bucket_0);
        self.k_buckets.insert(idx + 1, bucket_1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[test]
    fn test_routing_table_insert_and_closest() {
        let mut local_id = [0u8; 20];
        local_id[0] = 0x00;

        let mut rt = RoutingTable::new(local_id);

        for i in 1..=20 {
            let mut id = [0u8; 20];
            id[0] = i as u8;
            let info = NodeInfo::new_v4(
                id,
                SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, i as u8), 6881),
            );
            rt.inject_node(info);
        }

        assert!(rt.total_nodes() >= 8);

        let mut target = [0u8; 20];
        target[0] = 0x01; // Should find node 1 as closest

        let closest = rt.closest_nodes(&target, 3);
        assert_eq!(closest.len(), 3);
        assert_eq!(closest[0].id[0], 0x01);
    }

    #[test]
    fn test_bad_node_replacement() {
        let mut rt = RoutingTable::new([0u8; 20]);

        // Fill bucket with 8 nodes that don't match local_id prefix so it won't split
        for i in 1..=8 {
            let mut id = [0u8; 20];
            id[0] = 0x80 | i as u8;
            let info = NodeInfo::new_v4(
                id,
                SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, i as u8), 6881),
            );
            rt.inject_node(info);
        }

        assert_eq!(rt.total_nodes(), 8);

        // Fail node 1 twice so it becomes bad
        let bad_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 1), 6881));
        rt.record_failure(&bad_addr);
        rt.record_failure(&bad_addr);

        // Now insert 9th node - it should replace the bad node
        let mut new_id = [0u8; 20];
        new_id[0] = 0x80 | 9;
        let new_info = NodeInfo::new_v4(
            new_id,
            SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 9), 6881),
        );
        let inserted = rt.inject_node(new_info);
        assert!(inserted);
        assert_eq!(rt.total_nodes(), 8);

        // Node 1 should be gone
        let all = rt.all_nodes();
        assert!(!all.iter().any(|n| n.addr == bad_addr));
        assert!(all.iter().any(|n| n.id == new_id));
    }

    #[test]
    fn test_stale_buckets_detection() {
        let rt = RoutingTable::new([0u8; 20]);
        // Fresh bucket shouldn't be stale with 15min threshold
        assert_eq!(rt.stale_buckets(Duration::from_secs(900)).len(), 0);

        // With 0 threshold, all buckets are stale
        let stale = rt.stale_buckets(Duration::from_millis(0));
        assert_eq!(stale.len(), 1);
    }
}
