use std::time::{Duration, Instant};

use crate::id::{NodeId, NodeInfo};

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
