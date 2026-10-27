use std::net::IpAddr;

use dashmap::DashMap;
use tokio::time::Instant;

use crate::stats::stats::{
    PeerConnectionState, PeerStats, PeerStatsSnapshot, TorrentFileStats, TorrentStats,
    TorrentStatsSnapshot, TrackerStats,
};

// Global Thread Safe Registry
#[derive(Default)]
pub struct StatsRegistry {
    torrents: DashMap<String, TorrentStats>,
}

impl StatsRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    // called by a Torrent Task when it starts...
    pub fn register_torrent(
        &self,
        info_hash: &str,
        name: &str,
        category: &str,
        total_pieces: usize,
        piece_length: u32,
        total_bytes: u64,
        trackers: Vec<TrackerStats>,
        files: Vec<TorrentFileStats>,
    ) {
        self.torrents
            .entry(info_hash.to_string())
            .or_insert_with(|| {
                TorrentStats::new(
                    info_hash.to_string(),
                    name.to_string(),
                    category.to_string(),
                    total_pieces,
                    piece_length,
                    total_bytes,
                    trackers,
                    files,
                )
            });
    }

    /// Unregister / remove torrent from the registry.
    pub fn unregister_torrent(&self, info_hash: &str) {
        self.torrents.remove(info_hash);
    }

    // called by peerworker when handshake completes.
    pub fn peer_connected(&self, info_hash: &str, ip_addr: IpAddr, port: u16, client_id: String) {
        if let Some(mut torrent) = self.torrents.get_mut(info_hash) {
            torrent
                .peers_stats
                .entry((ip_addr, port))
                .or_insert_with(|| PeerStats::new(ip_addr, port, client_id));
        }
    }

    // called by PeerWorker on every received block.
    pub fn record_download(&self, info_hash: &str, ip_addr: IpAddr, port: u16, bytes: u64) {
        if let Some(mut torrent) = self.torrents.get_mut(info_hash) {
            torrent.downloaded_bytes = (torrent.downloaded_bytes + bytes).min(torrent.total_bytes);
            torrent.download_speed.record(bytes);

            if let Some(peer) = torrent.peers_stats.get_mut(&(ip_addr, port)) {
                peer.downloaded_bytes += bytes;
                peer.download_speed.record(bytes);
                peer.last_active = Instant::now();
                peer.state = PeerConnectionState::Downloading
            }
        }
    }

    // Called by PeerWorker whenever a block is uploaded to a peer.
    pub fn record_upload(&self, info_hash: &str, ip_addr: IpAddr, port: u16, bytes: u64) {
        if let Some(mut torrent) = self.torrents.get_mut(info_hash) {
            torrent.uploaded_bytes += bytes;
            torrent.upload_speed.record(bytes);

            if let Some(peer) = torrent.peers_stats.get_mut(&(ip_addr, port)) {
                peer.uploaded_bytes += bytes;
                peer.upload_speed.record(bytes);
                peer.last_active = Instant::now();
            }
        }
    }

    /// Called by `PeerWorker` after SHA1 piece verification succeeds.
    pub fn piece_completed(&self, info_hash: &str, ip: IpAddr, port: u16, piece_index: usize) {
        if let Some(mut torrent) = self.torrents.get_mut(info_hash) {
            let byte_idx = piece_index / 8;
            let bit_idx = 7 - (piece_index % 8);
            let is_new_piece = if byte_idx < torrent.bitfield.len() {
                let was_set = (torrent.bitfield[byte_idx] & (1 << bit_idx)) != 0;
                torrent.bitfield[byte_idx] |= 1 << bit_idx;
                !was_set
            } else {
                false
            };

            if is_new_piece {
                torrent.completed_pieces += 1;
                if torrent.total_pieces > 0 && torrent.completed_pieces >= torrent.total_pieces {
                    torrent.downloaded_bytes = torrent.total_bytes;
                }
            }

            if let Some(peer) = torrent.peers_stats.get_mut(&(ip, port)) {
                peer.pieces_received += 1;
            }
        }
    }

    /// Called by `PeerWorker` when connection state transitions (e.g. Choke/Unchoke).
    pub fn update_peer_state(
        &self,
        info_hash: &str,
        ip: IpAddr,
        port: u16,
        state: PeerConnectionState,
    ) {
        if let Some(mut torrent) = self.torrents.get_mut(info_hash) {
            if let Some(peer) = torrent.peers_stats.get_mut(&(ip, port)) {
                peer.state = state;
            }
        }
    }

    /// Called by `PeerWorker` when peer advertises available pieces.
    pub fn update_peer_progress(
        &self,
        info_hash: &str,
        ip: IpAddr,
        port: u16,
        progress: f64,
    ) {
        if let Some(mut torrent) = self.torrents.get_mut(info_hash) {
            if let Some(peer) = torrent.peers_stats.get_mut(&(ip, port)) {
                peer.progress = progress;
            }
        }
    }

    /// Called by `PeerWorker` on worker exit.
    pub fn peer_disconnected(&self, info_hash: &str, ip: IpAddr, port: u16) {
        if let Some(mut torrent) = self.torrents.get_mut(info_hash) {
            torrent.peers_stats.remove(&(ip, port));
        }
    }

    /// Clear all peer stats for a torrent (e.g. when pausing, rechecking, or stopping all workers).
    pub fn clear_peers(&self, info_hash: &str) {
        if let Some(mut torrent) = self.torrents.get_mut(info_hash) {
            torrent.peers_stats.clear();
        }
    }

    /// Called by HTTP API to fetch all active torrent metrics.
    pub fn snapshot_all(&self) -> Vec<TorrentStatsSnapshot> {
        self.torrents
            .iter()
            .map(|entry| entry.value().to_snapshot())
            .collect()
    }

    /// Returns IPv4 (ip, port) pairs of peers that have completed a handshake for PEX advertisement.
    pub fn active_peer_endpoints(&self, info_hash: &str) -> Vec<( std::net::Ipv4Addr, u16 )> {
        self.torrents
            .get(info_hash)
            .map(|torrent| {
                torrent
                    .peers_stats
                    .keys()
                    .filter_map(|&(ip, port)| match ip {
                        IpAddr::V4(v4) => Some((v4, port)),
                        IpAddr::V6(_) => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Called by HTTP API to fetch detailed peer stats for a given torrent.
    pub fn snapshot_peers(&self, info_hash: &str) -> Vec<PeerStatsSnapshot> {
        self.torrents
            .get(info_hash)
            .map(|torrent| {
                torrent
                    .peers_stats
                    .values()
                    .map(|peer| peer.to_snapshot())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Called by HTTP API to fetch snapshot for a single torrent.
    pub fn snapshot_torrent(&self, info_hash: &str) -> Option<TorrentStatsSnapshot> {
        self.torrents
            .get(info_hash)
            .map(|torrent| torrent.to_snapshot())
    }

    /// Set initial bitfield and progress from saved state.
    pub fn set_initial_bitfield(
        &self,
        info_hash: &str,
        bitfield: &[u8],
        completed_pieces: usize,
        downloaded_bytes: u64,
    ) {
        if let Some(mut torrent) = self.torrents.get_mut(info_hash) {
            torrent.bitfield = bitfield.to_vec();
            torrent.completed_pieces = completed_pieces;
            torrent.downloaded_bytes = if torrent.total_pieces > 0 && completed_pieces >= torrent.total_pieces {
                torrent.total_bytes
            } else {
                downloaded_bytes.min(torrent.total_bytes)
            };
        }
    }

    /// Set initial progress from saved state.
    pub fn set_initial_progress(&self, info_hash: &str, completed_pieces: usize, downloaded_bytes: u64) {
        if let Some(mut torrent) = self.torrents.get_mut(info_hash) {
            torrent.completed_pieces = completed_pieces;
            torrent.downloaded_bytes = if torrent.total_pieces > 0 && completed_pieces >= torrent.total_pieces {
                torrent.total_bytes
            } else {
                downloaded_bytes.min(torrent.total_bytes)
            };
        }
    }

    /// Update status, peer count, interval, and next announce time for a tracker URL.
    pub fn update_tracker(
        &self,
        info_hash: &str,
        url: &str,
        status: &str,
        peers: Option<usize>,
        interval: Option<u64>,
        next_announce: Option<u64>,
    ) {
        if let Some(mut torrent) = self.torrents.get_mut(info_hash) {
            if let Some(tracker) = torrent.trackers.iter_mut().find(|t| t.url == url) {
                tracker.status = status.to_string();
                if let Some(p) = peers {
                    tracker.peers = p;
                }
                if let Some(i) = interval {
                    tracker.interval = i;
                }
                if let Some(na) = next_announce {
                    tracker.next_announce = na;
                }
            } else {
                let tier = torrent.trackers.len() + 1;
                torrent.trackers.push(TrackerStats {
                    tier,
                    url: url.to_string(),
                    status: status.to_string(),
                    peers: peers.unwrap_or(0),
                    interval: interval.unwrap_or(0),
                    next_announce: next_announce.unwrap_or(0),
                });
            }
        }
    }

    /// Called by HTTP API to fetch tracker list for a given torrent.
    pub fn snapshot_trackers(&self, info_hash: &str) -> Vec<TrackerStats> {
        self.torrents
            .get(info_hash)
            .map(|torrent| torrent.trackers.clone())
            .unwrap_or_default()
    }

    /// Called by HTTP API to fetch file list with calculated download progress for a given torrent.
    pub fn snapshot_files(&self, info_hash: &str) -> Vec<TorrentFileStats> {
        if let Some(torrent) = self.torrents.get(info_hash) {
            let piece_len = torrent.piece_length as u64;
            let is_all_complete =
                torrent.total_pieces > 0 && torrent.completed_pieces >= torrent.total_pieces;
            torrent
                .files
                .iter()
                .map(|f| {
                    let mut file = f.clone();
                    if is_all_complete || f.size == 0 {
                        file.progress = 1.0;
                    } else if piece_len > 0 {
                        let first_piece = (f.offset / piece_len) as usize;
                        let last_piece = ((f.offset + f.size - 1) / piece_len) as usize;
                        let mut completed_bytes = 0u64;
                        for p in first_piece..=last_piece {
                            let byte_idx = p / 8;
                            let bit_idx = 7 - (p % 8);
                            let is_piece_done = if byte_idx < torrent.bitfield.len() {
                                (torrent.bitfield[byte_idx] & (1 << bit_idx)) != 0
                            } else {
                                false
                            };
                            if is_piece_done {
                                let p_start = p as u64 * piece_len;
                                let p_end = (p_start + piece_len).min(torrent.total_bytes);
                                let overlap_start = f.offset.max(p_start);
                                let overlap_end = (f.offset + f.size).min(p_end);
                                if overlap_end > overlap_start {
                                    completed_bytes += overlap_end - overlap_start;
                                }
                            }
                        }
                        file.progress =
                            (completed_bytes as f64 / f.size as f64).clamp(0.0, 1.0);
                    }
                    file
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Called by HTTP API to fetch bitfield snapshot for a given torrent.
    pub fn snapshot_bitfield(&self, info_hash: &str) -> Vec<u8> {
        if let Some(torrent) = self.torrents.get(info_hash) {
            if torrent.total_pieces > 0 && torrent.completed_pieces >= torrent.total_pieces {
                let num_bytes = (torrent.total_pieces + 7) / 8;
                let mut full = vec![0xFF; num_bytes];
                let rem = torrent.total_pieces % 8;
                if rem > 0 {
                    full[num_bytes - 1] = !((1u8 << (8 - rem)) - 1);
                }
                return full;
            }
            torrent.bitfield.clone()
        } else {
            Vec::new()
        }
    }

    /// Set custom status string for torrent (e.g. "paused", "checking").
    pub fn set_status(&self, info_hash: &str, status: Option<String>) {
        if let Some(mut torrent) = self.torrents.get_mut(info_hash) {
            torrent.custom_status = status;
        }
    }

    /// Set sequential download flag for torrent.
    pub fn set_sequential(&self, info_hash: &str, sequential: bool) {
        if let Some(mut torrent) = self.torrents.get_mut(info_hash) {
            torrent.sequential = sequential;
        }
    }

    /// Set category for torrent.
    pub fn set_category(&self, info_hash: &str, category: &str) {
        if let Some(mut torrent) = self.torrents.get_mut(info_hash) {
            torrent.category = category.to_string();
        }
    }
}

