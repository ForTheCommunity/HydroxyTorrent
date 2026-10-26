use rand::distr::{Alphanumeric, SampleString};

pub mod peer_id;
pub mod torrent_info;

pub use peer_id::parse_peer_id;

pub fn get_peer_id() -> [u8; 20] {
    let mut peer_id = [0u8; 20];

    // Getting version from Cargo
    let major: u8 = env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap_or(0);
    let minor: u8 = env!("CARGO_PKG_VERSION_MINOR").parse().unwrap_or(0);
    let patch: u8 = env!("CARGO_PKG_VERSION_PATCH").parse().unwrap_or(0);

    // Format to 4 digits Azureus convention: e.g., 0100 for 0.1.0
    // (1 digit major, 1 digit minor, 2 digits patch)
    let version_str = format!("{:01}{:01}{:02}", major % 10, minor % 10, patch % 100);

    // 8-byte prefix: -HT0100-
    let prefix = format!("-HT{}-", version_str);
    peer_id[..8].copy_from_slice(prefix.as_bytes());

    // Filling remaining 12 bytes with randomness
    let random_suffix = Alphanumeric.sample_string(&mut rand::rng(), 12);
    peer_id[8..].copy_from_slice(random_suffix.as_bytes());
    peer_id
}

pub fn get_piece_size(bytes: usize) -> String {
    if bytes >= 1048576 {
        format!("{:.2} MB", bytes as f64 / 1048576.0)
    } else {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    }
}

pub fn get_total_pieces_count(pieces_len: usize) -> usize {
    pieces_len / 20
}
