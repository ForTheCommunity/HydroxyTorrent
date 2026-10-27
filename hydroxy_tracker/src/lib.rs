use std::net::{Ipv4Addr, Ipv6Addr};

use crate::errors::TrackerError;

pub mod errors;
mod http_s;
mod udp;
mod ws;

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
