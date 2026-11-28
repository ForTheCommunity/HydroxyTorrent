use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use tokio::{net::UdpSocket, sync::Notify};

use crate::errors::TrackerError;

pub mod errors;
mod http_s;
mod udp;
mod ws;

const DNS_POSITIVE_TTL: Duration = Duration::from_secs(600);
const DNS_NEGATIVE_TTL: Duration = Duration::from_secs(120);

const QTYPE_A: u16 = 1;
const QTYPE_AAAA: u16 = 28;

/// Hardcoded cross-platform IPv4 DNS servers.
const DEFAULT_IPV4_DNS_SERVERS: [SocketAddr; 4] = [
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 53), // Cloudflare
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53), // Google
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9)), 53), // Quad9
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(77, 88, 8, 8)), 53), // Yandex
];

/// Hardcoded cross-platform IPv6 DNS servers (used automatically on IPv6-only networks).
const DEFAULT_IPV6_DNS_SERVERS: [SocketAddr; 3] = [
    SocketAddr::new(
        IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111)),
        53,
    ), // Cloudflare
    SocketAddr::new(
        IpAddr::V6(Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888)),
        53,
    ), // Google
    SocketAddr::new(
        IpAddr::V6(Ipv6Addr::new(0x2620, 0x00fe, 0, 0, 0, 0, 0, 0x00fe)),
        53,
    ), // Quad9
];

fn dns_cache() -> &'static Mutex<HashMap<String, (Instant, Vec<IpAddr>)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Vec<IpAddr>)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn inflight_dns() -> &'static Mutex<HashMap<String, Arc<Notify>>> {
    static INFLIGHT: OnceLock<Mutex<HashMap<String, Arc<Notify>>>> = OnceLock::new();
    INFLIGHT.get_or_init(|| Mutex::new(HashMap::new()))
}

fn build_dns_query(host: &str, tx_id: u16, qtype: u16) -> Option<Vec<u8>> {
    let host = host.trim_end_matches('.');
    if host.is_empty() || host.len() > 253 {
        return None;
    }

    let mut packet = Vec::with_capacity(64);
    packet.extend_from_slice(&tx_id.to_be_bytes());
    packet.extend_from_slice(&0x0100u16.to_be_bytes()); // Flags: Standard query, RD=1
    packet.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT = 1
    packet.extend_from_slice(&0u16.to_be_bytes()); // ANCOUNT = 0
    packet.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT = 0
    packet.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT = 0

    for label in host.split('.') {
        let bytes = label.as_bytes();
        if bytes.is_empty() || bytes.len() > 63 {
            return None;
        }
        packet.push(bytes.len() as u8);
        packet.extend_from_slice(bytes);
    }
    packet.push(0); // Root label
    packet.extend_from_slice(&qtype.to_be_bytes()); // QTYPE = A (1) or AAAA (28)
    packet.extend_from_slice(&1u16.to_be_bytes()); // QCLASS = IN (1)
    Some(packet)
}

fn skip_dns_name(buf: &[u8], mut pos: usize) -> Option<usize> {
    let mut jumps = 0;
    loop {
        let len = *buf.get(pos)?;
        if len == 0 {
            return Some(pos + 1);
        }
        if (len & 0xC0) == 0xC0 {
            // Compression pointer is 2 bytes and terminates the name field
            if pos + 1 >= buf.len() {
                return None;
            }
            return Some(pos + 2);
        }
        if (len & 0xC0) != 0 {
            return None;
        }
        pos += 1 + (len as usize);
        if pos > buf.len() {
            return None;
        }
        jumps += 1;
        if jumps > 128 {
            return None;
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum DnsParseResult {
    Found(Vec<IpAddr>),
    NoDataOrNxDomain,
    ServerFailure,
}

fn parse_dns_response(buf: &[u8], expected_tx_id: u16, expected_qtype: u16) -> Option<DnsParseResult> {
    if buf.len() < 12 {
        return None;
    }
    let tx_id = u16::from_be_bytes([buf[0], buf[1]]);
    if tx_id != expected_tx_id {
        return None;
    }
    let flags = u16::from_be_bytes([buf[2], buf[3]]);
    if (flags & 0x8000) == 0 {
        // Not a response
        return None;
    }
    let rcode = flags & 0x000F;
    if rcode == 3 {
        // NXDOMAIN
        return Some(DnsParseResult::NoDataOrNxDomain);
    }
    if rcode != 0 {
        return Some(DnsParseResult::ServerFailure);
    }

    let qdcount = u16::from_be_bytes([buf[4], buf[5]]) as usize;
    let ancount = u16::from_be_bytes([buf[6], buf[7]]) as usize;

    let mut pos = 12;
    for _ in 0..qdcount {
        pos = skip_dns_name(buf, pos)?;
        pos = pos.checked_add(4)?;
        if pos > buf.len() {
            return None;
        }
    }

    let mut found = Vec::new();
    for _ in 0..ancount {
        pos = skip_dns_name(buf, pos)?;
        if pos + 10 > buf.len() {
            return None;
        }
        let rtype = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        let rclass = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]);
        let rdlength = u16::from_be_bytes([buf[pos + 8], buf[pos + 9]]) as usize;
        pos += 10;
        if pos + rdlength > buf.len() {
            return None;
        }
        if rclass == 1 && rtype == expected_qtype {
            if rtype == QTYPE_A && rdlength == 4 {
                let ip = IpAddr::V4(Ipv4Addr::new(
                    buf[pos],
                    buf[pos + 1],
                    buf[pos + 2],
                    buf[pos + 3],
                ));
                if !found.contains(&ip) {
                    found.push(ip);
                }
            } else if rtype == QTYPE_AAAA && rdlength == 16 {
                let mut octets = [0u8; 16];
                octets.copy_from_slice(&buf[pos..pos + 16]);
                let ip = Peer::normalize_ip(IpAddr::V6(Ipv6Addr::from(octets)));
                if !found.contains(&ip) {
                    found.push(ip);
                }
            }
        }
        pos += rdlength;
    }

    if !found.is_empty() {
        Some(DnsParseResult::Found(found))
    } else {
        Some(DnsParseResult::NoDataOrNxDomain)
    }
}

async fn query_dns_over_servers(
    query: &[u8],
    tx_id: u16,
    qtype: u16,
    bind_addr: &str,
    servers: &[SocketAddr],
) -> Option<DnsParseResult> {
    let socket = UdpSocket::bind(bind_addr).await.ok()?;
    let mut recv_buf = [0u8; 512];

    for &ns in servers {
        if socket.send_to(query, ns).await.is_err() {
            continue;
        }

        let deadline = tokio::time::Instant::now() + Duration::from_millis(1500);
        while let Ok(Ok((n, _from))) =
            tokio::time::timeout_at(deadline, socket.recv_from(&mut recv_buf)).await
        {
            if let Some(res) = parse_dns_response(&recv_buf[..n], tx_id, qtype) {
                match res {
                    DnsParseResult::ServerFailure => break,
                    other => return Some(other),
                }
            }
        }
    }
    None
}

async fn query_single_qtype(host: &str, qtype: u16) -> Vec<IpAddr> {
    let tx_id: u16 = rand::random();
    let Some(query) = build_dns_query(host, tx_id, qtype) else {
        return Vec::new();
    };

    // Try IPv4 DNS transport first (fast path on IPv4 & dual-stack ISPs without waiting on broken IPv6),
    // then seamlessly fall back to IPv6 DNS servers when running on an IPv6-only network.
    if let Some(res) =
        query_dns_over_servers(&query, tx_id, qtype, "0.0.0.0:0", &DEFAULT_IPV4_DNS_SERVERS).await
    {
        return match res {
            DnsParseResult::Found(ips) => ips,
            DnsParseResult::NoDataOrNxDomain | DnsParseResult::ServerFailure => Vec::new(),
        };
    }

    if let Some(res) =
        query_dns_over_servers(&query, tx_id, qtype, "[::]:0", &DEFAULT_IPV6_DNS_SERVERS).await
    {
        if let DnsParseResult::Found(ips) = res {
            return ips;
        }
    }

    Vec::new()
}

async fn query_udp_dns_dual_stack(host: &str) -> Vec<IpAddr> {
    let (v4_ips, v6_ips) = tokio::join!(
        query_single_qtype(host, QTYPE_A),
        query_single_qtype(host, QTYPE_AAAA),
    );
    // Order IPv4 first, followed by IPv6: preserves instant connectivity on dual-stack ISPs
    // whose upstream IPv6 transit is broken, while automatically connecting over IPv6 when
    // either the tracker is IPv6-only or the client host is IPv6-only.
    let mut combined = Vec::with_capacity(v4_ips.len() + v6_ips.len());
    for ip in v4_ips.into_iter().chain(v6_ips.into_iter()) {
        if !combined.contains(&ip) {
            combined.push(ip);
        }
    }
    combined
}

struct InflightGuard {
    key: String,
    notify: Arc<Notify>,
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        if let Ok(mut map) = inflight_dns().lock() {
            map.remove(&self.key);
        }
        self.notify.notify_waiters();
    }
}

/// Pure-async, zero-OS-thread dual-stack DNS resolver (`A` + `AAAA`) with request coalescing
/// and positive/negative caching. Returns all resolved `SocketAddr`s (IPv4 ordered before IPv6).
pub(crate) async fn resolve_host_ports(
    host: &str,
    port: u16,
) -> Result<Vec<SocketAddr>, TrackerError> {
    let trimmed = host.trim_matches(|c| c == '[' || c == ']');
    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(Peer::normalize_ip(ip), port)]);
    }

    if trimmed.eq_ignore_ascii_case("localhost") {
        return Ok(vec![
            SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
            SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port),
        ]);
    }

    let key = trimmed.to_ascii_lowercase();

    loop {
        // Check cache
        {
            let mut cache = dns_cache().lock().unwrap();
            if let Some((ts, cached)) = cache.get(&key) {
                let ttl = if !cached.is_empty() {
                    DNS_POSITIVE_TTL
                } else {
                    DNS_NEGATIVE_TTL
                };
                if ts.elapsed() < ttl {
                    if cached.is_empty() {
                        return Err(TrackerError::NetworkError(format!(
                            "DNS lookup failed (cached) for {}:{}",
                            host, port
                        )));
                    }
                    return Ok(cached
                        .iter()
                        .map(|&ip| SocketAddr::new(ip, port))
                        .collect());
                } else {
                    cache.remove(&key);
                }
            }
        }

        // Coalesce concurrent lookups for the same hostname
        let wait_notify = {
            let mut inflight = inflight_dns().lock().unwrap();
            if let Some(existing) = inflight.get(&key) {
                Some(Arc::clone(existing))
            } else {
                let notify = Arc::new(Notify::new());
                inflight.insert(key.clone(), Arc::clone(&notify));
                None
            }
        };

        if let Some(notify) = wait_notify {
            let _ = tokio::time::timeout(Duration::from_secs(4), notify.notified()).await;
            continue;
        }

        let notify = {
            let inflight = inflight_dns().lock().unwrap();
            Arc::clone(inflight.get(&key).unwrap())
        };
        let _guard = InflightGuard {
            key: key.clone(),
            notify,
        };

        let resolved = query_udp_dns_dual_stack(&key).await;

        {
            let mut cache = dns_cache().lock().unwrap();
            if cache.len() >= 1024 {
                cache.retain(|_, (ts, val)| {
                    let ttl = if !val.is_empty() {
                        DNS_POSITIVE_TTL
                    } else {
                        DNS_NEGATIVE_TTL
                    };
                    ts.elapsed() < ttl
                });
            }
            cache.insert(key.clone(), (Instant::now(), resolved.clone()));
        }

        if resolved.is_empty() {
            return Err(TrackerError::NetworkError(format!(
                "Failed to resolve tracker host {}:{}",
                host, port
            )));
        }

        return Ok(resolved
            .into_iter()
            .map(|ip| SocketAddr::new(ip, port))
            .collect());
    }
}

pub struct Tracker {
    pub announce_url: String,
    pub peer_id: [u8; 20],
    pub port: String,
    pub info_hash: [u8; 20],
    pub downloaded: i64,
    pub uploaded: i64,
    pub remaining_size: i64,
    pub event: Option<TrackerEvents>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackerEvents {
    Started,
    Completed,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Peer {
    pub ip_addr: IpAddr,
    pub port: u16,
}

impl Peer {
    /// Unwraps IPv4-mapped IPv6 addresses (`::ffff:a.b.c.d`) into canonical `IpAddr::V4`
    /// so dual-stack listeners and peer pools never duplicate the same endpoint.
    pub fn normalize_ip(ip: IpAddr) -> IpAddr {
        match ip {
            IpAddr::V4(v4) => IpAddr::V4(v4),
            IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
                Some(v4) => IpAddr::V4(v4),
                None => IpAddr::V6(v6),
            },
        }
    }

    /// Validates whether an IP address is a routable swarm peer address (rejecting unspecified,
    /// loopback, broadcast, multicast, and IPv6 link-local `fe80::/10`).
    pub fn is_valid_peer_ip(ip: IpAddr) -> bool {
        match Self::normalize_ip(ip) {
            IpAddr::V4(v4) => {
                !v4.is_unspecified()
                    && !v4.is_broadcast()
                    && !v4.is_loopback()
                    && !v4.is_multicast()
            }
            IpAddr::V6(v6) => {
                !v6.is_unspecified()
                    && !v6.is_loopback()
                    && !v6.is_multicast()
                    && (v6.segments()[0] & 0xffc0) != 0xfe80
            }
        }
    }

    /// Parses compact 6-byte IPv4 peer entries (BEP 0023).
    pub fn parse_peers(peers_bytes: &[u8]) -> Vec<Self> {
        peers_bytes
            .chunks_exact(6)
            .filter_map(|chunk| {
                let ip = IpAddr::V4(Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]));
                let port = u16::from_be_bytes([chunk[4], chunk[5]]);
                if port != 0 && Self::is_valid_peer_ip(ip) {
                    Some(Peer { ip_addr: ip, port })
                } else {
                    None
                }
            })
            .collect()
    }

    /// Parses compact 18-byte IPv6 peer entries (BEP 0007 `peers6` / BEP 0011 `added6`).
    pub fn parse_peers6(peers6_bytes: &[u8]) -> Vec<Self> {
        peers6_bytes
            .chunks_exact(18)
            .filter_map(|chunk| {
                let mut octets = [0u8; 16];
                octets.copy_from_slice(&chunk[0..16]);
                let raw_v6 = Ipv6Addr::from(octets);
                let ip = Self::normalize_ip(IpAddr::V6(raw_v6));
                let port = u16::from_be_bytes([chunk[16], chunk[17]]);
                if port != 0 && Self::is_valid_peer_ip(ip) {
                    Some(Peer { ip_addr: ip, port })
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Tracker {
    pub async fn announce(&self) -> Result<TrackerResponse, TrackerError> {
        if self.announce_url.starts_with("http://") || self.announce_url.starts_with("https://") {
            return http_s::announce(self).await;
        } else if self.announce_url.starts_with("udp://") {
            return udp::announce(self).await;
        } else if self.announce_url.starts_with("ws://") || self.announce_url.starts_with("wss://")
        {
            return ws::announce(self).await;
        }

        Err(TrackerError::InvalidUrl(self.announce_url.clone()))
    }
}

#[derive(Debug)]
pub struct TrackerResponse {
    pub complete: usize,
    pub downloaded: usize,
    pub incomplete: usize,
    pub interval: usize,
    pub min_interval: usize,
    pub ipv4_peers: Vec<Peer>,
    pub ipv6_peers: Vec<Peer>,
}

impl TrackerResponse {
    /// Combines discovered IPv4 (`peers`) and IPv6 (`peers6`) endpoints into a single peer list.
    pub fn all_peers(mut self) -> Vec<Peer> {
        if self.ipv6_peers.is_empty() {
            return self.ipv4_peers;
        }
        if self.ipv4_peers.is_empty() {
            return self.ipv6_peers;
        }
        self.ipv4_peers.append(&mut self.ipv6_peers);
        self.ipv4_peers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_query_and_response_parser_a_and_aaaa() {
        let query = build_dns_query("tracker.opentrackr.org", 0x1234, QTYPE_A).unwrap();
        assert_eq!(&query[0..2], &[0x12, 0x34]);

        // Construct a synthetic DNS response with a CNAME answer followed by an A record answer
        let mut resp = Vec::new();
        resp.extend_from_slice(&0x1234u16.to_be_bytes()); // tx_id
        resp.extend_from_slice(&0x8180u16.to_be_bytes()); // QR=1, RD=1, RA=1, RCODE=0
        resp.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT=1
        resp.extend_from_slice(&2u16.to_be_bytes()); // ANCOUNT=2
        resp.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT=0
        resp.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT=0
        resp.extend_from_slice(&query[12..]);

        // Answer 1: CNAME (TYPE=5) using compression pointer 0xC00C
        resp.extend_from_slice(&[0xC0, 0x0C]);
        resp.extend_from_slice(&5u16.to_be_bytes()); // TYPE=CNAME
        resp.extend_from_slice(&1u16.to_be_bytes()); // CLASS=IN
        resp.extend_from_slice(&300u32.to_be_bytes()); // TTL
        resp.extend_from_slice(&2u16.to_be_bytes()); // RDLENGTH=2
        resp.extend_from_slice(&[0xC0, 0x0C]); // RDATA

        // Answer 2: A record (TYPE=1) -> 93.158.213.92
        resp.extend_from_slice(&[0xC0, 0x0C]);
        resp.extend_from_slice(&1u16.to_be_bytes()); // TYPE=A
        resp.extend_from_slice(&1u16.to_be_bytes()); // CLASS=IN
        resp.extend_from_slice(&300u32.to_be_bytes()); // TTL
        resp.extend_from_slice(&4u16.to_be_bytes()); // RDLENGTH=4
        resp.extend_from_slice(&[93, 158, 213, 92]);

        let parsed = parse_dns_response(&resp, 0x1234, QTYPE_A);
        assert_eq!(
            parsed,
            Some(DnsParseResult::Found(vec![IpAddr::V4(Ipv4Addr::new(
                93, 158, 213, 92
            ))]))
        );

        // Synthetic AAAA response (QTYPE=28)
        let q6 = build_dns_query("tracker6.example.org", 0x5678, QTYPE_AAAA).unwrap();
        let expected_v6: Ipv6Addr = "2001:db8:85a3::8a2e:370:7334".parse().unwrap();
        let mut resp6 = Vec::new();
        resp6.extend_from_slice(&0x5678u16.to_be_bytes());
        resp6.extend_from_slice(&0x8180u16.to_be_bytes());
        resp6.extend_from_slice(&1u16.to_be_bytes());
        resp6.extend_from_slice(&1u16.to_be_bytes());
        resp6.extend_from_slice(&0u16.to_be_bytes());
        resp6.extend_from_slice(&0u16.to_be_bytes());
        resp6.extend_from_slice(&q6[12..]);
        resp6.extend_from_slice(&[0xC0, 0x0C]);
        resp6.extend_from_slice(&QTYPE_AAAA.to_be_bytes());
        resp6.extend_from_slice(&1u16.to_be_bytes());
        resp6.extend_from_slice(&300u32.to_be_bytes());
        resp6.extend_from_slice(&16u16.to_be_bytes());
        resp6.extend_from_slice(&expected_v6.octets());

        let parsed6 = parse_dns_response(&resp6, 0x5678, QTYPE_AAAA);
        assert_eq!(
            parsed6,
            Some(DnsParseResult::Found(vec![IpAddr::V6(expected_v6)]))
        );
    }

    #[test]
    fn test_bep0007_parse_peers6_and_http_convert() {
        use hydroxy_bencode::decode::BencodedValue;
        use std::collections::BTreeMap;

        // Valid global IPv6: 2406:9cc0:200:1521::1 port 51413
        let valid_v6: Ipv6Addr = "2406:9cc0:200:1521::1".parse().unwrap();
        // Link-local IPv6 (must be rejected): fe80::1 port 6881
        let link_local_v6: Ipv6Addr = "fe80::1".parse().unwrap();
        // Loopback IPv6 (must be rejected): ::1 port 6881
        let loopback_v6 = Ipv6Addr::LOCALHOST;
        // IPv4-mapped IPv6 (must normalize to IpAddr::V4): ::ffff:203.0.113.9 port 8999
        let mapped_v6 = Ipv4Addr::new(203, 0, 113, 9).to_ipv6_mapped();

        let mut peers6_blob = Vec::new();
        for (ip, port) in [
            (valid_v6, 51413u16),
            (link_local_v6, 6881),
            (loopback_v6, 6881),
            (mapped_v6, 8999),
        ] {
            peers6_blob.extend_from_slice(&ip.octets());
            peers6_blob.extend_from_slice(&port.to_be_bytes());
        }

        let parsed = Peer::parse_peers6(&peers6_blob);
        assert_eq!(parsed.len(), 2);
        assert_eq!(
            parsed[0],
            Peer {
                ip_addr: IpAddr::V6(valid_v6),
                port: 51413,
            }
        );
        assert_eq!(
            parsed[1],
            Peer {
                ip_addr: IpAddr::V4(Ipv4Addr::new(203, 0, 113, 9)),
                port: 8999,
            }
        );

        // Verify http_s::convert merges both `peers` (IPv4) and `peers6` (IPv6)
        let mut dict = BTreeMap::new();
        dict.insert(b"interval".to_vec(), BencodedValue::Integer(1800));
        dict.insert(b"complete".to_vec(), BencodedValue::Integer(5));
        dict.insert(
            b"peers".to_vec(),
            BencodedValue::String(vec![80, 4, 4, 200, 0x1A, 0xE1]),
        );
        dict.insert(b"peers6".to_vec(), BencodedValue::String(peers6_blob));

        let resp = http_s::convert(&BencodedValue::Dictionary(dict)).unwrap();
        assert_eq!(resp.ipv4_peers.len(), 2); // 80.4.4.200 + normalized 203.0.113.9
        assert_eq!(resp.ipv6_peers.len(), 1); // 2406:9cc0:200:1521::1
        let combined = resp.all_peers();
        assert_eq!(combined.len(), 3);
    }
}
