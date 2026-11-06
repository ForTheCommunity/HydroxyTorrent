use std::net::{Ipv4Addr, SocketAddrV4};

use rand::Rng;

use crate::errors::DHTError;

// 20 Byte (160 bits) Node ID
pub type NodeId = [u8; 20];

#[derive(Debug, PartialEq, Clone, Copy, Eq, Hash)]
pub struct NodeInfo {
    pub id: NodeId,
    pub ip_v4_addr: SocketAddrV4,
}

impl NodeInfo {
    pub fn new(id: NodeId, ip_v4_addr: SocketAddrV4) -> Self {
        Self { id, ip_v4_addr }
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

/// Returns the number of leading identical bits (common prefix length) between two IDs.
pub fn common_prefix_len(n_1: &NodeId, n_2: &NodeId) -> usize {
    for i in 0..20 {
        let x = n_1[i] ^ n_2[i];
        if x != 0 {
            return i * 8 + x.leading_zeros() as usize;
        }
    }
    160
}

/// Encodes a list of NodeInfo into Compact Node Info Binary Format (BEP-5).
/// Each Node is represented as 26 Bytes.
/// Node Id (20 Bytes) + IPv4 Addr (4 Bytes) + Port (2 Bytes) [big-endian]
pub fn compact_nodes_encode(nodes: &[NodeInfo]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(nodes.len() * 26);
    for node in nodes {
        buf.extend_from_slice(&node.id);
        buf.extend_from_slice(&node.ip_v4_addr.ip().octets());
        buf.extend_from_slice(&node.ip_v4_addr.port().to_be_bytes());
    }
    buf
}

/// Decodes Compact Node Info Binary Format into a list of NodeInfo (BEP-5).
pub fn compact_nodes_decode(bytes: &[u8]) -> Result<Vec<NodeInfo>, DHTError> {
    // each Encoded NodeInfo has exact length of 26, so overall length also should be multiple of 26
    if bytes.len() % 26 != 0 {
        return Err(DHTError::InvalidCompactNodeInfo(format!(
            "Length: {} is not a multiple of 26",
            bytes.len()
        )));
    }

    let mut nodes = Vec::with_capacity(bytes.len() / 26);
    for chunk in bytes.chunks_exact(26) {
        let mut node_id = [0u8; 20];
        node_id.copy_from_slice(&chunk[..20]);
        let ip_v4 = Ipv4Addr::new(chunk[20], chunk[21], chunk[22], chunk[23]);
        let port = u16::from_be_bytes([chunk[24], chunk[25]]);
        nodes.push(NodeInfo {
            id: node_id,
            ip_v4_addr: SocketAddrV4::new(ip_v4, port),
        });
    }

    Ok(nodes)
}

/// Encodes an IPv4 socket address into 6-byte compact peer format (BEP 5 / 23).
pub fn compact_peer_encode(ip_v4_addr: &SocketAddrV4) -> [u8; 6] {
    let mut buf = [0u8; 6];
    buf[..4].copy_from_slice(&ip_v4_addr.ip().octets());
    buf[4..].copy_from_slice(&ip_v4_addr.port().to_be_bytes());
    buf
}

/// Decodes encoded 6-bytes IPv4 Addr (BEP 5 / 23).
pub fn compact_peer_decode(enc_bytes: &[u8; 6]) -> Option<SocketAddrV4> {
    if enc_bytes.len() != 6 {
        return None;
    }

    let ip_v4_addr = Ipv4Addr::new(enc_bytes[0], enc_bytes[1], enc_bytes[2], enc_bytes[3]);
    let port = u16::from_be_bytes([enc_bytes[4], enc_bytes[5]]);
    if port == 0 {
        return None;
    }

    Some(SocketAddrV4::new(ip_v4_addr, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_and_compare() {
        let mut target = [0u8; 20];
        target[0] = 0x80;

        let mut a = [0u8; 20];
        a[0] = 0x81; // diff = 1

        let mut b = [0u8; 20];
        b[0] = 0x90; // diff = 0x10

        assert_eq!(compare_distance(&target, &a, &b), std::cmp::Ordering::Less);
        assert_eq!(
            compare_distance(&target, &b, &a),
            std::cmp::Ordering::Greater
        );
        assert_eq!(compare_distance(&target, &a, &a), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_common_prefix_len() {
        let mut a = [0u8; 20];
        let mut b = [0u8; 20];

        assert_eq!(common_prefix_len(&a, &b), 160);

        a[0] = 0b1000_0000;
        b[0] = 0b0000_0000;
        assert_eq!(common_prefix_len(&a, &b), 0);

        a[0] = 0b1100_0000;
        b[0] = 0b1110_0000;
        assert_eq!(common_prefix_len(&a, &b), 2);
    }

    #[test]
    fn test_compact_nodes_roundtrip() {
        let node1 = NodeInfo {
            id: [1u8; 20],
            ip_v4_addr: SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 10), 6881),
        };
        let node2 = NodeInfo {
            id: [2u8; 20],
            ip_v4_addr: SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), 51413),
        };

        let encoded = compact_nodes_encode(&[node1, node2]);
        assert_eq!(encoded.len(), 52);

        let decoded = compact_nodes_decode(&encoded).unwrap();
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0], node1);
        assert_eq!(decoded[1], node2);
    }

    #[test]
    fn test_compact_peer_roundtrip() {
        let addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);
        let encoded = compact_peer_encode(&addr);
        let decoded = compact_peer_decode(&encoded).unwrap();
        assert_eq!(decoded, addr);
    }
}
