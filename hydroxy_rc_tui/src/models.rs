use base64::Engine;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoginResponse {
    #[allow(dead_code)]
    pub status: String,
    pub token: String,
    pub username: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TorrentItem {
    pub info_hash: String,
    pub name: String,
    #[serde(default)]
    pub storage: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub downloaded: u64,
    #[serde(default)]
    pub uploaded: u64,
    #[serde(default)]
    pub progress: f64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub download_speed: f64,
    #[serde(default)]
    pub upload_speed: f64,
    #[serde(default)]
    pub peers: usize,
    #[serde(default)]
    pub total_pieces: usize,
    #[serde(default)]
    pub completed_pieces: usize,
    #[serde(default)]
    pub sequential: bool,
}

impl TorrentItem {
    pub fn ratio(&self) -> f64 {
        if self.downloaded == 0 {
            0.0
        } else {
            self.uploaded as f64 / self.downloaded as f64
        }
    }

    pub fn eta_secs(&self) -> Option<u64> {
        let st = self.status.to_ascii_lowercase();
        if st == "seeding" || st == "completed" || self.progress >= 1.0 {
            return Some(0);
        }
        if self.download_speed <= 1.0 || self.size <= self.downloaded {
            return None;
        }
        let remaining = self.size.saturating_sub(self.downloaded);
        Some((remaining as f64 / self.download_speed) as u64)
    }

    pub fn eta_string(&self) -> String {
        match self.eta_secs() {
            Some(0) => "Done".to_string(),
            Some(secs) => format_duration(secs),
            None => "∞".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[allow(dead_code)]
pub struct TorrentDetail {
    pub info_hash: String,
    pub name: String,
    #[serde(default)]
    pub storage: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub downloaded: u64,
    #[serde(default)]
    pub uploaded: u64,
    #[serde(default)]
    pub progress: f64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub download_speed: f64,
    #[serde(default)]
    pub upload_speed: f64,
    #[serde(default)]
    pub peers: usize,
    #[serde(default)]
    pub total_pieces: usize,
    #[serde(default)]
    pub completed_pieces: usize,
    #[serde(default)]
    pub sequential: bool,
    #[serde(default)]
    pub pieces: String,
    #[serde(default)]
    pub peer_list: Vec<PeerItem>,
    #[serde(default)]
    pub trackers: Vec<TrackerItem>,
    #[serde(default)]
    pub files: Vec<FileItem>,
}

impl TorrentDetail {
    pub fn decoded_bitfield(&self) -> Vec<u8> {
        if self.pieces.is_empty() {
            return Vec::new();
        }
        base64::engine::general_purpose::STANDARD
            .decode(self.pieces.trim())
            .unwrap_or_default()
    }

    pub fn sample_piece_buckets(&self, bucket_count: usize) -> Vec<f64> {
        if bucket_count == 0 {
            return Vec::new();
        }
        if self.total_pieces == 0 {
            return vec![0.0; bucket_count];
        }
        if self.completed_pieces >= self.total_pieces {
            return vec![1.0; bucket_count];
        }
        let raw = self.decoded_bitfield();
        if raw.is_empty() {
            return vec![0.0; bucket_count];
        }

        let mut buckets = Vec::with_capacity(bucket_count);
        for b in 0..bucket_count {
            let start_piece = (b * self.total_pieces) / bucket_count;
            let end_piece = (((b + 1) * self.total_pieces) / bucket_count).max(start_piece + 1);
            let mut have = 0usize;
            let mut count = 0usize;
            for p in start_piece..end_piece.min(self.total_pieces) {
                let byte_idx = p / 8;
                let bit_idx = 7 - (p % 8);
                if let Some(&byte) = raw.get(byte_idx) {
                    if ((byte >> bit_idx) & 1) == 1 {
                        have += 1;
                    }
                }
                count += 1;
            }
            let ratio = if count > 0 {
                have as f64 / count as f64
            } else {
                0.0
            };
            buckets.push(ratio);
        }
        buckets
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PeerItem {
    pub ip_addr: String,
    pub port: u16,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub downloaded_bytes: u64,
    #[serde(default)]
    pub uploaded_bytes: u64,
    #[serde(default)]
    pub download_speed: f64,
    #[serde(default)]
    pub upload_speed: f64,
    #[serde(default)]
    pub progress: f64,
    #[serde(default)]
    pub pieces_received: u32,
    #[serde(default)]
    pub connected_duration_secs: u64,
}

impl PeerItem {
    pub fn endpoint(&self) -> String {
        format!("{}:{}", self.ip_addr, self.port)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct TrackerItem {
    #[serde(default)]
    pub tier: usize,
    pub url: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub peers: usize,
    #[serde(default)]
    pub interval: u64,
    #[serde(default)]
    pub next_announce: u64,
}

impl TrackerItem {
    pub fn reannounce_in_string(&self) -> String {
        if self.next_announce == 0 {
            return "—".to_string();
        }
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if self.next_announce <= now_secs {
            "Due now".to_string()
        } else {
            format_duration(self.next_announce - now_secs)
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct FileItem {
    pub id: usize,
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub offset: u64,
    #[serde(default)]
    pub progress: f64,
    #[serde(default)]
    pub priority: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SystemStats {
    #[serde(default)]
    pub system_total_bytes: u64,
    #[serde(default)]
    pub system_used_bytes: u64,
    #[serde(default)]
    pub process_rss_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    All,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
    Normal,
}

impl LogLevel {
    pub fn detect(line: &str) -> Self {
        let upper = line.to_ascii_uppercase();
        if upper.contains("ERROR")
            || upper.contains("FATAL")
            || upper.contains("PANIC")
            || upper.contains("FAIL")
        {
            Self::Error
        } else if upper.contains("WARN") {
            Self::Warn
        } else if upper.contains("INFO") {
            Self::Info
        } else if upper.contains("DEBUG") {
            Self::Debug
        } else if upper.contains("TRACE") {
            Self::Trace
        } else {
            Self::Normal
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Error => "ERROR",
            Self::Warn => "WARN",
            Self::Info => "INFO",
            Self::Debug => "DEBUG",
            Self::Trace => "TRACE",
            Self::Normal => "RAW",
        }
    }

    pub fn next_filter(self) -> Self {
        match self {
            Self::All => Self::Error,
            Self::Error => Self::Warn,
            Self::Warn => Self::Info,
            Self::Info => Self::Debug,
            Self::Debug => Self::Trace,
            Self::Trace | Self::Normal => Self::All,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedLogLine {
    pub number: usize,
    pub text: String,
    pub level: LogLevel,
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;

    let b = bytes as f64;
    if b >= TB {
        format!("{:.2} TB", b / TB)
    } else if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.1} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_speed(bps: f64) -> String {
    if bps <= 0.5 {
        return "0 B/s".to_string();
    }
    format!("{}/s", format_bytes(bps as u64))
}

pub fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        format!("{}h {:02}m", h, m)
    } else {
        let d = secs / 86400;
        let h = (secs % 86400) / 3600;
        format!("{}d {}h", d, h)
    }
}

pub fn sanitize_relative_path(raw_path: &str, torrent_name: &str) -> String {
    let mut p = raw_path.replace('\\', "/");
    while p.starts_with('/') {
        p = p[1..].to_string();
    }
    let t_name = torrent_name.trim();
    if !t_name.is_empty() {
        if let Some(idx) = p.find(t_name) {
            return p[idx..].to_string();
        }
    }
    p
}

#[derive(Debug, Clone, Deserialize, Default)]
#[allow(dead_code)]
pub struct StorageTargetItem {
    pub name: String,
    #[serde(default)]
    pub watch_directory: String,
    #[serde(default)]
    pub download_directory: String,
    #[serde(default)]
    pub temp_directory: String,
    #[serde(default)]
    pub categories: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes_and_speed() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0 KB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(format_speed(0.0), "0 B/s");
        assert_eq!(format_speed(1024.0 * 1024.0), "1.00 MB/s");
    }

    #[test]
    fn test_sample_piece_buckets() {
        let mut detail = TorrentDetail {
            total_pieces: 8,
            completed_pieces: 4,
            // 0b11110000 = 240 -> base64 "8A=="
            pieces: base64::engine::general_purpose::STANDARD.encode([0b1111_0000]),
            ..Default::default()
        };
        let buckets = detail.sample_piece_buckets(2);
        assert_eq!(buckets, vec![1.0, 0.0]);

        detail.completed_pieces = 8;
        assert_eq!(detail.sample_piece_buckets(4), vec![1.0, 1.0, 1.0, 1.0]);
    }
}
