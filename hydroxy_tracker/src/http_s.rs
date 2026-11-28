use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr, UdpSocket},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use hydroxy_bencode::decode::{BencodedValue, decode};
use reqwest::dns::{Addrs, Name, Resolve, Resolving};

use crate::{Peer, Tracker, TrackerEvents, TrackerResponse, errors::TrackerError};

const LOCAL_IPV6_CACHE_TTL: Duration = Duration::from_secs(60);

/// Detects a local Global Unicast IPv6 address (`2000::/3`, excluding documentation `2001:db8::/32`)
/// via a zero-packet kernel routing lookup, cached for 60s for BEP 0007 `&ipv6=` announces.
fn detect_global_ipv6() -> Option<Ipv6Addr> {
    static CACHE: OnceLock<Mutex<Option<(Instant, Option<Ipv6Addr>)>>> = OnceLock::new();
    let mutex = CACHE.get_or_init(|| Mutex::new(None));

    if let Ok(guard) = mutex.lock() {
        if let Some((ts, cached)) = *guard {
            if ts.elapsed() < LOCAL_IPV6_CACHE_TTL {
                return cached;
            }
        }
    }

    let detected = (|| {
        let sock = UdpSocket::bind("[::]:0").ok()?;
        // Purely local kernel route selection — sends 0 network packets on the wire
        sock.connect("[2606:4700:4700::1111]:80").ok()?;
        if let Ok(SocketAddr::V6(local_v6)) = sock.local_addr() {
            let ip = *local_v6.ip();
            let segs = ip.segments();
            let is_global_unicast = (segs[0] & 0xe000) == 0x2000;
            let is_documentation = segs[0] == 0x2001 && segs[1] == 0x0db8;
            if is_global_unicast && !is_documentation {
                return Some(ip);
            }
        }
        None
    })();

    if let Ok(mut guard) = mutex.lock() {
        *guard = Some((Instant::now(), detected));
    }

    detected
}

struct HydroxyDnsResolver;

impl Resolve for HydroxyDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let addr = crate::resolve_host_port(&host, 0)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            let addrs: Addrs = Box::new(std::iter::once(addr));
            Ok(addrs)
        })
    }
}

fn shared_http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .dns_resolver(Arc::new(HydroxyDnsResolver))
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(8))
            .pool_max_idle_per_host(4)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

pub async fn announce(tracker: &Tracker) -> Result<TrackerResponse, TrackerError> {
    let mut url = format!(
        "{}?info_hash={}&peer_id={}&port={}&uploaded={}&downloaded={}&left={}&compact=1",
        tracker.announce_url,
        urlencoding::encode_binary(&tracker.info_hash).to_string(),
        urlencoding::encode_binary(&tracker.peer_id).to_string(),
        tracker.port,
        tracker.uploaded,
        tracker.downloaded,
        tracker.remaining_size,
    );

    if let Some(ref event) = tracker.event {
        let event_str = match event {
            TrackerEvents::Started => "started",
            TrackerEvents::Completed => "completed",
            TrackerEvents::Stopped => "stopped",
        };
        url.push_str("&event=");
        url.push_str(event_str);
    }

    // BEP 0007: Advertise our global unicast IPv6 address (if available) so dual-stack
    // and intra-ISP IPv6 peers can discover and connect to us even over IPv4 trackers.
    if let Some(local_v6) = detect_global_ipv6() {
        let v6_str = local_v6.to_string();
        url.push_str("&ipv6=");
        url.push_str(&urlencoding::encode(&v6_str));
    }

    let response = shared_http_client()
        .get(url)
        .send()
        .await
        .map_err(|e| TrackerError::NetworkError(e.to_string()))?;

    if !response.status().is_success() && response.status() != reqwest::StatusCode::OK {
        return Err(TrackerError::NetworkError(format!(
            "HTTP Error: {}",
            response.status()
        )));
    }

    let response_bytes = response
        .bytes()
        .await
        .map_err(|e| TrackerError::TrackerResponseParseError(e.to_string()))?;

    let decoded_response = decode(&response_bytes).map_err(|e| TrackerError::DecodeError(e))?;

    let tracker_response = convert(&decoded_response)
        .map_err(|e| TrackerError::TrackerResponseParseError(e.to_string()))?;

    Ok(tracker_response)
}

pub fn convert(decoded_tracker_response: &BencodedValue) -> Result<TrackerResponse, TrackerError> {
    let tracker_response = if let BencodedValue::Dictionary(dict) = decoded_tracker_response {
        // completed peers count.
        let complete = match dict.get(&b"complete"[..]) {
            Some(BencodedValue::Integer(completed_peers_count)) => *completed_peers_count as usize,
            _ => 0,
        };

        // downloaded peers count.
        let downloaded = match dict.get(&b"downloaded"[..]) {
            Some(BencodedValue::Integer(downloaded_peers_count)) => {
                *downloaded_peers_count as usize
            }
            _ => 0,
        };

        // incomplete peers count.
        let incomplete = match dict.get(&b"incomplete"[..]) {
            Some(BencodedValue::Integer(incomplete_peers_count)) => {
                *incomplete_peers_count as usize
            }
            _ => 0,
        };

        // interval
        let interval = match dict.get(&b"interval"[..]) {
            Some(BencodedValue::Integer(interval)) => *interval as usize,
            _ => 0,
        };

        // min interval
        let min_interval = match dict.get(&b"min interval"[..]) {
            Some(BencodedValue::Integer(min_interval)) => *min_interval as usize,
            _ => 0,
        };

        // IPv4 Peers (`peers` compact 6-byte string or non-compact dictionary list)
        let mut ipv4_peers = Vec::new();
        let mut ipv6_peers = Vec::new();

        match dict.get(&b"peers"[..]) {
            Some(BencodedValue::String(bytes)) => {
                ipv4_peers.extend(Peer::parse_peers(bytes));
            }
            Some(BencodedValue::List(items)) => {
                for item in items {
                    if let BencodedValue::Dictionary(pdict) = item {
                        let ip_opt = match pdict.get(&b"ip"[..]) {
                            Some(BencodedValue::String(s)) => std::str::from_utf8(s)
                                .ok()
                                .and_then(|txt| txt.trim_matches(|c| c == '[' || c == ']').parse::<IpAddr>().ok()),
                            _ => None,
                        };
                        let port_opt = match pdict.get(&b"port"[..]) {
                            Some(BencodedValue::Integer(p)) if *p > 0 && *p <= 65535 => {
                                Some(*p as u16)
                            }
                            _ => None,
                        };
                        if let (Some(raw_ip), Some(port)) = (ip_opt, port_opt) {
                            let ip = Peer::normalize_ip(raw_ip);
                            if Peer::is_valid_peer_ip(ip) {
                                let peer = Peer { ip_addr: ip, port };
                                match ip {
                                    IpAddr::V4(_) => ipv4_peers.push(peer),
                                    IpAddr::V6(_) => ipv6_peers.push(peer),
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        // BEP 0007: Compact 18-byte IPv6 Peers (`peers6`)
        if let Some(BencodedValue::String(bytes6)) = dict.get(&b"peers6"[..]) {
            for peer in Peer::parse_peers6(bytes6) {
                match peer.ip_addr {
                    IpAddr::V4(_) => ipv4_peers.push(peer),
                    IpAddr::V6(_) => ipv6_peers.push(peer),
                }
            }
        }

        Ok(TrackerResponse {
            complete,
            downloaded,
            incomplete,
            interval,
            min_interval,
            ipv4_peers,
            ipv6_peers,
        })
    } else {
        Err(TrackerError::TrackerResponseParseError(
            "Error Invalid Tracker Response. !!!!".into(),
        ))
    };
    tracker_response
}
