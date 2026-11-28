use sha1::{Digest, Sha1};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Default number of pieces in an Allowed Fast set per BEP 0006 convention (`k = 10`).
pub const DEFAULT_ALLOWED_FAST_K: usize = 10;

fn generate_allowed_fast_from_seed(mut x: Vec<u8>, num_pieces: u32, k: usize) -> Vec<u32> {
    if num_pieces == 0 || k == 0 {
        return Vec::new();
    }

    let target_len = k.min(num_pieces as usize);
    let mut allowed = Vec::with_capacity(target_len);

    while allowed.len() < target_len {
        let digest = Sha1::digest(&x);
        x.clear();
        x.extend_from_slice(&digest);

        for i in 0..5 {
            if allowed.len() >= target_len {
                break;
            }
            let j = i * 4;
            let y = u32::from_be_bytes(x[j..j + 4].try_into().unwrap());
            let index = y % num_pieces;
            if !allowed.contains(&index) {
                allowed.push(index);
            }
        }
    }

    allowed
}

/// Generates the canonical BEP 0006 Allowed Fast piece index set for a remote IPv4 peer.
///
/// Per BEP 0006:
/// 1. Mask `ip` with `0xFFFFFF00` (class C `/24` prefix in network byte order) so peers on a
///    single `/24` subnet share the same Allowed Fast set.
/// 2. Concatenate the 4-byte masked IP with the 20-byte `info_hash` (24 bytes total).
/// 3. Repeatedly compute `x = SHA1(x)` (20 bytes) and extract up to 5 big-endian `u32` words
///    per digest, adding `y % num_pieces` if not already in the set, until `min(k, num_pieces)`
///    unique piece indices have been collected.
pub fn generate_allowed_fast_set(
    ip: Ipv4Addr,
    info_hash: &[u8; 20],
    num_pieces: u32,
    k: usize,
) -> Vec<u32> {
    let masked_ip = (u32::from(ip) & 0xFFFF_FF00).to_be_bytes();
    let mut x = Vec::with_capacity(24);
    x.extend_from_slice(&masked_ip);
    x.extend_from_slice(info_hash);
    generate_allowed_fast_from_seed(x, num_pieces, k)
}

/// Generates the Allowed Fast piece index set for an IPv6 peer by masking to its `/48` prefix
/// (so hosts within the same end-site `/48` allocation share one Allowed Fast set).
pub fn generate_allowed_fast_set_v6(
    ip: Ipv6Addr,
    info_hash: &[u8; 20],
    num_pieces: u32,
    k: usize,
) -> Vec<u32> {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return generate_allowed_fast_set(v4, info_hash, num_pieces, k);
    }
    let octets = ip.octets();
    let mut masked_ip = [0u8; 16];
    masked_ip[..6].copy_from_slice(&octets[..6]); // /48 prefix
    let mut x = Vec::with_capacity(36);
    x.extend_from_slice(&masked_ip);
    x.extend_from_slice(info_hash);
    generate_allowed_fast_from_seed(x, num_pieces, k)
}

/// Generates the Allowed Fast piece index set for either an IPv4 or IPv6 peer.
pub fn generate_allowed_fast_set_for_ip(
    ip: IpAddr,
    info_hash: &[u8; 20],
    num_pieces: u32,
    k: usize,
) -> Vec<u32> {
    match ip {
        IpAddr::V4(v4) => generate_allowed_fast_set(v4, info_hash, num_pieces, k),
        IpAddr::V6(v6) => generate_allowed_fast_set_v6(v6, info_hash, num_pieces, k),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bep0006_official_vector_k7_and_k9() {
        // Official BEP 0006 test vector:
        // IP = 80.4.4.200, infohash = 0xaaaa...aa (20 bytes), num_pieces = 1313
        let ip = Ipv4Addr::new(80, 4, 4, 200);
        let info_hash = [0xAAu8; 20];
        let num_pieces = 1313;

        let set_k7 = generate_allowed_fast_set(ip, &info_hash, num_pieces, 7);
        assert_eq!(set_k7, vec![1059, 431, 808, 1217, 287, 376, 1188]);

        let set_k9 = generate_allowed_fast_set(ip, &info_hash, num_pieces, 9);
        assert_eq!(
            set_k9,
            vec![1059, 431, 808, 1217, 287, 376, 1188, 353, 508]
        );
    }

    #[test]
    fn test_allowed_fast_subnet_masking_and_small_torrents() {
        let info_hash = [0xAAu8; 20];
        let set_a = generate_allowed_fast_set(Ipv4Addr::new(80, 4, 4, 1), &info_hash, 1313, 7);
        let set_b = generate_allowed_fast_set(Ipv4Addr::new(80, 4, 4, 254), &info_hash, 1313, 7);
        assert_eq!(set_a, set_b);

        // Different /24 subnet produces a different set
        let set_c = generate_allowed_fast_set(Ipv4Addr::new(80, 4, 5, 200), &info_hash, 1313, 7);
        assert_ne!(set_a, set_c);

        // Torrent with fewer pieces than k must terminate cleanly without infinite loop
        let small = generate_allowed_fast_set(Ipv4Addr::new(10, 0, 0, 5), &info_hash, 4, 10);
        assert_eq!(small.len(), 4);
        for p in 0..4u32 {
            assert!(small.contains(&p));
        }

        // Zero pieces or k = 0 returns empty Vec
        assert!(generate_allowed_fast_set(Ipv4Addr::new(10, 0, 0, 5), &info_hash, 0, 10).is_empty());
        assert!(generate_allowed_fast_set(Ipv4Addr::new(10, 0, 0, 5), &info_hash, 100, 0).is_empty());

        // IPv6 /48 subnet masking: same /48 produces identical Allowed Fast set
        let v6_a: IpAddr = "2406:9cc0:200:1521::1".parse().unwrap();
        let v6_b: IpAddr = "2406:9cc0:200:9999::abcd".parse().unwrap();
        let v6_c: IpAddr = "2406:9cc0:201:1521::1".parse().unwrap();
        let set_v6_a = generate_allowed_fast_set_for_ip(v6_a, &info_hash, 1313, 10);
        let set_v6_b = generate_allowed_fast_set_for_ip(v6_b, &info_hash, 1313, 10);
        let set_v6_c = generate_allowed_fast_set_for_ip(v6_c, &info_hash, 1313, 10);
        assert_eq!(set_v6_a.len(), 10);
        assert_eq!(set_v6_a, set_v6_b);
        assert_ne!(set_v6_a, set_v6_c);
    }
}
