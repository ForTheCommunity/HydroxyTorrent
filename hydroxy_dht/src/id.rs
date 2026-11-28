use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

use rand::Rng;

use crate::errors::DHTError;

// 20 Byte (160 bits) Node ID
pub type NodeId = [u8; 20];

#[derive(Debug, PartialEq, Clone, Copy, Eq, Hash)]
pub struct NodeInfo {
    pub id: NodeId,
    pub addr: SocketAddr,
}

impl NodeInfo {
    pub fn new(id: NodeId, addr: SocketAddr) -> Self {
        Self { id, addr }
    }

    pub fn new_v4(id: NodeId, v4: SocketAddrV4) -> Self {
        Self {
            id,
            addr: SocketAddr::V4(v4),
        }
    }

    pub fn new_v6(id: NodeId, v6: SocketAddrV6) -> Self {
        Self {
            id,
            addr: SocketAddr::V6(v6),
        }
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
pub fn compare_distance(target: &NodeId, n_1: &NodeId, n_2: &NodeId) -> std::cmp::Ordering {
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

/// Encodes IPv4 NodeInfo entries into Compact Node Info Binary Format (BEP 0005 `nodes`).
/// Each IPv4 node is 26 bytes: Node Id (20) + IPv4 Addr (4) + Port (2, big-endian).
pub fn compact_nodes_encode(nodes: &[NodeInfo]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(nodes.len() * 26);
    for node in nodes {
        if let SocketAddr::V4(v4) = node.addr {
            buf.extend_from_slice(&node.id);
            buf.extend_from_slice(&v4.ip().octets());
            buf.extend_from_slice(&v4.port().to_be_bytes());
        }
    }
    buf
}

/// Decodes Compact IPv4 Node Info Binary Format (BEP 0005 `nodes`, 26 bytes per node).
pub fn compact_nodes_decode(bytes: &[u8]) -> Result<Vec<NodeInfo>, DHTError> {
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
        if port != 0 && !ip_v4.is_unspecified() && !ip_v4.is_broadcast() {
            nodes.push(NodeInfo::new_v4(node_id, SocketAddrV4::new(ip_v4, port)));
        }
    }

    Ok(nodes)
}

/// Encodes IPv6 NodeInfo entries into Compact IPv6 Node Info Binary Format (BEP 0032 `nodes6`).
/// Each IPv6 node is 38 bytes: Node Id (20) + IPv6 Addr (16) + Port (2, big-endian).
pub fn compact_nodes6_encode(nodes: &[NodeInfo]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(nodes.len() * 38);
    for node in nodes {
        if let SocketAddr::V6(v6) = node.addr {
            buf.extend_from_slice(&node.id);
            buf.extend_from_slice(&v6.ip().octets());
            buf.extend_from_slice(&v6.port().to_be_bytes());
        }
    }
    buf
}

/// Decodes Compact IPv6 Node Info Binary Format (BEP 0032 `nodes6`, 38 bytes per node).
pub fn compact_nodes6_decode(bytes: &[u8]) -> Result<Vec<NodeInfo>, DHTError> {
    if bytes.len() % 38 != 0 {
        return Err(DHTError::InvalidCompactNodeInfo(format!(
            "Length: {} is not a multiple of 38",
            bytes.len()
        )));
    }

    let mut nodes = Vec::with_capacity(bytes.len() / 38);
    for chunk in bytes.chunks_exact(38) {
        let mut node_id = [0u8; 20];
        node_id.copy_from_slice(&chunk[..20]);
        let mut octets = [0u8; 16];
        octets.copy_from_slice(&chunk[20..36]);
        let ip_v6 = Ipv6Addr::from(octets);
        let port = u16::from_be_bytes([chunk[36], chunk[37]]);
        if port == 0 || ip_v6.is_unspecified() || ip_v6.is_multicast() {
            continue;
        }
        if let Some(mapped_v4) = ip_v6.to_ipv4_mapped() {
            nodes.push(NodeInfo::new_v4(
                node_id,
                SocketAddrV4::new(mapped_v4, port),
            ));
        } else {
            nodes.push(NodeInfo::new_v6(
                node_id,
                SocketAddrV6::new(ip_v6, port, 0, 0),
            ));
        }
    }

    Ok(nodes)
}

/// Encodes a socket address into compact peer format:
/// - IPv4: 6 bytes (BEP 0005 / BEP 0023)
/// - IPv6: 18 bytes (BEP 0032 / BEP 0007)
pub fn compact_peer_encode(addr: &SocketAddr) -> Vec<u8> {
    match addr {
        SocketAddr::V4(v4) => {
            let mut buf = Vec::with_capacity(6);
            buf.extend_from_slice(&v4.ip().octets());
            buf.extend_from_slice(&v4.port().to_be_bytes());
            buf
        }
        SocketAddr::V6(v6) => {
            if let Some(mapped_v4) = v6.ip().to_ipv4_mapped() {
                let mut buf = Vec::with_capacity(6);
                buf.extend_from_slice(&mapped_v4.octets());
                buf.extend_from_slice(&v6.port().to_be_bytes());
                buf
            } else {
                let mut buf = Vec::with_capacity(18);
                buf.extend_from_slice(&v6.ip().octets());
                buf.extend_from_slice(&v6.port().to_be_bytes());
                buf
            }
        }
    }
}

/// Decodes a compact peer entry:
/// - 6 bytes -> `SocketAddr::V4` (BEP 0005)
/// - 18 bytes -> `SocketAddr::V6` (BEP 0032, normalizing IPv4-mapped addresses to V4)
pub fn compact_peer_decode(enc_bytes: &[u8]) -> Option<SocketAddr> {
    match enc_bytes.len() {
        6 => {
            let ip_v4 = Ipv4Addr::new(enc_bytes[0], enc_bytes[1], enc_bytes[2], enc_bytes[3]);
            let port = u16::from_be_bytes([enc_bytes[4], enc_bytes[5]]);
            if port == 0 {
                return None;
            }
            Some(SocketAddr::V4(SocketAddrV4::new(ip_v4, port)))
        }
        18 => {
            let mut octets = [0u8; 16];
            octets.copy_from_slice(&enc_bytes[0..16]);
            let ip_v6 = Ipv6Addr::from(octets);
            let port = u16::from_be_bytes([enc_bytes[16], enc_bytes[17]]);
            if port == 0 || ip_v6.is_unspecified() || ip_v6.is_multicast() {
                return None;
            }
            if let Some(mapped_v4) = ip_v6.to_ipv4_mapped() {
                Some(SocketAddr::V4(SocketAddrV4::new(mapped_v4, port)))
            } else {
                Some(SocketAddr::V6(SocketAddrV6::new(ip_v6, port, 0, 0)))
            }
        }
        _ => None,
    }
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
    fn test_compact_nodes_v4_and_v6_roundtrip() {
        let node1 = NodeInfo::new_v4([1u8; 20], SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 10), 6881));
        let node2 = NodeInfo::new_v4([2u8; 20], SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 1), 51413));

        let encoded = compact_nodes_encode(&[node1, node2]);
        assert_eq!(encoded.len(), 52);

        let decoded = compact_nodes_decode(&encoded).unwrap();
        assert_eq!(decoded, vec![node1, node2]);

        let v6_ip: Ipv6Addr = "2406:9cc0:200:1521::99".parse().unwrap();
        let node6 = NodeInfo::new_v6([3u8; 20], SocketAddrV6::new(v6_ip, 6881, 0, 0));
        let encoded6 = compact_nodes6_encode(&[node6]);
        assert_eq!(encoded6.len(), 38);
        let decoded6 = compact_nodes6_decode(&encoded6).unwrap();
        assert_eq!(decoded6, vec![node6]);
    }

    #[test]
    fn test_compact_peer_v4_and_v6_roundtrip() {
        let addr4 = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080));
        let encoded4 = compact_peer_encode(&addr4);
        assert_eq!(encoded4.len(), 6);
        assert_eq!(compact_peer_decode(&encoded4), Some(addr4));

        let v6_ip: Ipv6Addr = "2406:9cc0:200:1521::42".parse().unwrap();
        let addr6 = SocketAddr::V6(SocketAddrV6::new(v6_ip, 51413, 0, 0));
        let encoded6 = compact_peer_encode(&addr6);
        assert_eq!(encoded6.len(), 18);
        assert_eq!(compact_peer_decode(&encoded6), Some(addr6));
    }
}
