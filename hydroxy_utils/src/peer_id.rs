/// Parses a 20-byte BitTorrent Peer ID into a human-readable client name and version.
/// Supports Azureus-style (BEP 0020), Shadow-style, Mainline-style, BitComet/BitLord,
/// and custom client signatures.
pub fn parse_peer_id(peer_id: &[u8; 20]) -> String {
    // Azureus-style: '-<2-char ID><4-char version>-<12 random bytes>'
    // Also handles 3-char prefix like '-aria2-' or special Azureus variants
    if let Some(res) = parse_azureus_style(peer_id) {
        return res;
    }

    // Special custom prefixes (exbc/BitComet, BitLord, Opera, XBT, etc.)
    if let Some(res) = parse_custom_style(peer_id) {
        return res;
    }

    // Shadow's style: '<1-char code><3-5 version chars>---...'
    if let Some(res) = parse_shadow_style(peer_id) {
        return res;
    }

    // Mainline style: 'M<digits>-<digits>-...' or 'Q<digits>-<digits>-...'
    if let Some(res) = parse_mainline_style(peer_id) {
        return res;
    }

    // Fallback: printable ASCII prefix if clean
    if peer_id[..8].iter().all(|&b| (0x20..=0x7E).contains(&b)) {
        if let Ok(s) = std::str::from_utf8(&peer_id[..8]) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return format!("Unknown ({})", trimmed);
            }
        }
    }

    // Generic hex prefix fallback
    format!(
        "Unknown ({:02x}{:02x}{:02x}{:02x})",
        peer_id[0], peer_id[1], peer_id[2], peer_id[3]
    )
}

fn parse_azureus_style(peer_id: &[u8; 20]) -> Option<String> {
    if peer_id[0] != b'-' {
        return None;
    }

    // Check for "-aria2-" (6-char ID)
    if &peer_id[1..7] == b"aria2-" {
        return Some("aria2".to_string());
    }

    // Check for "-rqbit-"
    if &peer_id[1..7] == b"rqbit-" {
        return Some("rqbit".to_string());
    }

    // Standard Azureus style has '-' at index 7: '-XXYYYY-'
    if peer_id[7] != b'-' {
        return None;
    }

    let client_code = &peer_id[1..3];
    let ver_bytes = [peer_id[3], peer_id[4], peer_id[5], peer_id[6]];

    let (client_name, version) = match client_code {
        b"7T" => ("aTorrent", format_numeric_version(&ver_bytes)),
        b"A~" | b"AG" => ("Ares", format_numeric_version(&ver_bytes)),
        b"AB" => ("AnyEvent::BitTorrent", format_numeric_version(&ver_bytes)),
        b"AD" => (
            "Advanced Download Manager",
            format_numeric_version(&ver_bytes),
        ),
        b"AR" => ("Arctic", format_numeric_version(&ver_bytes)),
        b"AT" => ("Artemis", format_numeric_version(&ver_bytes)),
        b"AV" => ("Avicora", format_numeric_version(&ver_bytes)),
        b"AX" => ("BitPump", format_numeric_version(&ver_bytes)),
        b"AZ" => ("Azureus / Vuze", format_numeric_version(&ver_bytes)),
        b"BB" => ("BitBuddy", format_numeric_version(&ver_bytes)),
        b"BC" => ("BitComet", format_two_by_two_version(&ver_bytes)),
        b"BE" => ("Baretorrent", format_numeric_version(&ver_bytes)),
        b"BF" => ("BitFlu", format_numeric_version(&ver_bytes)),
        b"BG" => ("BTG", format_numeric_version(&ver_bytes)),
        b"BI" => ("BiglyBT", format_numeric_version(&ver_bytes)),
        b"BL" => ("BitBlinder", format_numeric_version(&ver_bytes)),
        b"BP" => ("BitTorrent Pro", format_numeric_version(&ver_bytes)),
        b"BR" => ("BitRocket", format_numeric_version(&ver_bytes)),
        b"BS" => ("BTSlave", format_numeric_version(&ver_bytes)),
        b"BT" => ("BitTorrent", format_utorrent_version(&ver_bytes)),
        b"Bt" => ("Bt", format_numeric_version(&ver_bytes)),
        b"BW" => ("BitWombat", format_numeric_version(&ver_bytes)),
        b"BX" => ("BitTorrent X", format_numeric_version(&ver_bytes)),
        b"CB" => ("Shareaza Plus", format_numeric_version(&ver_bytes)),
        b"CD" => ("Enhanced CTorrent", format_numeric_version(&ver_bytes)),
        b"CT" => ("CTorrent", format_numeric_version(&ver_bytes)),
        b"DE" => ("Deluge", format_deluge_version(&ver_bytes)),
        b"DL" => ("DelTorrent", format_numeric_version(&ver_bytes)),
        b"DP" => ("Propagate Data Client", format_numeric_version(&ver_bytes)),
        b"DT" => ("DeltaTorrent", format_numeric_version(&ver_bytes)),
        b"EB" => ("EBit", format_numeric_version(&ver_bytes)),
        b"ES" => ("Electric Sheep", format_numeric_version(&ver_bytes)),
        b"FC" => ("FileCroc", format_numeric_version(&ver_bytes)),
        b"FD" => ("Free Download Manager", format_fdm_version(&ver_bytes)),
        b"FG" => ("FlashGet", format_numeric_version(&ver_bytes)),
        b"FK" => ("Folx", format_numeric_version(&ver_bytes)),
        b"FL" => ("Flud", format_numeric_version(&ver_bytes)),
        b"FT" => ("FoxTorrent", format_numeric_version(&ver_bytes)),
        b"FW" => ("FrostWire", format_numeric_version(&ver_bytes)),
        b"FX" => ("Freebox BitTorrent", format_numeric_version(&ver_bytes)),
        b"G3" => ("G3 Torrent", format_numeric_version(&ver_bytes)),
        b"GP" => ("GoPeer", format_numeric_version(&ver_bytes)),
        b"GS" => ("GSTorrent", format_numeric_version(&ver_bytes)),
        b"GT" => ("anacrolix/torrent", format_numeric_version(&ver_bytes)),
        b"HK" => ("Hekate", format_numeric_version(&ver_bytes)),
        b"HL" => ("Halite", format_numeric_version(&ver_bytes)),
        b"HM" => ("hMule", format_numeric_version(&ver_bytes)),
        b"HN" => ("Hydranode", format_numeric_version(&ver_bytes)),
        b"IL" => ("iLivid", format_numeric_version(&ver_bytes)),
        b"JS" => ("JustTorrent", format_numeric_version(&ver_bytes)),
        b"JT" => ("JavaTorrent", format_numeric_version(&ver_bytes)),
        b"KG" => ("KGet", format_numeric_version(&ver_bytes)),
        b"KT" => ("KTorrent", format_ktorrent_version(&ver_bytes)),
        b"LC" => ("LeechCraft", format_numeric_version(&ver_bytes)),
        b"LH" => ("LH-ABC", format_numeric_version(&ver_bytes)),
        b"LP" => ("Lphant", format_numeric_version(&ver_bytes)),
        b"LT" => (
            "libtorrent (Rasterbar)",
            format_libtorrent_version(&ver_bytes),
        ),
        b"lt" => (
            "libtorrent (Rakshasa) / rTorrent",
            format_libtorrent_version(&ver_bytes),
        ),
        b"LW" => ("LimeWire", format_numeric_version(&ver_bytes)),
        b"M~" => ("Mainline", format_numeric_version(&ver_bytes)),
        b"MG" => ("MediaGet", format_numeric_version(&ver_bytes)),
        b"ML" => ("MLDonkey", format_numeric_version(&ver_bytes)),
        b"MO" => ("MonoTorrent", format_numeric_version(&ver_bytes)),
        b"MP" => ("MooPolice", format_numeric_version(&ver_bytes)),
        b"MR" => ("Miro", format_numeric_version(&ver_bytes)),
        b"MT" => ("MoonlightTorrent", format_numeric_version(&ver_bytes)),
        b"NB" => ("NetBit", format_numeric_version(&ver_bytes)),
        b"NE" => ("BT Next Evolution", format_numeric_version(&ver_bytes)),
        b"NX" => ("Net Transport", format_numeric_version(&ver_bytes)),
        b"OH" => ("HydroxyTorrent", format_numeric_version(&ver_bytes)),
        b"OS" => ("OneSwarm", format_numeric_version(&ver_bytes)),
        b"OT" => ("OmegaTorrent", format_numeric_version(&ver_bytes)),
        b"PB" => ("pbTorrent", format_numeric_version(&ver_bytes)),
        b"PC" => ("CacheLogic", format_numeric_version(&ver_bytes)),
        b"PD" => ("Pando", format_numeric_version(&ver_bytes)),
        b"PE" => ("PeerProject", format_numeric_version(&ver_bytes)),
        b"PI" => ("PicoTorrent", format_numeric_version(&ver_bytes)),
        b"PT" => ("PHPTracker", format_numeric_version(&ver_bytes)),
        b"qB" => ("qBittorrent", format_qbittorrent_version(&ver_bytes)),
        b"QD" => ("QQDownload", format_numeric_version(&ver_bytes)),
        b"QT" => ("Qt 4 Torrent", format_numeric_version(&ver_bytes)),
        b"R~" => ("Tribler", format_numeric_version(&ver_bytes)),
        b"RQ" => ("rqbit", format_numeric_version(&ver_bytes)),
        b"RT" => ("Retriever", format_numeric_version(&ver_bytes)),
        b"RZ" => ("RezTorrent", format_numeric_version(&ver_bytes)),
        b"S~" => ("Shareaza", format_numeric_version(&ver_bytes)),
        b"SB" => ("Swiftbit", format_numeric_version(&ver_bytes)),
        b"SD" => ("Thunder / Xunlei", format_numeric_version(&ver_bytes)),
        b"SG" => ("GS Torrent", format_numeric_version(&ver_bytes)),
        b"SM" => ("SoMud", format_numeric_version(&ver_bytes)),
        b"SN" => ("ShareNet", format_numeric_version(&ver_bytes)),
        b"SP" => ("BitSpirit", format_numeric_version(&ver_bytes)),
        b"SS" => ("SwarmScope", format_numeric_version(&ver_bytes)),
        b"ST" => ("SymTorrent", format_numeric_version(&ver_bytes)),
        b"st" => ("sharktorrent", format_numeric_version(&ver_bytes)),
        b"SZ" => ("Shareaza", format_numeric_version(&ver_bytes)),
        b"tT" => ("tTorrent", format_numeric_version(&ver_bytes)),
        b"TB" => ("Torch", format_numeric_version(&ver_bytes)),
        b"TE" => ("terasaur Seed Bank", format_numeric_version(&ver_bytes)),
        b"TG" => ("Torrent GO", format_numeric_version(&ver_bytes)),
        b"TL" => ("Tribler", format_numeric_version(&ver_bytes)),
        b"TN" => ("TorrentDotNET", format_numeric_version(&ver_bytes)),
        b"TR" => ("Transmission", format_transmission_version(&ver_bytes)),
        b"TS" => ("Torrentstorm", format_numeric_version(&ver_bytes)),
        b"TT" => ("TuoTu", format_numeric_version(&ver_bytes)),
        b"TX" => ("Tixati", format_numeric_version(&ver_bytes)),
        b"U~" => ("µTorrent", format_utorrent_version(&ver_bytes)),
        b"UL" => ("uLeecher!", format_numeric_version(&ver_bytes)),
        b"UM" => ("µTorrent Mac", format_utorrent_version(&ver_bytes)),
        b"UT" => ("µTorrent", format_utorrent_version(&ver_bytes)),
        b"UW" => ("µTorrent Web", format_utorrent_version(&ver_bytes)),
        b"VG" => ("Vagaa", format_numeric_version(&ver_bytes)),
        b"WD" => ("WebTorrent Desktop", format_numeric_version(&ver_bytes)),
        b"WT" => ("BitLet", format_numeric_version(&ver_bytes)),
        b"WW" => ("WebTorrent", format_numeric_version(&ver_bytes)),
        b"WY" => ("FireTorrent", format_numeric_version(&ver_bytes)),
        b"XC" | b"XX" => ("Xtorrent", format_numeric_version(&ver_bytes)),
        b"XF" => ("Xfplay", format_numeric_version(&ver_bytes)),
        b"XL" => ("Xunlei", format_numeric_version(&ver_bytes)),
        b"XT" => ("XanTorrent", format_numeric_version(&ver_bytes)),
        b"ZT" => ("ZipTorrent", format_numeric_version(&ver_bytes)),
        _ => ("", format_numeric_version(&ver_bytes)),
    };

    if !client_name.is_empty() {
        if version.is_empty() {
            Some(client_name.to_string())
        } else {
            Some(format!("{} {}", client_name, version))
        }
    } else if let Ok(code_str) = std::str::from_utf8(client_code) {
        if version.is_empty() {
            Some(format!("Unknown ({})", code_str))
        } else {
            Some(format!("Unknown ({}) {}", code_str, version))
        }
    } else {
        None
    }
}

fn parse_custom_style(peer_id: &[u8; 20]) -> Option<String> {
    // BitComet / BitLord style: 'exbc' + [major byte] + [minor byte]
    if &peer_id[0..4] == b"exbc" {
        let major = peer_id[4];
        let minor = peer_id[5];
        // BitLord signature check: bytes 6..10 == b"LORD"
        if &peer_id[6..10] == b"LORD" {
            return Some(format!("BitLord {}.{:02}", major, minor));
        }
        return Some(format!("BitComet {}.{:02}", major, minor));
    }

    // BitComet mod signatures
    if &peer_id[0..4] == b"FUTB" || &peer_id[0..4] == b"xUTB" {
        return Some("BitComet (Mod)".to_string());
    }

    // Free Download Manager: 'FDM'
    if &peer_id[0..3] == b"FDM" {
        return Some("Free Download Manager".to_string());
    }

    // Opera: 'OP' + 4-digit build number
    if peer_id[0] == b'O' && peer_id[1] == b'P' && peer_id[2..6].iter().all(|b| b.is_ascii_digit())
    {
        if let Ok(build) = std::str::from_utf8(&peer_id[2..6]) {
            return Some(format!("Opera {}", build));
        }
    }

    // XBT Client: 'XBT' + 3 digits
    if &peer_id[0..3] == b"XBT" && peer_id[3..6].iter().all(|b| b.is_ascii_digit()) {
        return Some(format!(
            "XBT Client {}.{}.{}",
            (peer_id[3] - b'0'),
            (peer_id[4] - b'0'),
            (peer_id[5] - b'0')
        ));
    }

    // Bits on Wheels: '-BOW' + 3 chars + '-'
    if &peer_id[0..4] == b"-BOW" && peer_id[7] == b'-' {
        if let Ok(ver) = std::str::from_utf8(&peer_id[4..7]) {
            return Some(format!("Bits on Wheels {}", ver.trim()));
        }
        return Some("Bits on Wheels".to_string());
    }

    // MLDonkey: '-ML' + dot-separated version
    if &peer_id[0..3] == b"-ML" {
        if let Some(end) = peer_id[3..].iter().position(|&b| b == b'-') {
            if let Ok(ver) = std::str::from_utf8(&peer_id[3..3 + end]) {
                return Some(format!("MLDonkey {}", ver.trim()));
            }
        }
        return Some("MLDonkey".to_string());
    }

    // BitSpirit: \0\x02BS or \0\x03BS or HTTPBT
    if peer_id[0] == 0 && peer_id[1] == 2 && &peer_id[2..4] == b"BS" {
        return Some("BitSpirit 2".to_string());
    }
    if peer_id[0] == 0 && peer_id[1] == 3 && &peer_id[2..4] == b"BS" {
        return Some("BitSpirit 3".to_string());
    }
    if &peer_id[0..6] == b"HTTPBT" {
        return Some("BitSpirit".to_string());
    }

    // Tixati: 'TIX' + 4 digits
    if &peer_id[0..3] == b"TIX" && peer_id[3..7].iter().all(|b| b.is_ascii_digit()) {
        if let Ok(ver) = std::str::from_utf8(&peer_id[3..7]) {
            return Some(format!("Tixati {}", ver));
        }
    }

    // BitTorrent DNA: 'DNA'
    if &peer_id[0..3] == b"DNA" {
        return Some("BitTorrent DNA".to_string());
    }

    // LimeWire: 'LIME'
    if &peer_id[0..4] == b"LIME" {
        return Some("LimeWire".to_string());
    }

    // burst!: 'Mbrst'
    if &peer_id[0..5] == b"Mbrst" {
        return Some("burst!".to_string());
    }

    // Pando: 'Pando'
    if &peer_id[0..5] == b"Pando" {
        return Some("Pando".to_string());
    }

    // Plus!: 'Plus' or 'PRC.P'
    if &peer_id[0..4] == b"Plus" || &peer_id[0..5] == b"PRC.P" {
        return Some("Plus!".to_string());
    }

    // TurboBT: 'turb'
    if &peer_id[0..4] == b"turb" {
        return Some("TurboBT".to_string());
    }

    // btpd: 'btpd/'
    if &peer_id[0..5] == b"btpd/" {
        return Some("btpd".to_string());
    }

    // XanTorrent: 'DansClient'
    if &peer_id[0..10] == b"DansClient" {
        return Some("XanTorrent".to_string());
    }

    None
}

// Shadow's style: <1-char ID><3-5 version chars>---...
fn parse_shadow_style(peer_id: &[u8; 20]) -> Option<String> {
    let client_char = peer_id[0];
    let client_name = match client_char {
        b'A' => "ABC",
        b'O' => "Osprey Permaseed",
        b'Q' => "BTQueue",
        b'R' => "Tribler",
        b'S' => "Shadow's client",
        b'T' => "BitTornado",
        b'U' => "UPnP NAT BitTorrent",
        _ => return None,
    };

    // Shadow style requires dashes starting between index 4 and index 8
    let mut dash_start = None;
    for i in 2..8 {
        if peer_id[i] == b'-'
            && i + 2 < peer_id.len()
            && peer_id[i + 1] == b'-'
            && peer_id[i + 2] == b'-'
        {
            dash_start = Some(i);
            break;
        }
    }

    let end = dash_start.unwrap_or(5);
    let ver_slice = &peer_id[1..end];
    if ver_slice.is_empty() || !ver_slice.iter().all(|&b| b.is_ascii_alphanumeric()) {
        return None;
    }

    let mut parts = Vec::new();
    for &b in ver_slice {
        let val = if b.is_ascii_digit() {
            (b - b'0') as u32
        } else if b.is_ascii_uppercase() {
            (b - b'A' + 10) as u32
        } else if b.is_ascii_lowercase() {
            (b - b'a' + 36) as u32
        } else {
            0
        };
        parts.push(val.to_string());
    }

    Some(format!("{} {}", client_name, parts.join(".")))
}

// Mainline style: M4-3-6--... or Q1-20-4-...
fn parse_mainline_style(peer_id: &[u8; 20]) -> Option<String> {
    let client_name = match peer_id[0] {
        b'M' => "BitTorrent",
        b'Q' => "Queen Bee",
        _ => return None,
    };

    // Expect 'M' or 'Q' followed by digits and dashes: M4-20-8--
    if peer_id[1] == b'-' || !peer_id[1].is_ascii_digit() {
        return None;
    }

    let rest = match std::str::from_utf8(&peer_id[1..12]) {
        Ok(s) => s,
        Err(_) => return None,
    };

    let segments: Vec<&str> = rest
        .split('-')
        .take_while(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
        .collect();

    if segments.len() >= 2 {
        Some(format!("{} {}", client_name, segments.join(".")))
    } else {
        None
    }
}

fn format_numeric_version(ver: &[u8; 4]) -> String {
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
    } else if ver
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || *b == b'-' || *b == b' ')
    {
        if let Ok(s) = std::str::from_utf8(ver) {
            s.trim_matches(&['-', ' '][..]).to_string()
        } else {
            String::new()
        }
    } else {
        String::new()
    }
}

fn format_qbittorrent_version(ver: &[u8; 4]) -> String {
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
    } else {
        format_numeric_version(ver)
    }
}

fn format_transmission_version(ver: &[u8; 4]) -> String {
    // Transmission: e.g. '0006' -> '0.6', '0072' -> '0.72', '1330' -> '1.33', '2940' -> '2.94', '4050' -> '4.0.5'
    let is_nightly = ver[3] == b'Z' || ver[3] == b'+';
    let digits = [
        ver[0],
        ver[1],
        ver[2],
        if is_nightly { b'0' } else { ver[3] },
    ];

    if digits.iter().all(|b| b.is_ascii_digit()) {
        let major = (digits[0] - b'0') as u32;
        let d1 = (digits[1] - b'0') as u32;
        let d2 = (digits[2] - b'0') as u32;
        let d3 = (digits[3] - b'0') as u32;

        let base_ver = if major == 0 {
            let minor = d1 * 10 + d2;
            if d3 == 0 {
                format!("0.{}", minor)
            } else {
                format!("0.{}.{}", minor, d3)
            }
        } else if major < 4 {
            let minor = d1 * 10 + d2;
            if d3 == 0 {
                format!("{}.{:02}", major, minor)
            } else {
                format!("{}.{:02}.{}", major, minor, d3)
            }
        } else {
            // Transmission 4.x.x
            if d3 == 0 {
                format!("{}.{}.{}", major, d1, d2)
            } else {
                format!("{}.{}.{}.{}", major, d1, d2, d3)
            }
        };

        if is_nightly {
            format!("{}+", base_ver)
        } else {
            base_ver
        }
    } else {
        format_numeric_version(ver)
    }
}

fn format_utorrent_version(ver: &[u8; 4]) -> String {
    if ver[0].is_ascii_digit() && ver[1].is_ascii_digit() && ver[2].is_ascii_digit() {
        let d0 = (ver[0] - b'0') as u32;
        let d1 = (ver[1] - b'0') as u32;
        let d2 = (ver[2] - b'0') as u32;
        let suffix = match ver[3] {
            b'0' | b'S' => String::new(),
            b'B' => " Beta".to_string(),
            b'A' => " Alpha".to_string(),
            b'D' => " Dev".to_string(),
            b if b.is_ascii_digit() => format!(".{}", b - b'0'),
            b if b.is_ascii_alphanumeric() => format!(" {}", b as char),
            _ => String::new(),
        };
        format!("{}.{}.{}{}", d0, d1, d2, suffix)
    } else {
        format_numeric_version(ver)
    }
}

fn format_two_by_two_version(ver: &[u8; 4]) -> String {
    if ver.iter().all(|b| b.is_ascii_digit()) {
        let major = (ver[0] - b'0') * 10 + (ver[1] - b'0');
        let minor = (ver[2] - b'0') * 10 + (ver[3] - b'0');
        format!("{}.{:02}", major, minor)
    } else {
        format_numeric_version(ver)
    }
}

fn format_libtorrent_version(ver: &[u8; 4]) -> String {
    let parse_hex = |b: u8| -> Option<u32> {
        match b {
            b'0'..=b'9' => Some((b - b'0') as u32),
            b'A'..=b'F' => Some((b - b'A' + 10) as u32),
            b'a'..=b'f' => Some((b - b'a' + 10) as u32),
            _ => None,
        }
    };

    if let (Some(d0), Some(d1), Some(d2), Some(d3)) = (
        parse_hex(ver[0]),
        parse_hex(ver[1]),
        parse_hex(ver[2]),
        parse_hex(ver[3]),
    ) {
        if d3 == 0 {
            format!("{}.{}.{}", d0, d1, d2)
        } else {
            format!("{}.{}.{}.{}", d0, d1, d2, d3)
        }
    } else {
        format_numeric_version(ver)
    }
}

fn format_deluge_version(ver: &[u8; 4]) -> String {
    format_libtorrent_version(ver)
}

fn format_fdm_version(ver: &[u8; 4]) -> String {
    let clean: String = ver
        .iter()
        .filter(|&&b| b.is_ascii_digit())
        .map(|&b| b as char)
        .collect();
    if clean.len() >= 2 {
        format!("{}.{}", &clean[0..1], &clean[1..])
    } else if !clean.is_empty() {
        clean
    } else {
        String::new()
    }
}

fn format_ktorrent_version(ver: &[u8; 4]) -> String {
    if ver[0].is_ascii_digit() && ver[1].is_ascii_digit() {
        let d0 = (ver[0] - b'0') as u32;
        let d1 = (ver[1] - b'0') as u32;
        let suffix = match ver[2] {
            b'R' => format!(" RC{}", (ver[3] as char)),
            b'D' => " Dev".to_string(),
            b if b.is_ascii_digit() => format!(".{}.{}", b - b'0', ver[3] as char),
            _ => String::new(),
        };
        format!("{}.{}{}", d0, d1, suffix)
    } else {
        format_numeric_version(ver)
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

        let mut id2 = [0u8; 20];
        id2[..8].copy_from_slice(b"-qB4651-");
        assert_eq!(parse_peer_id(&id2), "qBittorrent 4.6.5.1");
    }

    #[test]
    fn test_transmission() {
        let mut id = [0u8; 20];
        id[..8].copy_from_slice(b"-TR4050-");
        assert_eq!(parse_peer_id(&id), "Transmission 4.0.5");

        let mut id2 = [0u8; 20];
        id2[..8].copy_from_slice(b"-TR2940-");
        assert_eq!(parse_peer_id(&id2), "Transmission 2.94");

        let mut id3 = [0u8; 20];
        id3[..8].copy_from_slice(b"-TR400Z-");
        assert_eq!(parse_peer_id(&id3), "Transmission 4.0.0+");
    }

    #[test]
    fn test_utorrent() {
        let mut id = [0u8; 20];
        id[..8].copy_from_slice(b"-UT3550-");
        assert_eq!(parse_peer_id(&id), "µTorrent 3.5.5");

        let mut id_beta = [0u8; 20];
        id_beta[..8].copy_from_slice(b"-UT355B-");
        assert_eq!(parse_peer_id(&id_beta), "µTorrent 3.5.5 Beta");

        let mut id_web = [0u8; 20];
        id_web[..8].copy_from_slice(b"-UW1020-");
        assert_eq!(parse_peer_id(&id_web), "µTorrent Web 1.0.2");
    }

    #[test]
    fn test_deluge_and_libtorrent() {
        let mut id_deluge = [0u8; 20];
        id_deluge[..8].copy_from_slice(b"-DE13F0-");
        assert_eq!(parse_peer_id(&id_deluge), "Deluge 1.3.15");

        let mut id_lt = [0u8; 20];
        id_lt[..8].copy_from_slice(b"-lt0D80-");
        assert_eq!(
            parse_peer_id(&id_lt),
            "libtorrent (Rakshasa) / rTorrent 0.13.8"
        );

        let mut id_rasterbar = [0u8; 20];
        id_rasterbar[..8].copy_from_slice(b"-LT20A0-");
        assert_eq!(
            parse_peer_id(&id_rasterbar),
            "libtorrent (Rasterbar) 2.0.10"
        );
    }

    #[test]
    fn test_hydroxytorrent() {
        let mut id = [0u8; 20];
        id[..8].copy_from_slice(b"-OH0100-");
        assert_eq!(parse_peer_id(&id), "HydroxyTorrent 0.1.0");


        let generated_id = crate::get_peer_id();
        assert_eq!(&generated_id[..3], b"-OH");
        assert_eq!(generated_id[7], b'-');
        assert!(parse_peer_id(&generated_id).starts_with("HydroxyTorrent"));
    }

    #[test]
    fn test_shadow_style() {
        let mut id_bittornado = [0u8; 20];
        id_bittornado[..9].copy_from_slice(b"T03A-----");
        assert_eq!(parse_peer_id(&id_bittornado), "BitTornado 0.3.10");

        let mut id_abc = [0u8; 20];
        id_abc[..9].copy_from_slice(b"A280-----");
        assert_eq!(parse_peer_id(&id_abc), "ABC 2.8.0");

        let mut id_shadow = [0u8; 20];
        id_shadow[..9].copy_from_slice(b"S58B-----");
        assert_eq!(parse_peer_id(&id_shadow), "Shadow's client 5.8.11");
    }

    #[test]
    fn test_mainline_style() {
        let mut id_mainline = [0u8; 20];
        id_mainline[..10].copy_from_slice(b"M4-20-8---");
        assert_eq!(parse_peer_id(&id_mainline), "BitTorrent 4.20.8");

        let mut id_queenbee = [0u8; 20];
        id_queenbee[..10].copy_from_slice(b"Q1-10-0---");
        assert_eq!(parse_peer_id(&id_queenbee), "Queen Bee 1.10.0");
    }

    #[test]
    fn test_bitcomet_and_bitlord() {
        let mut id_bc = [0u8; 20];
        id_bc[..6].copy_from_slice(b"exbc\x01\x38");
        assert_eq!(parse_peer_id(&id_bc), "BitComet 1.56");

        let mut id_lord = [0u8; 20];
        id_lord[..10].copy_from_slice(b"exbc\x02\x05LORD");
        assert_eq!(parse_peer_id(&id_lord), "BitLord 2.05");

        let mut id_bc_az = [0u8; 20];
        id_bc_az[..8].copy_from_slice(b"-BC0156-");
        assert_eq!(parse_peer_id(&id_bc_az), "BitComet 1.56");
    }

    #[test]
    fn test_custom_clients() {
        let mut id_aria2 = [0u8; 20];
        id_aria2[..7].copy_from_slice(b"-aria2-");
        assert_eq!(parse_peer_id(&id_aria2), "aria2");

        let mut id_rqbit = [0u8; 20];
        id_rqbit[..7].copy_from_slice(b"-rqbit-");
        assert_eq!(parse_peer_id(&id_rqbit), "rqbit");

        let mut id_opera = [0u8; 20];
        id_opera[..6].copy_from_slice(b"OP9500");
        assert_eq!(parse_peer_id(&id_opera), "Opera 9500");

        let mut id_xbt = [0u8; 20];
        id_xbt[..6].copy_from_slice(b"XBT054");
        assert_eq!(parse_peer_id(&id_xbt), "XBT Client 0.5.4");

        let mut id_tixati = [0u8; 20];
        id_tixati[..7].copy_from_slice(b"TIX0325");
        assert_eq!(parse_peer_id(&id_tixati), "Tixati 0325");
    }
}
