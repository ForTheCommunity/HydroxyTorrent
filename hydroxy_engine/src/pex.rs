use std::{collections::BTreeMap, net::IpAddr};

use hydroxy_bencode::{
    b_dict::BDict,
    decode::{BencodedValue, decode},
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
    m_dict.insert(
        UT_PEX_NAME.to_vec(),
        BencodedValue::Integer(UT_PEX_EXT_ID as i64),
    );

    let mut root = BTreeMap::new();
    root.insert(b"m".to_vec(), BencodedValue::Dictionary(m_dict));
    root.insert(
        b"v".to_vec(),
        BencodedValue::String(b"HydroxyTorrent 1.0".to_vec()),
    );

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

/// Build a BEP 11 Peer Exchange (PEX) message payload supporting both:
/// - IPv4 peers (`added` / `added.f` / `dropped` — 6 bytes per peer)
/// - IPv6 peers (`added6` / `added6.f` / `dropped6` — 18 bytes per peer)
pub fn build_pex_message(
    added_peers: &[(IpAddr, u16)],
    dropped_peers: &[(IpAddr, u16)],
) -> Vec<u8> {
    let mut root = BTreeMap::new();

    let mut added_v4 = Vec::new();
    let mut added_v4_flags = Vec::new();
    let mut added_v6 = Vec::new();
    let mut added_v6_flags = Vec::new();

    for &(ip, port) in added_peers {
        match Peer::normalize_ip(ip) {
            IpAddr::V4(v4) => {
                added_v4.extend_from_slice(&v4.octets());
                added_v4.extend_from_slice(&port.to_be_bytes());
                added_v4_flags.push(0x00);
            }
            IpAddr::V6(v6) => {
                added_v6.extend_from_slice(&v6.octets());
                added_v6.extend_from_slice(&port.to_be_bytes());
                added_v6_flags.push(0x00);
            }
        }
    }

    root.insert(b"added".to_vec(), BencodedValue::String(added_v4));
    root.insert(b"added.f".to_vec(), BencodedValue::String(added_v4_flags));

    if !added_v6.is_empty() {
        root.insert(b"added6".to_vec(), BencodedValue::String(added_v6));
        root.insert(b"added6.f".to_vec(), BencodedValue::String(added_v6_flags));
    }

    let mut dropped_v4 = Vec::new();
    let mut dropped_v6 = Vec::new();
    for &(ip, port) in dropped_peers {
        match Peer::normalize_ip(ip) {
            IpAddr::V4(v4) => {
                dropped_v4.extend_from_slice(&v4.octets());
                dropped_v4.extend_from_slice(&port.to_be_bytes());
            }
            IpAddr::V6(v6) => {
                dropped_v6.extend_from_slice(&v6.octets());
                dropped_v6.extend_from_slice(&port.to_be_bytes());
            }
        }
    }

    if !dropped_v4.is_empty() {
        root.insert(b"dropped".to_vec(), BencodedValue::String(dropped_v4));
    }
    if !dropped_v6.is_empty() {
        root.insert(b"dropped6".to_vec(), BencodedValue::String(dropped_v6));
    }

    encode(&BencodedValue::Dictionary(root))
}

/// Parse newly discovered IPv4 (`added`) and IPv6 (`added6`) peers from an incoming BEP 11 PEX payload.
pub fn parse_pex_added_peers(payload: &[u8]) -> Vec<Peer> {
    let mut peers = Vec::new();
    let Ok(value) = decode(payload) else {
        return peers;
    };

    if let BencodedValue::Dictionary(dict) = value {
        let bdict = BDict::new(&dict);
        if let Some(added) = bdict.get_string(b"added") {
            peers.extend(Peer::parse_peers(&added));
        }
        if let Some(added6) = bdict.get_string(b"added6") {
            peers.extend(Peer::parse_peers6(&added6));
        }
    }

    peers
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_extended_handshake_roundtrip() {
        let handshake = build_extended_handshake();
        let remote_id = parse_remote_ut_pex_id(&handshake);
        assert_eq!(remote_id, Some(UT_PEX_EXT_ID));
    }

    #[test]
    fn test_pex_message_roundtrip_dual_stack() {
        let v6_peer: Ipv6Addr = "2406:9cc0:200:1521::42".parse().unwrap();
        let peers = vec![
            (IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 6881),
            (IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)), 51413),
            (IpAddr::V6(v6_peer), 55000),
        ];
        let dropped = vec![
            (IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1)), 8999),
            (IpAddr::V6(v6_peer), 55001),
        ];

        let pex_msg = build_pex_message(&peers, &dropped);
        let parsed = parse_pex_added_peers(&pex_msg);

        assert_eq!(parsed.len(), 3);
        assert_eq!(
            parsed[0].ip_addr,
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100))
        );
        assert_eq!(parsed[0].port, 6881);
        assert_eq!(parsed[1].ip_addr, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)));
        assert_eq!(parsed[1].port, 51413);
        assert_eq!(parsed[2].ip_addr, IpAddr::V6(v6_peer));
        assert_eq!(parsed[2].port, 55000);
    }
}
