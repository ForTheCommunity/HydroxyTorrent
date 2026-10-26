/// Parses a 20-byte BitTorrent Peer ID into a human-readable client name and version.
/// Supports Azureus-style (BEP 0020), BitComet, and other client naming conventions.
pub fn parse_peer_id(peer_id: &[u8; 20]) -> String {
    // Azureus-style: '-<2-char ID><4-char version>-<12 random bytes>'
    if peer_id[0] == b'-' && peer_id[7] == b'-' {
        let client_code = &peer_id[1..3];
        let ver_bytes = [peer_id[3], peer_id[4], peer_id[5], peer_id[6]];
        let client_name = match client_code {
            b"OH" | b"HT" | b"HY" => "HydroxyTorrent",
            b"qB" => "qBittorrent",
            b"TR" => "Transmission",
            b"UT" => "µTorrent",
            b"BT" => "BitTorrent",
            b"DE" => "Deluge",
            b"LT" => "libtorrent (Rasterbar)",
            b"lt" => "libtorrent (rakshasa)",
            b"AZ" => "Azureus / Vuze",
            b"BI" => "BiglyBT",
            b"BC" => "BitComet",
            b"KT" => "KTorrent",
            b"WD" => "WebTorrent",
            b"FD" => "Free Download Manager",
            b"PI" => "PicoTorrent",
            b"AR" => "Arctic",
            b"AT" => "Artemis",
            b"AV" => "Avicora",
            b"BG" => "BTG",
            b"BR" => "BitRocket",
            b"CT" => "CTorrent",
            b"CD" => "Enhanced CTorrent",
            b"FC" => "FileCroc",
            b"FT" => "FoxTorrent",
            b"HL" => "Halite",
            b"HN" => "Hydranode",
            b"IL" => "iLivid",
            b"JS" => "JustTorrent",
            b"KG" => "KGet",
            b"LP" => "Lphant",
            b"MG" => "MediaGet",
            b"ML" => "MLDonkey",
            b"MO" => "MonoTorrent",
            b"MR" => "Miro",
            b"MT" => "MoonlightTorrent",
            b"NX" => "Net Transport",
            b"OS" => "OneSwarm",
            b"OT" => "OmegaTorrent",
            b"PD" => "Pando",
            b"QD" => "QQDownload",
            b"QT" => "Qt 4 Torrent",
            b"RT" => "Retriever",
            b"RZ" => "RezTorrent",
            b"SS" => "SwarmScope",
            b"SZ" => "Shareaza",
            b"TN" => "TorrentDotNET",
            b"TS" => "Torrentstorm",
            b"TT" => "TuoTu",
            b"UL" => "uLeecher!",
            b"VG" => "Vagaa",
            b"WT" => "BitLet",
            b"WY" => "FireTorrent",
            b"XL" => "Xunlei",
            b"XT" => "XanTorrent",
            b"XX" => "Xtorrent",
            b"ZT" => "ZipTorrent",
            _ => "",
        };

        let version = format_azureus_version(&ver_bytes);

        if !client_name.is_empty() {
            if version.is_empty() {
                return client_name.to_string();
            } else {
                return format!("{} {}", client_name, version);
            }
        } else if let Ok(code_str) = std::str::from_utf8(client_code) {
            if version.is_empty() {
                return format!("Unknown ({})", code_str);
            } else {
                return format!("Unknown ({}) {}", code_str, version);
            }
        }
    }

    // BitComet style: 'exbc' + [major] + [minor]
    if &peer_id[0..4] == b"exbc" {
        return format!("BitComet {}.{}", peer_id[4], peer_id[5]);
    }

    // Free Download Manager: 'FDM'
    if &peer_id[0..3] == b"FDM" {
        return "Free Download Manager".to_string();
    }

    // Mainline style: 'M' followed by digits/dashes
    if peer_id[0] == b'M' && peer_id[2] == b'-' && peer_id[4] == b'-' {
        let v0 = peer_id[1] as char;
        let v1 = peer_id[3] as char;
        let v2 = peer_id[5] as char;
        return format!("BitTorrent {}.{}.{}", v0, v1, v2);
    }

    // Fallback: check if first 8 bytes are printable ASCII
    if peer_id[..8].iter().all(|&b| (0x20..=0x7E).contains(&b)) {
        if let Ok(s) = std::str::from_utf8(&peer_id[..8]) {
            return format!("Unknown ({})", s.trim());
        }
    }

    // Generic hex prefix fallback
    format!(
        "Unknown ({:02x}{:02x}{:02x}{:02x})",
        peer_id[0], peer_id[1], peer_id[2], peer_id[3]
    )
}

fn format_azureus_version(ver: &[u8; 4]) -> String {
    if ver.iter().all(|b| b.is_ascii_digit()) {
        let d0 = (ver[0] - b'0') as u32;
        let d1 = (ver[1] - b'0') as u32;
        let d2 = (ver[2] - b'0') as u32;
        let d3 = (ver[3] - b'0') as u32;

        if d3 == 0 {
            format!("{}.{}.{}", d0, d1, d2)
        } else {
            format!("{}.{}.{}.{}", d0, d1, d2, d3)
        }
    } else if ver.iter().all(|b| b.is_ascii_alphanumeric()) {
        if let Ok(s) = std::str::from_utf8(ver) {
            s.trim().to_string()
        } else {
            String::new()
        }
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qbittorrent() {
        let mut id = [0u8; 20];
        id[..8].copy_from_slice(b"-qB4600-");
        assert_eq!(parse_peer_id(&id), "qBittorrent 4.6.0");
    }

    #[test]
    fn test_transmission() {
        let mut id = [0u8; 20];
        id[..8].copy_from_slice(b"-TR4050-");
        assert_eq!(parse_peer_id(&id), "Transmission 4.0.5");
    }

    #[test]
    fn test_utorrent() {
        let mut id = [0u8; 20];
        id[..8].copy_from_slice(b"-UT3550-");
        assert_eq!(parse_peer_id(&id), "µTorrent 3.5.5");
    }

    #[test]
    fn test_hydroxytorrent() {
        let mut id = [0u8; 20];
        id[..8].copy_from_slice(b"-OH0100-");
        assert_eq!(parse_peer_id(&id), "HydroxyTorrent 0.1.0");

        let mut legacy_id = [0u8; 20];
        legacy_id[..8].copy_from_slice(b"-HT0100-");
        assert_eq!(parse_peer_id(&legacy_id), "HydroxyTorrent 0.1.0");

        let generated_id = crate::get_peer_id();
        assert_eq!(&generated_id[..3], b"-OH");
        assert_eq!(generated_id[7], b'-');
        assert!(parse_peer_id(&generated_id).starts_with("HydroxyTorrent"));
    }

    #[test]
    fn test_bitcomet() {
        let mut id = [0u8; 20];
        id[..6].copy_from_slice(b"exbc\x02\x05");
        assert_eq!(parse_peer_id(&id), "BitComet 2.5");
    }
}
