//! BEP 0019: WebSeed - HTTP/FTP Seeding (GetRight-style)
//!
//! Implements HTTP/HTTPS WebSeeding using standard RFC 9110 `Range: bytes=start-end`
//! requests for both single-file and multi-file torrents, including pieces that span
//! multiple file boundaries, `206 Partial Content` / full-file `200 OK` validation,
//! `503 Service Unavailable` `Retry-After` backoff, streaming chunk speed accounting,
//! and SHA-1 piece verification.

use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr},
    sync::{Arc, OnceLock},
    time::Duration,
};

use hydroxy_state::TorrentState;
use hydroxy_storage_io::{file_region::FileRegion, piece_io::PieceIO};
use hydroxy_utils::torrent_info::{OtherInfos, TorrentInfo};
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use tokio::sync::{Mutex, RwLock, Semaphore, broadcast};
use tokio::time::Instant;
use tracing::{debug, info, warn};

use crate::{
    errors::EngineError,
    piece_allocator::PieceAllocator,
    piece_assembler::PieceAssembler,
    stats::{stats::PeerConnectionState, stats_registry::StatsRegistry},
};

/// Maximum concurrent HTTP WebSeed transfers per torrent so large mirror lists stay polite.
pub const MAX_CONCURRENT_WEBSEEDS_PER_TORRENT: usize = 4;

const HEADER_TIMEOUT: Duration = Duration::from_secs(15);
const CHUNK_READ_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_CONSECUTIVE_ERRORS: u32 = 5;
const MAX_CORRUPT_PIECES: u32 = 2;

struct WebSeedDnsResolver;

impl Resolve for WebSeedDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let addrs_vec = hydroxy_tracker::resolve_host_ports(&host, 0)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            let addrs: Addrs = Box::new(addrs_vec.into_iter());
            Ok(addrs)
        })
    }
}

fn shared_webseed_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .dns_resolver(Arc::new(WebSeedDnsResolver))
            .user_agent("HydroxyTorrent/0.1")
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(5))
            .pool_max_idle_per_host(4)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

/// Extracts, normalizes, and deduplicates supported HTTP/HTTPS WebSeed URLs from `url-list`.
pub fn extract_http_webseeds(raw_urls: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for raw in raw_urls {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(parsed) = url::Url::parse(trimmed) {
            let scheme = parsed.scheme();
            if (scheme == "http" || scheme == "https") && parsed.host_str().is_some() {
                if seen.insert(trimmed.to_string()) {
                    out.push(trimmed.to_string());
                }
            }
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct WebSeedFileEntry {
    pub rel_segments: Vec<String>,
    pub offset_in_torrent: u64,
    pub length: u64,
}

#[derive(Debug, Clone)]
pub struct WebSeedLayout {
    pub is_multi_file: bool,
    pub torrent_name: String,
    pub files: Vec<WebSeedFileEntry>,
}

impl WebSeedLayout {
    pub fn from_torrent_info(torrent_info: &TorrentInfo) -> Self {
        if torrent_info.info.files.is_empty() {
            let length = torrent_info.info.length.unwrap_or(0) as u64;
            Self {
                is_multi_file: false,
                torrent_name: torrent_info.info.name.clone(),
                files: vec![WebSeedFileEntry {
                    rel_segments: Vec::new(),
                    offset_in_torrent: 0,
                    length,
                }],
            }
        } else {
            let mut offset = 0u64;
            let mut files = Vec::with_capacity(torrent_info.info.files.len());
            for f in &torrent_info.info.files {
                let len = f.length as u64;
                files.push(WebSeedFileEntry {
                    rel_segments: f.path.clone(),
                    offset_in_torrent: offset,
                    length: len,
                });
                offset += len;
            }
            Self {
                is_multi_file: true,
                torrent_name: torrent_info.info.name.clone(),
                files,
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSeedRangeRequest {
    pub url: String,
    pub range_start: u64,
    pub range_end_inclusive: u64,
    pub expected_len: usize,
    pub full_file_len: u64,
}

fn encode_clean_segments(raw: &str) -> Result<Vec<String>, EngineError> {
    if raw.is_empty() || raw.contains('\0') {
        return Err(EngineError::Error(
            "Invalid empty or NUL path segment in torrent metadata".into(),
        ));
    }
    let mut encoded = Vec::new();
    for part in raw.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(EngineError::Error(format!(
                "Unsafe path segment '{}' in torrent metadata",
                part
            )));
        }
        encoded.push(urlencoding::encode(part).into_owned());
    }
    Ok(encoded)
}

/// Constructs the exact BEP 0019 target file URL for single-file and multi-file torrents.
///
/// - **Single-file**:
///   - If `base_url` does not end with `/` (and has an explicit path), `base_url` is used directly.
///   - If `base_url` ends with `/` (or is a bare host origin), `<info.name>` is percent-encoded and appended.
/// - **Multi-file**:
///   - Appends `<info.name>/<file.path...>` onto `base_url` (with RFC 3986 path segment encoding),
///     preserving any query string parameters (e.g. signed CDN tokens).
pub fn build_webseed_file_url(
    base_url: &str,
    is_multi_file: bool,
    torrent_name: &str,
    file_segments: &[String],
) -> Result<String, EngineError> {
    let mut parsed = url::Url::parse(base_url)
        .map_err(|e| EngineError::Error(format!("Invalid WebSeed URL '{}': {}", base_url, e)))?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(EngineError::Error(format!(
            "Unsupported WebSeed scheme '{}'",
            scheme
        )));
    }

    let raw_no_query = base_url.split(&['?', '#'][..]).next().unwrap_or(base_url);
    let has_trailing_slash = raw_no_query.ends_with('/');
    let after_scheme = raw_no_query
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or("");
    let is_bare_origin = !after_scheme.contains('/');

    if !is_multi_file {
        if has_trailing_slash || is_bare_origin {
            let name_parts = encode_clean_segments(torrent_name)?;
            let base_path = parsed.path().trim_end_matches('/');
            let new_path = format!("{}/{}", base_path, name_parts.join("/"));
            parsed.set_path(&new_path);
        }
        return Ok(parsed.to_string());
    }

    if file_segments.is_empty() {
        return Err(EngineError::Error(
            "Multi-file torrent entry has empty path segments".into(),
        ));
    }

    let mut all_parts = encode_clean_segments(torrent_name)?;
    for seg in file_segments {
        all_parts.extend(encode_clean_segments(seg)?);
    }

    let base_path = parsed.path().trim_end_matches('/');
    let new_path = format!("{}/{}", base_path, all_parts.join("/"));
    parsed.set_path(&new_path);
    Ok(parsed.to_string())
}

/// Plans the ordered HTTP `Range` requests required to assemble `piece_index` (including pieces
/// that span across multiple files in multi-file torrents, while skipping 0-byte files).
pub fn plan_piece_requests(
    base_url: &str,
    layout: &WebSeedLayout,
    piece_index: u32,
    piece_length: u32,
    total_pieces: usize,
    total_torrent_size: u64,
) -> Result<Vec<WebSeedRangeRequest>, EngineError> {
    if (piece_index as usize) >= total_pieces || total_torrent_size == 0 {
        return Err(EngineError::Error(format!(
            "Out-of-bounds piece {} for WebSeed request",
            piece_index
        )));
    }

    let actual_len = PieceAssembler::actual_piece_length(
        piece_index,
        total_pieces as u32,
        piece_length,
        total_torrent_size,
    );
    if actual_len == 0 {
        return Ok(Vec::new());
    }

    let piece_start = piece_index as u64 * piece_length as u64;
    let piece_end = piece_start + actual_len as u64;
    let mut requests = Vec::new();

    for file in &layout.files {
        if file.length == 0 {
            continue;
        }
        let file_end = file.offset_in_torrent + file.length;
        if piece_start >= file_end || piece_end <= file.offset_in_torrent {
            continue;
        }

        let overlap_start = piece_start.max(file.offset_in_torrent);
        let overlap_end = piece_end.min(file_end);
        let slice_len = (overlap_end - overlap_start) as usize;
        let range_start = overlap_start - file.offset_in_torrent;
        let range_end_inclusive = overlap_end - file.offset_in_torrent - 1;

        let url = build_webseed_file_url(
            base_url,
            layout.is_multi_file,
            &layout.torrent_name,
            &file.rel_segments,
        )?;

        requests.push(WebSeedRangeRequest {
            url,
            range_start,
            range_end_inclusive,
            expected_len: slice_len,
            full_file_len: file.length,
        });
    }

    Ok(requests)
}

/// Ensures the WebSeed's `StatsRegistry` entry and any leased piece are cleaned up even if aborted.
struct WebSeedCleanupGuard {
    stats_registry: Arc<StatsRegistry>,
    info_hash_sha1: String,
    peer_ip: IpAddr,
    peer_port: u16,
}

impl Drop for WebSeedCleanupGuard {
    fn drop(&mut self) {
        self.stats_registry
            .peer_disconnected(&self.info_hash_sha1, self.peer_ip, self.peer_port);
    }
}

pub struct WebSeedWorker {
    pub seed_index: usize,
    pub base_url: String,
    pub info_hash: [u8; 20],
    pub layout: Arc<WebSeedLayout>,
    pub total_pieces: usize,
    pub piece_length: u32,
    pub total_torrent_size: u64,
    pub piece_hashes: Arc<Vec<u8>>,
    pub piece_allocator: Arc<Mutex<PieceAllocator>>,
    pub file_map: Arc<RwLock<Vec<FileRegion>>>,
    pub torrent_state: Arc<Mutex<TorrentState>>,
    pub stats_registry: Arc<StatsRegistry>,
    pub have_tx: broadcast::Sender<u32>,
    pub concurrency_sem: Arc<Semaphore>,
}

enum FetchOutcome {
    Data(Vec<u8>),
    RetryAfter(Duration),
    TransientError(String),
    FatalError(String),
}

impl WebSeedWorker {
    pub async fn start(self) {
        if self.total_pieces == 0 || self.total_torrent_size == 0 {
            return;
        }
        if self.piece_allocator.lock().await.all_complete() {
            return;
        }

        let parsed = match url::Url::parse(&self.base_url) {
            Ok(u) => u,
            Err(e) => {
                warn!("Invalid WebSeed URL '{}': {}", self.base_url, e);
                return;
            }
        };
        let host = match parsed.host_str() {
            Some(h) => h.trim_matches(|c| c == '[' || c == ']').to_string(),
            None => return,
        };
        let default_port = if parsed.scheme() == "https" { 443 } else { 80 };
        let raw_port = parsed.port().unwrap_or(default_port);

        // Resolve IP for UI peer display, falling back to a unique synthetic 0.0.0.x key
        // if DNS hasn't resolved yet or collides across mirrors on the same CDN endpoint.
        let resolved_ip = hydroxy_tracker::resolve_host_ports(&host, raw_port)
            .await
            .ok()
            .and_then(|v| v.first().map(|sa| sa.ip()))
            .unwrap_or_else(|| {
                IpAddr::V4(Ipv4Addr::new(0, 0, 0, ((self.seed_index % 250) + 1) as u8))
            });
        // Offset port by seed_index when > 0 so multiple URLs on the same CDN host have distinct keys
        let display_port = raw_port.wrapping_add(self.seed_index as u16);
        let client_label = format!("WebSeed ({})", host);
        let info_hash_sha1 = OtherInfos::to_sha1(&self.info_hash);

        self.stats_registry.webseed_connected(
            &info_hash_sha1,
            resolved_ip,
            display_port,
            client_label,
        );
        let _cleanup_guard = WebSeedCleanupGuard {
            stats_registry: Arc::clone(&self.stats_registry),
            info_hash_sha1: info_hash_sha1.clone(),
            peer_ip: resolved_ip,
            peer_port: display_port,
        };

        let full_bitfield = PieceAllocator::build_full_bitfield(self.total_pieces);
        let mut excluded_pieces: HashSet<usize> = HashSet::new();
        let mut consecutive_errors: u32 = 0;
        let mut corrupt_pieces: u32 = 0;

        info!(
            "WebSeed worker started: '{}' for torrent {}",
            self.base_url, info_hash_sha1
        );

        loop {
            // Acquire concurrency permit before leasing a piece so idle mirrors don't hoard piece locks
            let permit = match self.concurrency_sem.clone().acquire_owned().await {
                Ok(p) => p,
                Err(_) => break,
            };

            let piece_idx_opt = {
                let mut alloc = self.piece_allocator.lock().await;
                if alloc.all_complete() {
                    debug!(
                        "All pieces complete — stopping WebSeed worker '{}'",
                        self.base_url
                    );
                    break;
                }
                alloc.allocate_piece_excluding(
                    &full_bitfield,
                    &resolved_ip,
                    self.total_pieces,
                    &excluded_pieces,
                )
            };

            let piece_idx = match piece_idx_opt {
                Some(idx) => idx,
                None => {
                    drop(permit);
                    if self.piece_allocator.lock().await.all_complete() {
                        break;
                    }
                    // Remaining pieces are currently in-progress on other workers; wait briefly
                    excluded_pieces.clear();
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
            };

            let requests = match plan_piece_requests(
                &self.base_url,
                &self.layout,
                piece_idx as u32,
                self.piece_length,
                self.total_pieces,
                self.total_torrent_size,
            ) {
                Ok(reqs) => reqs,
                Err(e) => {
                    self.piece_allocator.lock().await.release_piece(piece_idx);
                    warn!(
                        "WebSeed '{}' failed to plan piece {}: {} — retiring WebSeed",
                        self.base_url, piece_idx, e
                    );
                    break;
                }
            };

            let expected_piece_len: usize = requests.iter().map(|r| r.expected_len).sum();
            let mut piece_buf = Vec::with_capacity(expected_piece_len);
            let mut outcome = None;

            for req in &requests {
                match self
                    .fetch_range_slice(req, &info_hash_sha1, resolved_ip, display_port)
                    .await
                {
                    FetchOutcome::Data(slice) => {
                        piece_buf.extend_from_slice(&slice);
                    }
                    other => {
                        outcome = Some(other);
                        break;
                    }
                }
            }

            drop(permit);

            if let Some(err_outcome) = outcome {
                self.piece_allocator.lock().await.release_piece(piece_idx);
                match err_outcome {
                    FetchOutcome::RetryAfter(wait) => {
                        consecutive_errors += 1;
                        self.stats_registry.update_peer_state(
                            &info_hash_sha1,
                            resolved_ip,
                            display_port,
                            PeerConnectionState::Choked,
                        );
                        warn!(
                            "WebSeed '{}' busy (503/429) on piece {} — backing off for {:?}",
                            self.base_url, piece_idx, wait
                        );
                        if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                            warn!(
                                "WebSeed '{}' exceeded max retries — retiring",
                                self.base_url
                            );
                            break;
                        }
                        tokio::time::sleep(wait).await;
                        continue;
                    }
                    FetchOutcome::TransientError(msg) => {
                        consecutive_errors += 1;
                        let backoff_secs = (2u64 << consecutive_errors.min(4)).min(30);
                        warn!(
                            "WebSeed '{}' transient error on piece {} ({}/{}) [{}s backoff]: {}",
                            self.base_url,
                            piece_idx,
                            consecutive_errors,
                            MAX_CONSECUTIVE_ERRORS,
                            backoff_secs,
                            msg
                        );
                        if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                            warn!(
                                "WebSeed '{}' reached {} consecutive failures — retiring",
                                self.base_url, consecutive_errors
                            );
                            break;
                        }
                        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                        continue;
                    }
                    FetchOutcome::FatalError(msg) => {
                        warn!("WebSeed '{}' retired: {}", self.base_url, msg);
                        break;
                    }
                    FetchOutcome::Data(_) => unreachable!(),
                }
            }

            // SHA-1 verify assembled piece on blocking thread pool
            let hash_start = piece_idx * 20;
            if hash_start + 20 > self.piece_hashes.len() {
                self.piece_allocator.lock().await.release_piece(piece_idx);
                break;
            }
            let mut expected_hash = [0u8; 20];
            expected_hash.copy_from_slice(&self.piece_hashes[hash_start..hash_start + 20]);

            let verify_res = tokio::task::spawn_blocking(move || {
                let valid = PieceAssembler::verify_piece(&piece_buf, &expected_hash);
                (valid, piece_buf)
            })
            .await;

            let (is_valid, verified_buf) = match verify_res {
                Ok(pair) => pair,
                Err(e) => {
                    self.piece_allocator.lock().await.release_piece(piece_idx);
                    warn!("WebSeed SHA-1 join error: {}", e);
                    break;
                }
            };

            if !is_valid {
                self.piece_allocator.lock().await.release_piece(piece_idx);
                corrupt_pieces += 1;
                excluded_pieces.insert(piece_idx);
                warn!(
                    "WebSeed '{}' returned corrupt data failing SHA-1 on piece {} ({}/{})",
                    self.base_url, piece_idx, corrupt_pieces, MAX_CORRUPT_PIECES
                );
                if corrupt_pieces >= MAX_CORRUPT_PIECES {
                    warn!(
                        "WebSeed '{}' sent {} corrupt pieces — permanently retiring for session",
                        self.base_url, corrupt_pieces
                    );
                    break;
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }

            // Write verified piece to disk
            let write_res = {
                let map = self.file_map.read().await;
                PieceIO::write_piece_to_disk(&*map, piece_idx as u32, self.piece_length, &verified_buf)
                    .await
            };
            if let Err(e) = write_res {
                self.piece_allocator.lock().await.release_piece(piece_idx);
                warn!(
                    "WebSeed disk write error on piece {} for {}: {}",
                    piece_idx, info_hash_sha1, e
                );
                break;
            }

            consecutive_errors = 0;
            excluded_pieces.remove(&piece_idx);

            self.stats_registry.piece_completed(
                &info_hash_sha1,
                resolved_ip,
                display_port,
                piece_idx,
            );

            {
                let snap = self.stats_registry.snapshot_torrent(&info_hash_sha1);
                let mut state = self.torrent_state.lock().await;
                state.mark_completed_piece(piece_idx);
                if let Some(s) = snap {
                    state.total_downloaded = state.total_downloaded.max(s.downloaded_bytes);
                    state.total_uploaded = state.total_uploaded.max(s.uploaded_bytes);
                }
                let snapshot = state.clone();
                if let Err(e) = TorrentState::save_state(snapshot).await {
                    warn!("State save failed after WebSeed piece {}: {}", piece_idx, e);
                }
            }

            self.piece_allocator.lock().await.mark_complete(piece_idx);
            let _ = self.have_tx.send(piece_idx as u32);
            debug!(
                "WebSeed '{}' completed & verified piece {}",
                self.base_url, piece_idx
            );
        }
    }

    async fn fetch_range_slice(
        &self,
        req: &WebSeedRangeRequest,
        info_hash_sha1: &str,
        peer_ip: IpAddr,
        peer_port: u16,
    ) -> FetchOutcome {
        let range_header = format!("bytes={}-{}", req.range_start, req.range_end_inclusive);
        let send_fut = shared_webseed_client()
            .get(&req.url)
            .header(reqwest::header::RANGE, range_header)
            .send();

        let mut response = match tokio::time::timeout(HEADER_TIMEOUT, send_fut).await {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => return FetchOutcome::TransientError(e.to_string()),
            Err(_) => {
                return FetchOutcome::TransientError(format!(
                    "HTTP header timeout requesting {}",
                    req.url
                ));
            }
        };

        let status = response.status();
        if status == reqwest::StatusCode::SERVICE_UNAVAILABLE
            || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        {
            let retry_secs = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.trim().parse::<u64>().ok())
                .map(|s| s.clamp(5, 300))
                .unwrap_or(15);
            return FetchOutcome::RetryAfter(Duration::from_secs(retry_secs));
        }

        if status == reqwest::StatusCode::PARTIAL_CONTENT {
            // Standard RFC 9110 `206 Partial Content`
        } else if status == reqwest::StatusCode::OK {
            // Only accept `200 OK` if the requested slice covers the entire file from offset 0
            // AND Content-Length does not exceed expected_len (protects against servers ignoring `Range` on multi-GB files)
            let is_whole_file =
                req.range_start == 0 && (req.expected_len as u64) == req.full_file_len;
            let cl_ok = response
                .content_length()
                .map(|cl| cl == req.expected_len as u64)
                .unwrap_or(true);
            if !is_whole_file || !cl_ok {
                return FetchOutcome::FatalError(format!(
                    "HTTP server ignored Range header (returned 200 OK for bytes={}-{} of {} bytes)",
                    req.range_start, req.range_end_inclusive, req.full_file_len
                ));
            }
        } else if status.is_server_error() {
            return FetchOutcome::TransientError(format!("HTTP {}", status));
        } else {
            return FetchOutcome::FatalError(format!("HTTP {} for {}", status, req.url));
        }

        let mut buf = Vec::with_capacity(req.expected_len);
        loop {
            match tokio::time::timeout(CHUNK_READ_TIMEOUT, response.chunk()).await {
                Ok(Ok(Some(chunk))) => {
                    if buf.len() + chunk.len() > req.expected_len {
                        return FetchOutcome::FatalError(format!(
                            "WebSeed response exceeded requested range length ({} > {})",
                            buf.len() + chunk.len(),
                            req.expected_len
                        ));
                    }
                    buf.extend_from_slice(&chunk);
                    self.stats_registry.record_download(
                        info_hash_sha1,
                        peer_ip,
                        peer_port,
                        chunk.len() as u64,
                    );
                }
                Ok(Ok(None)) => break,
                Ok(Err(e)) => return FetchOutcome::TransientError(e.to_string()),
                Err(_) => {
                    return FetchOutcome::TransientError(
                        "HTTP body stream read timed out".to_string(),
                    );
                }
            }
        }

        if buf.len() != req.expected_len {
            return FetchOutcome::TransientError(format!(
                "Truncated HTTP Range body: got {} bytes, expected {}",
                buf.len(),
                req.expected_len
            ));
        }

        let _ = Instant::now();
        FetchOutcome::Data(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha1::{Digest, Sha1};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn test_bep0019_single_file_and_multi_file_url_construction() {
        // 1. Single-file direct URL (no trailing slash) -> unchanged
        let u1 = build_webseed_file_url(
            "https://releases.ubuntu.com/24.04/ubuntu-24.04.iso",
            false,
            "ubuntu-24.04.iso",
            &[],
        )
        .unwrap();
        assert_eq!(u1, "https://releases.ubuntu.com/24.04/ubuntu-24.04.iso");

        // 2. Single-file directory URL (trailing slash) -> appends percent-encoded name + keeps query
        let u2 = build_webseed_file_url(
            "https://cdn.example.com/isos/?token=xyz",
            false,
            "Ubuntu 24.04 #1.iso",
            &[],
        )
        .unwrap();
        assert_eq!(
            u2,
            "https://cdn.example.com/isos/Ubuntu%2024.04%20%231.iso?token=xyz"
        );

        // 3. Multi-file torrent -> appends `<name>/<subdir>/<file>` with percent-encoding
        let u3 = build_webseed_file_url(
            "http://mirror.example.org/pub/",
            true,
            "My Album [FLAC]",
            &["Disc 1".to_string(), "01 - Intro.flac".to_string()],
        )
        .unwrap();
        assert_eq!(
            u3,
            "http://mirror.example.org/pub/My%20Album%20%5BFLAC%5D/Disc%201/01%20-%20Intro.flac"
        );

        // 4. Reject path traversal segments
        assert!(
            build_webseed_file_url(
                "http://mirror.example.org/pub/",
                true,
                " SafeName",
                &["..".to_string(), "passwd".to_string()]
            )
            .is_err()
        );
    }

    #[test]
    fn test_bep0019_plan_piece_spanning_multiple_files_and_empty_file() {
        let layout = WebSeedLayout {
            is_multi_file: true,
            torrent_name: "pack".to_string(),
            files: vec![
                WebSeedFileEntry {
                    rel_segments: vec!["a.bin".to_string()],
                    offset_in_torrent: 0,
                    length: 10,
                },
                WebSeedFileEntry {
                    rel_segments: vec!["empty.txt".to_string()],
                    offset_in_torrent: 10,
                    length: 0,
                },
                WebSeedFileEntry {
                    rel_segments: vec!["sub".to_string(), "b.bin".to_string()],
                    offset_in_torrent: 10,
                    length: 15,
                },
            ],
        };

        // Total = 25 bytes, piece_length = 16 -> piece 0 = [0..16] spans `a.bin` (0..10) + `b.bin` (0..6), skipping `empty.txt`
        let reqs0 = plan_piece_requests("http://127.0.0.1:8080/seed/", &layout, 0, 16, 2, 25)
            .expect("plan piece 0");
        assert_eq!(reqs0.len(), 2);
        assert_eq!(
            reqs0[0],
            WebSeedRangeRequest {
                url: "http://127.0.0.1:8080/seed/pack/a.bin".to_string(),
                range_start: 0,
                range_end_inclusive: 9,
                expected_len: 10,
                full_file_len: 10,
            }
        );
        assert_eq!(
            reqs0[1],
            WebSeedRangeRequest {
                url: "http://127.0.0.1:8080/seed/pack/sub/b.bin".to_string(),
                range_start: 0,
                range_end_inclusive: 5,
                expected_len: 6,
                full_file_len: 15,
            }
        );

        // Piece 1 = [16..25] (fractional last piece of 9 bytes) -> `b.bin` bytes 6..=14
        let reqs1 = plan_piece_requests("http://127.0.0.1:8080/seed/", &layout, 1, 16, 2, 25)
            .expect("plan piece 1");
        assert_eq!(reqs1.len(), 1);
        assert_eq!(
            reqs1[0],
            WebSeedRangeRequest {
                url: "http://127.0.0.1:8080/seed/pack/sub/b.bin".to_string(),
                range_start: 6,
                range_end_inclusive: 14,
                expected_len: 9,
                full_file_len: 15,
            }
        );
    }

    #[tokio::test]
    async fn test_bep0019_end_to_end_http_206_multifile_webseed_download() {
        // Build a 2-file, 2-piece torrent (file1 = 10 bytes, file2 = 14 bytes, piece_length = 16 -> 24 bytes total)
        let file1_data = b"0123456789".to_vec();
        let file2_data = b"ABCDEFGHIJKLMN".to_vec();
        let mut combined = Vec::new();
        combined.extend_from_slice(&file1_data);
        combined.extend_from_slice(&file2_data);

        let piece0_hash: [u8; 20] = Sha1::digest(&combined[0..16]).into();
        let piece1_hash: [u8; 20] = Sha1::digest(&combined[16..24]).into();
        let mut piece_hashes = Vec::with_capacity(40);
        piece_hashes.extend_from_slice(&piece0_hash);
        piece_hashes.extend_from_slice(&piece1_hash);

        // Spawn minimal local HTTP/1.1 Range server on 127.0.0.1:0
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let f1_srv = file1_data.clone();
        let f2_srv = file2_data.clone();

        let srv_handle = tokio::spawn(async move {
            loop {
                let (mut stream, _) = match listener.accept().await {
                    Ok(s) => s,
                    Err(_) => break,
                };
                let f1 = f1_srv.clone();
                let f2 = f2_srv.clone();
                tokio::spawn(async move {
                    let mut req_buf = vec![0u8; 4096];
                    let n = match stream.read(&mut req_buf).await {
                        Ok(n) if n > 0 => n,
                        _ => return,
                    };
                    let req_str = String::from_utf8_lossy(&req_buf[..n]);
                    let first_line = req_str.lines().next().unwrap_or("");
                    let path = first_line.split_whitespace().nth(1).unwrap_or("");

                    let body_src = if path == "/bundle/part1.bin" {
                        &f1
                    } else if path == "/bundle/sub/part2.bin" {
                        &f2
                    } else {
                        let _ = stream
                            .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                            .await;
                        return;
                    };

                    let range_line = req_str
                        .lines()
                        .find(|l| l.to_ascii_lowercase().starts_with("range:"))
                        .unwrap_or("");
                    let range_spec = range_line
                        .split('=')
                        .nth(1)
                        .unwrap_or("0-0")
                        .trim();
                    let (s_str, e_str) = range_spec.split_once('-').unwrap();
                    let start: usize = s_str.parse().unwrap();
                    let end_incl: usize = e_str.parse().unwrap();
                    let slice = &body_src[start..=end_incl];

                    let header = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        start,
                        end_incl,
                        body_src.len(),
                        slice.len()
                    );
                    let _ = stream.write_all(header.as_bytes()).await;
                    let _ = stream.write_all(slice).await;
                });
            }
        });

        let temp_dir = std::env::temp_dir().join(format!(
            "hydroxy_webseed_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let out_f1 = temp_dir.join("bundle/part1.bin");
        let out_f2 = temp_dir.join("bundle/sub/part2.bin");

        let file_map = Arc::new(RwLock::new(vec![
            FileRegion {
                path: out_f1.to_string_lossy().to_string(),
                offset_in_torrent: 0,
                length: 10,
            },
            FileRegion {
                path: out_f2.to_string_lossy().to_string(),
                offset_in_torrent: 10,
                length: 14,
            },
        ]));

        let layout = Arc::new(WebSeedLayout {
            is_multi_file: true,
            torrent_name: "bundle".to_string(),
            files: vec![
                WebSeedFileEntry {
                    rel_segments: vec!["part1.bin".to_string()],
                    offset_in_torrent: 0,
                    length: 10,
                },
                WebSeedFileEntry {
                    rel_segments: vec!["sub".to_string(), "part2.bin".to_string()],
                    offset_in_torrent: 10,
                    length: 14,
                },
            ],
        });

        let info_hash = [0x55u8; 20];
        let info_hash_sha1 = OtherInfos::to_sha1(&info_hash);
        let stats_registry = Arc::new(StatsRegistry::new());
        stats_registry.register_torrent(
            &info_hash_sha1,
            "bundle",
            "default",
            "",
            2,
            16,
            24,
            vec![],
            vec![],
        );
        stats_registry.set_initial_bitfield(&info_hash_sha1, &[0u8], 0, 0);

        let piece_allocator = Arc::new(Mutex::new(PieceAllocator::new(2)));
        let torrent_state = Arc::new(Mutex::new(TorrentState {
            info_hash_v1: info_hash_sha1.clone(),
            version: 1,
            added_time: 0,
            total_downloaded: 0,
            total_uploaded: 0,
            paused: 0,
            sequential: 1,
            pieces: vec![0u8],
            trackers: vec![],
        }));
        let (have_tx, mut have_rx) = broadcast::channel(8);

        let worker = WebSeedWorker {
            seed_index: 0,
            base_url: format!("http://{}/", addr),
            info_hash,
            layout,
            total_pieces: 2,
            piece_length: 16,
            total_torrent_size: 24,
            piece_hashes: Arc::new(piece_hashes),
            piece_allocator: Arc::clone(&piece_allocator),
            file_map: Arc::clone(&file_map),
            torrent_state,
            stats_registry: Arc::clone(&stats_registry),
            have_tx,
            concurrency_sem: Arc::new(Semaphore::new(2)),
        };

        worker.start().await;
        srv_handle.abort();

        // Both pieces verified, written to disk, and announced on `have_tx`
        assert!(piece_allocator.lock().await.all_complete());
        assert_eq!(have_rx.try_recv().unwrap(), 0);
        assert_eq!(have_rx.try_recv().unwrap(), 1);
        assert_eq!(tokio::fs::read(&out_f1).await.unwrap(), file1_data);
        assert_eq!(tokio::fs::read(&out_f2).await.unwrap(), file2_data);

        // WebSeed cleanup guard removed peer entry upon completion, and PEX never saw it
        assert!(stats_registry.active_peer_endpoints(&info_hash_sha1).is_empty());
        let snap = stats_registry.snapshot_torrent(&info_hash_sha1).unwrap();
        assert_eq!(snap.completed_pieces, 2);
        assert_eq!(snap.downloaded_bytes, 24);

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
