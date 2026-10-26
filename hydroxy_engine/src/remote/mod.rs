use serde::Serialize;

use crate::stats::stats::{PeerStatsSnapshot, TorrentFileStats, TrackerStats};

pub mod api;
pub mod server;


#[derive(Serialize)]
pub struct TorrentApiItem {
    pub info_hash: String,
    pub name: String,
    pub category: String,
    pub size: u64,
    pub downloaded: u64,
    pub uploaded: u64,
    pub progress: f64,
    pub status: String,
    pub download_speed: f64,
    pub upload_speed: f64,
    pub peers: usize,
    pub total_pieces: usize,
    pub completed_pieces: usize,
    pub sequential: bool,
}

#[derive(Serialize)]
pub struct TorrentDetailApiItem {
    pub info_hash: String,
    pub name: String,
    pub category: String,
    pub size: u64,
    pub downloaded: u64,
    pub uploaded: u64,
    pub progress: f64,
    pub status: String,
    pub download_speed: f64,
    pub upload_speed: f64,
    pub peers: usize,
    pub total_pieces: usize,
    pub completed_pieces: usize,
    pub sequential: bool,
    pub pieces: String,
    pub peer_list: Vec<PeerStatsSnapshot>,
    pub trackers: Vec<TrackerStats>,
    pub files: Vec<TorrentFileStats>,
}