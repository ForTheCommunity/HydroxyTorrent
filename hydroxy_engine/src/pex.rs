use std::{
    collections::BTreeMap,
    net::Ipv4Addr,
};

use hydroxy_bencode::{
    b_dict::BDict,
    decode::{decode, BencodedValue},
    encode::encode,
};
use hydroxy_tracker::Peer;

/// Standard BEP 10 Extended Message ID for the extension handshake.
pub const EXTENSION_HANDSHAKE_ID: u8 = 0;

/// Our local extended message ID mapped to "ut_pex".
pub const UT_PEX_EXT_ID: u8 = 1;

/// Name of the PEX extension in the BEP 10 handshake dictionary.
pub const UT_PEX_NAME: &[u8] = b"ut_pex";

/// Build the BEP 10 Extended Handshake payload dictionary.
/// Advertises our supported extensions (specifically `ut_pex -> 1`) and client version.
pub fn build_extended_handshake() -> Vec<u8> {
    let mut m_dict = BTreeMap::new();
    m_dict.insert(UT_PEX_NAME.to_vec(), BencodedValue::Integer(UT_PEX_EXT_ID as i64));

    let mut root = BTreeMap::new();
    root.insert(b"m".to_vec(), BencodedValue::Dictionary(m_dict));
    root.insert(b"v".to_vec(), BencodedValue::String(b"HydroxyTorrent 1.0".to_vec()));

    encode(&BencodedValue::Dictionary(root))
}

/// Parse the remote peer's extended message ID for `ut_pex` from an Extended Handshake payload.
pub fn parse_remote_ut_pex_id(payload: &[u8]) -> Option<u8> {
    let value = decode(payload).ok()?;
    if let BencodedValue::Dictionary(dict) = value {
        let bdict = BDict::new(&dict);
        let m_dict = bdict.get_dict(b"m")?;
        let ut_pex_id = m_dict.get_int(UT_PEX_NAME)?;
        if ut_pex_id > 0 && ut_pex_id <= 255 {
            return Some(ut_pex_id as u8);
        }
    }
    None
}

/// Parse the remote peer's client version string from BEP 10 Extended Handshake (`v` key).
pub fn parse_remote_client_version(payload: &[u8]) -> Option<String> {
    let value = decode(payload).ok()?;
    if let BencodedValue::Dictionary(dict) = value {
        let bdict = BDict::new(&dict);
        if let Some(v_bytes) = bdict.get_string(b"v") {
            if let Ok(v_str) = std::str::from_utf8(&v_bytes) {
                let trimmed = v_str.trim();
                if !trimmed.is_empty() && trimmed.chars().all(|c| !c.is_control()) {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

/// Build a BEP 11 Peer Exchange (PEX) message payload.
/// Compact 6-byte IPv4 addresses (4 bytes IP + 2 bytes port).
pub fn build_pex_message(
    added_peers: &[(Ipv4Addr, u16)],
    dropped_peers: &[(Ipv4Addr, u16)],
) -> Vec<u8> {
    let mut root = BTreeMap::new();

    let mut added_bytes = Vec::with_capacity(added_peers.len() * 6);
    let mut added_flags = Vec::with_capacity(added_peers.len());
    for &(ip, port) in added_peers {
        added_bytes.extend_from_slice(&ip.octets());
        added_bytes.extend_from_slice(&port.to_be_bytes());
        // 0x00 = standard connection (0x01 = encryption preferred, 0x02 = seed)
        added_flags.push(0x00);
    }

    root.insert(b"added".to_vec(), BencodedValue::String(added_bytes));
    root.insert(b"added.f".to_vec(), BencodedValue::String(added_flags));

    if !dropped_peers.is_empty() {
        let mut dropped_bytes = Vec::with_capacity(dropped_peers.len() * 6);
        for &(ip, port) in dropped_peers {
            dropped_bytes.extend_from_slice(&ip.octets());
            dropped_bytes.extend_from_slice(&port.to_be_bytes());
        }
        root.insert(b"dropped".to_vec(), BencodedValue::String(dropped_bytes));
    }

    encode(&BencodedValue::Dictionary(root))
}

/// Parse newly discovered peers from an incoming BEP 11 PEX payload.
pub fn parse_pex_added_peers(payload: &[u8]) -> Vec<Peer> {
    let mut peers = Vec::new();
    let Ok(value) = decode(payload) else {
        return peers;
    };

    if let BencodedValue::Dictionary(dict) = value {
        let bdict = BDict::new(&dict);
        if let Some(added) = bdict.get_string(b"added") {
            for chunk in added.chunks_exact(6) {
                let ip = Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]);
                let port = u16::from_be_bytes([chunk[4], chunk[5]]);

                // Validate: reject 0.0.0.0, broadcast (255.255.255.255), loopback, and port 0
                if !ip.is_unspecified() && !ip.is_broadcast() && !ip.is_loopback() && port != 0 {
                    peers.push(Peer {
                        ipv4_addr: ip,
                        port,
                    });
                }
            }
        }
    }

    peers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extended_handshake_roundtrip() {
        let handshake = build_extended_handshake();
        let remote_id = parse_remote_ut_pex_id(&handshake);
        assert_eq!(remote_id, Some(UT_PEX_EXT_ID));
    }

    #[test]
    fn test_pex_message_roundtrip() {
        let peers = vec![
            (Ipv4Addr::new(192, 168, 1, 100), 6881),
            (Ipv4Addr::new(10, 0, 0, 5), 51413),
        ];
        let dropped = vec![(Ipv4Addr::new(172, 16, 0, 1), 8999)];

        let pex_msg = build_pex_message(&peers, &dropped);
        let parsed = parse_pex_added_peers(&pex_msg);

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].ipv4_addr, Ipv4Addr::new(192, 168, 1, 100));
        assert_eq!(parsed[0].port, 6881);
        assert_eq!(parsed[1].ipv4_addr, Ipv4Addr::new(10, 0, 0, 5));
        assert_eq!(parsed[1].port, 51413);
    }
}
