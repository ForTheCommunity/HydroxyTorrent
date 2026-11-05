use std::net::SocketAddr;

use rand::Rng;

// 20 Byte (160 bits) Node ID
pub type NodeId = [u8; 20];

pub struct NodeInfo {
    pub id: NodeId,
    pub addr: SocketAddr,
}

impl NodeInfo {
    pub fn new(id: NodeId, addr: SocketAddr) -> Self {
        Self { id, addr }
    }
}

/// Generate Random Node ID.
pub fn generate_random_id() -> NodeId {
    let mut id = [0u8; 20];
    rand::rng().fill_bytes(&mut id);
    id
}

/// Calculate Kademlia XOR Distance between two 160-bits IDs / Nodes.
pub fn xor_distance(n_1: &NodeId, n_2: &NodeId) -> NodeId {
    let mut result = [0u8; 20];
    for i in 0..20 {
        result[i] = n_1[i] ^ n_2[i];
    }
    result
}

/// Compares the distance of 'Node A' to 'Target' Versus 'Node B' to 'Target'.
/// Case - 1 => Returns Ordering::Less if 'Node A' is closer to 'Target' than 'Node B'.
/// Case - 2 => Returns Ordering::Greator if 'Node B' is closer to 'Target' than 'Node B'.
/// Case - 3 => Returns Equal if Distance is same, which is impossible, this only happens if 'Node A' == 'Node B'.
pub fn compare_distance(target: &NodeId, n_1: &NodeId, n_2: &NodeId) -> std::cmp::Ordering {
    // comparing their XOR results from the most significant
    // byte (index 0) to the least significant byte (index 19).
    for i in 0..20 {
        let n1_distance = n_1[i] ^ target[i];
        let n2_distance = n_2[i] ^ target[i];
        if n1_distance != n2_distance {
            return n1_distance.cmp(&n2_distance);
        }
    }
    std::cmp::Ordering::Equal
}
