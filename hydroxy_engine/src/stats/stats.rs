use std::{
    collections::{HashMap, VecDeque},
    net::IpAddr,
    time::Duration,
};

use serde::Serialize;
use tokio::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum PeerConnectionState {
    Connecting,
    Connected,    // handshake done, waiting for unchoke
    Downloading,  // actively receiving piece data
    Choked,       // peer choked us
    Disconnected, // worker exited
}

pub struct PeerStats {
    pub ip_addr: IpAddr,
    pub port: u16,
    pub client_id: String,
    pub state: PeerConnectionState,
    pub downloaded_bytes: u64,
    pub uploaded_bytes: u64,
    pub download_speed: SpeedTracker,
    pub upload_speed: SpeedTracker,
    pub progress: f64,
    pub pieces_received: u32,
    pub connected_at: Instant,
    pub last_active: Instant,
}

impl PeerStats {
    pub fn new(ip_addr: IpAddr, port: u16, client_id: String) -> Self {
        let now = Instant::now();
        Self {
            ip_addr,
            port,
            client_id,
            state: PeerConnectionState::Connected,
            downloaded_bytes: 0,
            uploaded_bytes: 0,
            download_speed: SpeedTracker::new(Duration::from_secs(5)),
            upload_speed: SpeedTracker::new(Duration::from_secs(5)),
            progress: 0.0,
            pieces_received: 0,
            connected_at: now,
            last_active: now,
        }
    }

    pub fn to_snapshot(&self) -> PeerStatsSnapshot {
        PeerStatsSnapshot {
            ip_addr: self.ip_addr,
            port: self.port,
            client_id: self.client_id.clone(),
            state: self.state.clone(),
            downloaded_bytes: self.downloaded_bytes,
            uploaded_bytes: self.uploaded_bytes,
            download_speed: self.download_speed.speed_bps(),
            upload_speed: self.upload_speed.speed_bps(),
            progress: self.progress,
            pieces_received: self.pieces_received,
            connected_duration_secs: self.connected_at.elapsed().as_secs(),
        }
    }
}

pub struct TorrentStats {
    pub info_hash: String,
    pub name: String,
    pub category: String,
    pub total_pieces: usize,
    pub completed_pieces: usize,
    pub piece_length: u32,
    pub total_bytes: u64,
    pub downloaded_bytes: u64, // total dl bytes of a torrent.
    pub uploaded_bytes: u64,   // total up bytes of a torrent.
    pub download_speed: SpeedTracker,
    pub upload_speed: SpeedTracker,
    pub peers_stats: HashMap<(IpAddr, u16), PeerStats>,
    pub started_at: Instant,
    pub custom_status: Option<String>,
    pub sequential: bool,
    pub bitfield: Vec<u8>,
    pub trackers: Vec<TrackerStats>,
    pub files: Vec<TorrentFileStats>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct TrackerStats {
    pub tier: usize,
    pub url: String,
    pub status: String,
    pub peers: usize,
    #[serde(default)]
    pub interval: u64,
    #[serde(default)]
    pub next_announce: u64,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct TorrentFileStats {
    pub id: usize,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub offset: u64,
    pub progress: f64,
    pub priority: String,
}

impl TorrentStats {
    pub fn new(
        info_hash: String,
        name: String,
        category: String,
        total_pieces: usize,
        piece_length: u32,
        total_bytes: u64,
        trackers: Vec<TrackerStats>,
        files: Vec<TorrentFileStats>,
    ) -> Self {
        Self {
            info_hash,
            name,
            category,
            total_pieces,
            completed_pieces: 0,
            piece_length,
            total_bytes,
            downloaded_bytes: 0,
            uploaded_bytes: 0,
            download_speed: SpeedTracker::new(Duration::from_secs(5)),
            upload_speed: SpeedTracker::new(Duration::from_secs(5)),
            peers_stats: HashMap::new(),
            started_at: Instant::now(),
            custom_status: None,
            sequential: false,
            bitfield: Vec::new(),
            trackers,
            files,
        }
    }

    pub fn to_snapshot(&self) -> TorrentStatsSnapshot {
        let is_complete = self.total_pieces > 0 && self.completed_pieces >= self.total_pieces;
        let downloaded_bytes = if is_complete {
            self.total_bytes
        } else {
            self.downloaded_bytes.min(self.total_bytes)
        };

        TorrentStatsSnapshot {
            info_hash: self.info_hash.clone(),
            name: self.name.clone(),
            category: self.category.clone(),
            total_pieces: self.total_pieces,
            completed_pieces: self.completed_pieces,
            total_bytes: self.total_bytes,
            downloaded_bytes,
            uploaded_bytes: self.uploaded_bytes,
            download_speed: self.download_speed.speed_bps(),
            upload_speed: self.upload_speed.speed_bps(),
            active_peers: self.peers_stats.len(),
            started_duration_secs: self.started_at.elapsed().as_secs(),
            custom_status: self.custom_status.clone(),
            sequential: self.sequential,
        }
    }
}

// Speed Tracker.
pub struct SpeedTracker {
    samples: VecDeque<(Instant, u64)>, // (timestamp, cumulative_bytes)
    window: Duration,
    total_bytes: u64,
}

impl SpeedTracker {
    pub fn new(window: Duration) -> Self {
        Self {
            samples: VecDeque::new(),
            window,
            total_bytes: 0,
        }
    }

    pub fn record(&mut self, bytes: u64) {
        let now = Instant::now();
        self.total_bytes = self.total_bytes.saturating_add(bytes);
        self.cleanup(now);

        // Coalesce high-frequency block samples within a 50ms bucket to bound VecDeque size
        // (max ~100 entries per 5s window regardless of transfer speed) while keeping baseline sample intact.
        if self.samples.len() >= 2 {
            if let Some(back) = self.samples.back_mut() {
                if now.saturating_duration_since(back.0) < Duration::from_millis(50) {
                    back.0 = now;
                    back.1 = self.total_bytes;
                    return;
                }
            }
        }

        self.samples.push_back((now, self.total_bytes));
    }

    // calculates bytes per second.
    pub fn speed_bps(&self) -> f64 {
        let now = Instant::now();

        // if samples doesn't has starting & ending point.
        if self.samples.len() < 2 {
            return 0.0;
        }

        // newest sample (back of queue)
        let newest_sample = match self.samples.back() {
            Some(sample) if now.saturating_duration_since(sample.0) <= self.window => sample,
            _ => return 0.0, // No samples, or the newest sample is already older than the window (idle peer)
        };

        // oldest sample within the window
        let oldest_sample = match self
            .samples
            .iter()
            .find(|(ts, _)| now.saturating_duration_since(*ts) <= self.window)
        {
            Some(sample) => sample,
            None => return 0.0,
        };

        let elapsed = now.duration_since(oldest_sample.0).as_secs_f64();
        if elapsed < 0.05 {
            return 0.0;
        }
        let bytes_delta = newest_sample.1.saturating_sub(oldest_sample.1);
        bytes_delta as f64 / elapsed
    }

    fn cleanup(&mut self, now: Instant) {
        while let Some(&(timestamp, _)) = self.samples.front() {
            if now.saturating_duration_since(timestamp) > self.window {
                self.samples.pop_front();
            } else {
                break;
            }
        }
        if self.samples.capacity() > 128 && self.samples.len() < 32 {
            self.samples.shrink_to(64);
        }
    }
}

// === JSON Serializable Snapshots for HTTP API ===

#[derive(Serialize)]
pub struct TorrentStatsSnapshot {
    pub info_hash: String,
    pub name: String,
    pub category: String,
    pub total_pieces: usize,
    pub completed_pieces: usize,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub uploaded_bytes: u64,
    pub download_speed: f64,
    pub upload_speed: f64,
    pub active_peers: usize,
    pub started_duration_secs: u64,
    pub custom_status: Option<String>,
    pub sequential: bool,
}

#[derive(Serialize)]
pub struct PeerStatsSnapshot {
    pub ip_addr: IpAddr,
    pub port: u16,
    pub client_id: String,
    pub state: PeerConnectionState,
    pub downloaded_bytes: u64,
    pub uploaded_bytes: u64,
    pub download_speed: f64,
    pub upload_speed: f64,
    pub progress: f64,
    pub pieces_received: u32,
    pub connected_duration_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speed_tracker_coalesces_samples() {
        let mut tracker = SpeedTracker::new(Duration::from_secs(5));
        // Simulate 5,000 rapid 16 KB blocks arriving within the same millisecond burst
        for i in 1..=5000u64 {
            tracker.record(i * 16384);
        }
        // Coalescing within 50ms buckets keeps sample count tiny instead of 5,000 entries
        assert!(tracker.samples.len() <= 4);
    }
}
