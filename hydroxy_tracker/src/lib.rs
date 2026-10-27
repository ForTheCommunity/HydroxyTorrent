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

/// Hardcoded cross-platform DNS servers (Cloudflare & Google).
/// Can be overridden via config file in the future without any OS dependency.
const DEFAULT_DNS_SERVERS: [SocketAddr; 2] = [
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)), 53),
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8)), 53),
];

fn dns_cache() -> &'static Mutex<HashMap<String, (Instant, Option<IpAddr>)>> {
    static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Option<IpAddr>)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn inflight_dns() -> &'static Mutex<HashMap<String, Arc<Notify>>> {
    static INFLIGHT: OnceLock<Mutex<HashMap<String, Arc<Notify>>>> = OnceLock::new();
    INFLIGHT.get_or_init(|| Mutex::new(HashMap::new()))
}

fn build_dns_a_query(host: &str, tx_id: u16) -> Option<Vec<u8>> {
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
    packet.extend_from_slice(&1u16.to_be_bytes()); // QTYPE = A (1)
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
    Found(Ipv4Addr),
    NoDataOrNxDomain,
    ServerFailure,
}

fn parse_dns_a_response(buf: &[u8], expected_tx_id: u16) -> Option<DnsParseResult> {
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
        if rtype == 1 && rclass == 1 && rdlength == 4 {
            let ip = Ipv4Addr::new(buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]);
            return Some(DnsParseResult::Found(ip));
        }
        pos += rdlength;
    }

    Some(DnsParseResult::NoDataOrNxDomain)
}

async fn query_udp_dns(host: &str) -> Option<IpAddr> {
    let tx_id: u16 = rand::random();
    let query = build_dns_a_query(host, tx_id)?;
    let socket = UdpSocket::bind("0.0.0.0:0").await.ok()?;

    let mut recv_buf = [0u8; 512];
    for &ns in &DEFAULT_DNS_SERVERS {
        if socket.send_to(&query, ns).await.is_err() {
            continue;
        }

        let deadline = tokio::time::Instant::now() + Duration::from_millis(1500);
        while let Ok(Ok((n, _from))) =
            tokio::time::timeout_at(deadline, socket.recv_from(&mut recv_buf)).await
        {
            if let Some(res) = parse_dns_a_response(&recv_buf[..n], tx_id) {
                match res {
                    DnsParseResult::Found(ipv4) => return Some(IpAddr::V4(ipv4)),
                    DnsParseResult::NoDataOrNxDomain => return None,
                    DnsParseResult::ServerFailure => break,
                }
            }
        }
    }
    None
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

/// Pure-async, zero-OS-thread DNS resolver with request coalescing and positive/negative caching.
pub(crate) async fn resolve_host_port(host: &str, port: u16) -> Result<SocketAddr, TrackerError> {
    let trimmed = host.trim_matches(|c| c == '[' || c == ']');
    if let Ok(ip) = trimmed.parse::<IpAddr>() {
        return Ok(SocketAddr::new(ip, port));
    }

    if trimmed.eq_ignore_ascii_case("localhost") {
        return Ok(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port));
    }

    let key = trimmed.to_ascii_lowercase();

    loop {
        // 1. Check cache
        {
            let mut cache = dns_cache().lock().unwrap();
            if let Some(&(ts, cached)) = cache.get(&key) {
                let ttl = if cached.is_some() {
                    DNS_POSITIVE_TTL
                } else {
                    DNS_NEGATIVE_TTL
                };
                if ts.elapsed() < ttl {
                    return cached
                        .map(|ip| SocketAddr::new(ip, port))
                        .ok_or_else(|| {
                            TrackerError::NetworkError(format!(
                                "DNS lookup failed (cached) for {}:{}",
                                host, port
                            ))
                        });
                } else {
                    cache.remove(&key);
                }
            }
        }

        // 2. Coalesce concurrent lookups for the same hostname
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
            // Another task is already resolving this exact hostname; wait for it to finish
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

        // 3. Perform pure-async UDP DNS resolution (0 OS blocking threads!)
        let resolved = query_udp_dns(&key).await;

        {
            let mut cache = dns_cache().lock().unwrap();
            if cache.len() >= 1024 {
                cache.retain(|_, (ts, val)| {
                    let ttl = if val.is_some() {
                        DNS_POSITIVE_TTL
                    } else {
                        DNS_NEGATIVE_TTL
                    };
                    ts.elapsed() < ttl
                });
            }
            cache.insert(key.clone(), (Instant::now(), resolved));
        }

        return resolved
            .map(|ip| SocketAddr::new(ip, port))
            .ok_or_else(|| {
                TrackerError::NetworkError(format!(
                    "Failed to resolve tracker host {}:{}",
                    host, port
                ))
            });
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
    pub ipv4_addr: Ipv4Addr,
    pub port: u16,
}

impl Peer {
    pub fn parse_peers(peers_bytes: &[u8]) -> Vec<Self> {
        peers_bytes
            // a peer & it's port consumes 6 bytes.
            // 4 for ipv4 address & 2 for port.
            .chunks_exact(6)
            .filter_map(|a_6bytes_chunk| {
                let ipv4_addr = Ipv4Addr::new(
                    a_6bytes_chunk[0],
                    a_6bytes_chunk[1],
                    a_6bytes_chunk[2],
                    a_6bytes_chunk[3],
                );
                let port = u16::from_be_bytes([a_6bytes_chunk[4], a_6bytes_chunk[5]]);
                if !ipv4_addr.is_unspecified()
                    && !ipv4_addr.is_broadcast()
                    && !ipv4_addr.is_loopback()
                    && port != 0
                {
                    Some(Peer { ipv4_addr, port })
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
        } else if self.announce_url.starts_with("ws://") || self.announce_url.starts_with("wss://") {
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
    pub ipv6_peers: Vec<Ipv6Addr>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_query_and_response_parser() {
        let query = build_dns_a_query("tracker.opentrackr.org", 0x1234).unwrap();
        assert_eq!(&query[0..2], &[0x12, 0x34]);

        // Construct a synthetic DNS response with a CNAME answer followed by an A record answer
        let mut resp = Vec::new();
        resp.extend_from_slice(&0x1234u16.to_be_bytes()); // tx_id
        resp.extend_from_slice(&0x8180u16.to_be_bytes()); // QR=1, RD=1, RA=1, RCODE=0
        resp.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT=1
        resp.extend_from_slice(&2u16.to_be_bytes()); // ANCOUNT=2
        resp.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT=0
        resp.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT=0
        // Question section (copied from query)
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

        let parsed = parse_dns_a_response(&resp, 0x1234);
        assert_eq!(
            parsed,
            Some(DnsParseResult::Found(Ipv4Addr::new(93, 158, 213, 92)))
        );
    }
}
