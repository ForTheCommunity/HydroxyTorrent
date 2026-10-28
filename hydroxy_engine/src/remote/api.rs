use std::{
    collections::VecDeque,
    sync::{Arc, RwLock},
};

use axum::{
    Json,
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use hydroxy_config::{HydroxyConfig, RemoteConfig};

use crate::{
    control::{TorrentCommand, TorrentControlRegistry},
    remote::{TorrentApiItem, TorrentDetailApiItem},
    stats::{stats::TorrentStatsSnapshot, stats_registry::StatsRegistry},
};

pub const MAX_ACTIVE_SESSIONS: usize = 5;

pub struct ServerState {
    pub stats: Arc<StatsRegistry>,
    pub remote_config: RemoteConfig,
    pub active_sessions: RwLock<VecDeque<String>>,
    pub torrent_controls: Arc<TorrentControlRegistry>,
    pub persist_sessions: bool,
}

impl ServerState {
    pub fn new(
        stats: Arc<StatsRegistry>,
        remote_config: RemoteConfig,
        torrent_controls: Arc<TorrentControlRegistry>,
    ) -> Self {
        Self {
            stats,
            remote_config,
            active_sessions: RwLock::new(VecDeque::new()),
            torrent_controls,
            persist_sessions: false,
        }
    }

    pub fn new_with_persistence(
        stats: Arc<StatsRegistry>,
        remote_config: RemoteConfig,
        torrent_controls: Arc<TorrentControlRegistry>,
    ) -> Self {
        let initial_sessions = Self::load_persisted_sessions();
        Self {
            stats,
            remote_config,
            active_sessions: RwLock::new(initial_sessions),
            torrent_controls,
            persist_sessions: true,
        }
    }

    fn sessions_file_path() -> Option<std::path::PathBuf> {
        HydroxyConfig::state_dir()
            .ok()
            .map(|dir| dir.join("active_sessions.json"))
    }

    fn load_persisted_sessions() -> VecDeque<String> {
        let Some(path) = Self::sessions_file_path() else {
            return VecDeque::new();
        };
        let Ok(content) = std::fs::read_to_string(path) else {
            return VecDeque::new();
        };
        let Ok(mut list) = serde_json::from_str::<VecDeque<String>>(&content) else {
            return VecDeque::new();
        };
        while list.len() > MAX_ACTIVE_SESSIONS {
            list.pop_front();
        }
        list
    }

    fn save_persisted_sessions(&self, sessions: &VecDeque<String>) {
        if !self.persist_sessions {
            return;
        }
        if let Some(path) = Self::sessions_file_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string(sessions) {
                let _ = std::fs::write(&path, json);
                hydroxy_config::secure_file_permissions(&path);
            }
        }
    }

    pub fn add_session(&self, token: impl Into<String>) {
        let token = token.into();
        let mut sessions = self.active_sessions.write().unwrap();
        sessions.retain(|s| s != &token);
        while sessions.len() >= MAX_ACTIVE_SESSIONS {
            sessions.pop_front();
        }
        sessions.push_back(token);
        self.save_persisted_sessions(&sessions);
    }

    pub fn remove_session(&self, token: &str) {
        let mut sessions = self.active_sessions.write().unwrap();
        sessions.retain(|s| s != token);
        self.save_persisted_sessions(&sessions);
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

#[derive(Debug, serde::Serialize)]
pub struct LoginResponse {
    pub status: String,
    pub token: String,
    pub username: String,
}

pub async fn login_handler(
    State(state): State<Arc<ServerState>>,
    Json(payload): Json<LoginPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if !state
        .remote_config
        .verify_credentials(&payload.username, &payload.password)
    {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Invalid username or password" })),
        ));
    }

    let token = hydroxy_config::auth::generate_session_token();
    state.add_session(token.clone());

    let cookie_val = format!(
        "hydroxy_session={}; Path=/; Max-Age=2592000; SameSite=Lax; HttpOnly",
        token
    );

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(axum::http::header::SET_COOKIE, cookie_val.parse().unwrap());
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        "no-store".parse().unwrap(),
    );

    let body = Json(LoginResponse {
        status: "ok".into(),
        token,
        username: payload.username,
    });

    Ok((headers, body))
}

pub async fn logout_handler(
    State(state): State<Arc<ServerState>>,
    req: Request,
) -> impl IntoResponse {
    for token in extract_all_tokens(&req) {
        state.remove_session(&token);
    }

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::SET_COOKIE,
        "hydroxy_session=; Path=/; Max-Age=0; SameSite=Lax; HttpOnly"
            .parse()
            .unwrap(),
    );

    (
        headers,
        Json(serde_json::json!({
            "status": "ok",
            "message": "Logged out successfully"
        })),
    )
}

pub async fn auth_status_handler(
    State(state): State<Arc<ServerState>>,
    req: Request,
) -> impl IntoResponse {
    let candidates = extract_all_tokens(&req);
    let active = state.active_sessions.read().unwrap();
    let matched_token = candidates
        .iter()
        .find(|t| active.contains(*t))
        .cloned()
        .or_else(|| candidates.into_iter().next())
        .unwrap_or_default();
    drop(active);

    Json(serde_json::json!({
        "authenticated": true,
        "username": state.remote_config.username,
        "token": matched_token
    }))
}

pub fn extract_all_tokens(req: &Request) -> Vec<String> {
    let mut tokens = Vec::new();
    let headers = req.headers();

    // Authorization: Bearer <token>
    if let Some(auth_val) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        if let Some(token) = auth_val.strip_prefix("Bearer ") {
            let trimmed = token.trim();
            if !trimmed.is_empty() {
                tokens.push(trimmed.to_string());
            }
        }
    }

    // X-Hydroxy-Token: <token>
    if let Some(token_val) = headers.get("x-hydroxy-token").and_then(|h| h.to_str().ok()) {
        let trimmed = token_val.trim();
        if !trimmed.is_empty() && !tokens.iter().any(|t| t == trimmed) {
            tokens.push(trimmed.to_string());
        }
    }

    // Cookie: hydroxy_session=<token>
    if let Some(cookie_val) = headers
        .get(axum::http::header::COOKIE)
        .and_then(|h| h.to_str().ok())
    {
        for cookie in cookie_val.split(';') {
            let cookie = cookie.trim();
            if let Some(token) = cookie.strip_prefix("hydroxy_session=") {
                let trimmed = token.trim();
                if !trimmed.is_empty() && !tokens.iter().any(|t| t == trimmed) {
                    tokens.push(trimmed.to_string());
                }
            }
        }
    }

    tokens
}

pub fn extract_token(req: &Request) -> Option<String> {
    extract_all_tokens(req).into_iter().next()
}

pub fn is_authenticated(state: &ServerState, req: &Request) -> bool {
    // Check all candidate token headers / cookies against active sessions
    let candidates = extract_all_tokens(req);
    if !candidates.is_empty() {
        let sessions = state.active_sessions.read().unwrap();
        if candidates.iter().any(|t| sessions.contains(t)) {
            return true;
        }
    }

    // Check HTTP Basic Auth (Authorization: Basic <base64>)
    if let Some(auth_val) = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
    {
        if let Some(basic_b64) = auth_val.strip_prefix("Basic ") {
            use base64::Engine;
            if let Ok(decoded_bytes) =
                base64::engine::general_purpose::STANDARD.decode(basic_b64.trim())
            {
                if let Ok(decoded_str) = String::from_utf8(decoded_bytes) {
                    if let Some((user, pass)) = decoded_str.split_once(':') {
                        if state.remote_config.verify_credentials(user, pass) {
                            return true;
                        }
                    }
                }
            }
        }
    }

    false
}

pub async fn auth_middleware(
    State(state): State<Arc<ServerState>>,
    request: Request,
    next: Next,
) -> Response {
    if is_authenticated(&state, &request) {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Unauthorized" })),
        )
            .into_response()
    }
}

pub async fn get_torrents(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    let snapshots = state.stats.snapshot_all();
    let items: Vec<TorrentApiItem> = snapshots.into_iter().map(snapshot_to_api_item).collect();
    Json(items)
}

pub async fn get_torrent_detail(
    Path(info_hash): Path<String>,
    State(state): State<Arc<ServerState>>,
) -> Result<Json<TorrentDetailApiItem>, StatusCode> {
    if let Some(snap) = state.stats.snapshot_torrent(&info_hash) {
        let peer_list = state.stats.snapshot_peers(&info_hash);
        let trackers = state.stats.snapshot_trackers(&info_hash);
        let files = state.stats.snapshot_files(&info_hash);
        let bitfield = state.stats.snapshot_bitfield(&info_hash);
        use base64::Engine;
        let pieces = base64::engine::general_purpose::STANDARD.encode(&bitfield);
        let is_complete = snap.total_pieces > 0 && snap.completed_pieces >= snap.total_pieces;
        let progress = if snap.total_pieces > 0 {
            (snap.completed_pieces as f64 / snap.total_pieces as f64).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let status = if let Some(ref s) = snap.custom_status {
            s.clone()
        } else if is_complete {
            "seeding".to_string()
        } else {
            "downloading".to_string()
        };
        let downloaded = if is_complete {
            snap.total_bytes
        } else {
            snap.downloaded_bytes.min(snap.total_bytes)
        };

        Ok(Json(TorrentDetailApiItem {
            info_hash: snap.info_hash,
            name: snap.name,
            category: snap.category,
            size: snap.total_bytes,
            downloaded,
            uploaded: snap.uploaded_bytes,
            progress,
            status,
            download_speed: snap.download_speed,
            upload_speed: snap.upload_speed,
            peers: snap.active_peers,
            total_pieces: snap.total_pieces,
            completed_pieces: snap.completed_pieces,
            sequential: snap.sequential,
            pieces,
            peer_list,
            trackers,
            files,
        }))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn pause_torrent(
    Path(info_hash): Path<String>,
    State(state): State<Arc<ServerState>>,
) -> Result<impl IntoResponse, StatusCode> {
    if state
        .torrent_controls
        .send_command(&info_hash, TorrentCommand::Pause)
        .await
    {
        state
            .stats
            .set_status(&info_hash, Some("paused".to_string()));
        Ok(Json(serde_json::json!({
            "status": "ok",
            "message": format!("Torrent {} paused", info_hash)
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn resume_torrent(
    Path(info_hash): Path<String>,
    State(state): State<Arc<ServerState>>,
) -> Result<impl IntoResponse, StatusCode> {
    if state
        .torrent_controls
        .send_command(&info_hash, TorrentCommand::Resume)
        .await
    {
        state.stats.set_status(&info_hash, None);
        Ok(Json(serde_json::json!({
            "status": "ok",
            "message": format!("Torrent {} resumed", info_hash)
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn recheck_torrent(
    Path(info_hash): Path<String>,
    State(state): State<Arc<ServerState>>,
) -> Result<impl IntoResponse, StatusCode> {
    if state
        .torrent_controls
        .send_command(&info_hash, TorrentCommand::Recheck)
        .await
    {
        Ok(Json(serde_json::json!({
            "status": "ok",
            "message": format!("Torrent {} recheck started", info_hash)
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

pub async fn reannounce_torrent(
    Path(info_hash): Path<String>,
    State(state): State<Arc<ServerState>>,
) -> Result<impl IntoResponse, StatusCode> {
    if state
        .torrent_controls
        .send_command(&info_hash, TorrentCommand::Reannounce)
        .await
    {
        Ok(Json(serde_json::json!({
            "status": "ok",
            "message": format!("Torrent {} reannounce started", info_hash)
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct SequentialPayload {
    pub sequential: bool,
}

pub async fn set_sequential_download(
    Path(info_hash): Path<String>,
    State(state): State<Arc<ServerState>>,
    Json(payload): Json<SequentialPayload>,
) -> Result<impl IntoResponse, StatusCode> {
    if state
        .torrent_controls
        .send_command(
            &info_hash,
            TorrentCommand::SetSequential(payload.sequential),
        )
        .await
    {
        state.stats.set_sequential(&info_hash, payload.sequential);
        Ok(Json(serde_json::json!({
            "status": "ok",
            "message": format!("Torrent {} sequential set to {}", info_hash, payload.sequential),
            "sequential": payload.sequential
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct RemoveTorrentPayload {
    #[serde(default)]
    pub delete_files: bool,
}

pub async fn remove_dottorrent_from_dir(dir: &std::path::Path, target_info_hash: &str) {
    if !dir.exists() {
        return;
    }
    let mut read_dir = match tokio::fs::read_dir(dir).await {
        Ok(rd) => rd,
        Err(_) => return,
    };

    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let path = entry.path();
        if path.is_dir() {
            Box::pin(remove_dottorrent_from_dir(&path, target_info_hash)).await;
        } else if path.is_file() {
            let is_torrent = path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("torrent"))
                .unwrap_or(false);

            if is_torrent {
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let name_matches = file_name
                    .to_ascii_lowercase()
                    .contains(&target_info_hash.to_ascii_lowercase());
                let mut content_matches = false;
                if let Ok(data) = tokio::fs::read(&path).await {
                    if let Ok(info) =
                        hydroxy_utils::torrent_info::TorrentInfo::parse_torrent_file(data)
                    {
                        let ih = hydroxy_utils::torrent_info::OtherInfos::to_sha1(
                            &info.other_infos.info_hash,
                        );
                        if ih.eq_ignore_ascii_case(target_info_hash) {
                            content_matches = true;
                        }
                    }
                }
                if name_matches || content_matches {
                    tracing::info!("Removing dottorrent file: {:?}", path);
                    let _ = tokio::fs::remove_file(&path).await;
                }
            }
        }
    }
}

pub async fn remove_state_file(state_dir: &std::path::Path, info_hash: &str) {
    let variations = [
        state_dir.join(format!("{}.hot_state", info_hash)),
        state_dir.join(format!("{}.hot_state", info_hash.to_ascii_lowercase())),
        state_dir.join(format!("{}.hot_state", info_hash.to_ascii_uppercase())),
    ];
    for p in &variations {
        if p.exists() {
            tracing::info!("Removing state file: {:?}", p);
            let _ = tokio::fs::remove_file(p).await;
        }
    }
}

pub async fn remove_downloaded_content(
    download_dir: &std::path::Path,
    category: &str,
    name: &str,
    file_paths: &[String],
) {
    // If individual file paths were recorded, delete them
    for fp in file_paths {
        let p = std::path::Path::new(fp);
        if p.exists() && p.is_file() {
            tracing::info!("Removing downloaded file: {:?}", p);
            let _ = tokio::fs::remove_file(p).await;
        }
    }

    // Check {download_dir}/{category}/{name}
    if !name.is_empty() {
        let root_dir = if category.is_empty() || category == "." {
            download_dir.to_path_buf()
        } else {
            download_dir.join(category)
        };
        let content_path = root_dir.join(name);
        if content_path.exists() {
            if content_path.is_dir() {
                tracing::info!("Removing downloaded directory: {:?}", content_path);
                let _ = tokio::fs::remove_dir_all(&content_path).await;
            } else if content_path.is_file() {
                tracing::info!("Removing downloaded file: {:?}", content_path);
                let _ = tokio::fs::remove_file(&content_path).await;
            }
        }

        // Also check directly under download_dir in case category was not set or changed
        let direct_path = download_dir.join(name);
        if direct_path.exists() {
            if direct_path.is_dir() {
                tracing::info!("Removing downloaded directory: {:?}", direct_path);
                let _ = tokio::fs::remove_dir_all(&direct_path).await;
            } else if direct_path.is_file() {
                tracing::info!("Removing downloaded file: {:?}", direct_path);
                let _ = tokio::fs::remove_file(&direct_path).await;
            }
        }
    }
}

pub async fn remove_torrent(
    Path(info_hash): Path<String>,
    State(state): State<Arc<ServerState>>,
    payload: Option<Json<RemoveTorrentPayload>>,
) -> Result<impl IntoResponse, StatusCode> {
    let delete_files = payload.map(|p| p.delete_files).unwrap_or(false);

    let torrent_snap = state.stats.snapshot_torrent(&info_hash);
    let file_stats = state.stats.snapshot_files(&info_hash);
    let file_paths: Vec<String> = file_stats.into_iter().map(|f| f.path).collect();

    // Stop active task workers if running
    if state
        .torrent_controls
        .send_command(&info_hash, TorrentCommand::Remove)
        .await
    {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // Remove state file from state directory
    if let Ok(state_dir) = HydroxyConfig::state_dir() {
        remove_state_file(&state_dir, &info_hash).await;
    }

    // Remove dottorrent file from watch directory
    if let Ok(config) = HydroxyConfig::init() {
        let watch_dir = std::path::PathBuf::from(&config.storage_config.watch_directory);
        remove_dottorrent_from_dir(&watch_dir, &info_hash).await;

        let fallback_watch = std::path::PathBuf::from("dottorrents");
        if fallback_watch != watch_dir {
            remove_dottorrent_from_dir(&fallback_watch, &info_hash).await;
        }

        // If delete_files requested, remove downloaded content from disk
        if delete_files {
            let download_dir = std::path::PathBuf::from(&config.storage_config.download_directory);
            let category = torrent_snap
                .as_ref()
                .map(|s| s.category.as_str())
                .unwrap_or("");
            let name = torrent_snap.as_ref().map(|s| s.name.as_str()).unwrap_or("");
            remove_downloaded_content(&download_dir, category, name, &file_paths).await;

            let fallback_dl = std::path::PathBuf::from("downloads");
            if fallback_dl != download_dir {
                remove_downloaded_content(&fallback_dl, category, name, &file_paths).await;
            }
        }
    }

    // Unregister from stats and control registry
    let removed_name = torrent_snap
        .as_ref()
        .map(|s| s.name.as_str())
        .unwrap_or("Unknown");
    state.stats.unregister_torrent(&info_hash);
    state.torrent_controls.unregister(&info_hash);

    tracing::info!(
        target: "major",
        "Torrent removed: '{}' ({}) [delete_files: {}]",
        removed_name,
        info_hash,
        delete_files
    );

    Ok(Json(serde_json::json!({
        "status": "ok",
        "message": format!("Torrent {} removed", info_hash),
        "deleted_files": delete_files
    })))
}

fn snapshot_to_api_item(snap: TorrentStatsSnapshot) -> TorrentApiItem {
    let is_complete = snap.total_pieces > 0 && snap.completed_pieces >= snap.total_pieces;
    let progress = if snap.total_pieces > 0 {
        (snap.completed_pieces as f64 / snap.total_pieces as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let status = if let Some(ref s) = snap.custom_status {
        s.clone()
    } else if is_complete {
        "seeding".to_string()
    } else {
        "downloading".to_string()
    };
    let downloaded = if is_complete {
        snap.total_bytes
    } else {
        snap.downloaded_bytes.min(snap.total_bytes)
    };

    TorrentApiItem {
        info_hash: snap.info_hash,
        name: snap.name,
        category: snap.category,
        size: snap.total_bytes,
        downloaded,
        uploaded: snap.uploaded_bytes,
        progress,
        status,
        download_speed: snap.download_speed,
        upload_speed: snap.upload_speed,
        peers: snap.active_peers,
        total_pieces: snap.total_pieces,
        completed_pieces: snap.completed_pieces,
        sequential: snap.sequential,
    }
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct MinorLogsQuery {
    #[serde(default)]
    pub sub_id: String,
}

pub async fn get_logs() -> Result<Json<serde_json::Value>, StatusCode> {
    let log_path = match HydroxyConfig::log_file_path() {
        Ok(p) => p,
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    if !log_path.exists() {
        return Ok(Json(serde_json::json!({
            "logs": "No logs recorded yet."
        })));
    }

    match read_log_tail(&log_path, 128 * 1024, 500).await {
        Ok(trimmed) => Ok(Json(serde_json::json!({
            "logs": trimmed
        }))),
        Err(e) => Ok(Json(serde_json::json!({
            "logs": format!("Error reading log file: {}", e)
        }))),
    }
}

pub async fn get_minor_logs(
    axum::extract::Query(query): axum::extract::Query<MinorLogsQuery>,
) -> Json<serde_json::Value> {
    let lines = crate::log_broadcast::poll_minor_logs(&query.sub_id);
    Json(serde_json::json!({
        "logs": lines
    }))
}

pub async fn unsubscribe_minor_logs(
    axum::extract::Query(query): axum::extract::Query<MinorLogsQuery>,
) -> Json<serde_json::Value> {
    crate::log_broadcast::unsubscribe_minor_logs(&query.sub_id);
    Json(serde_json::json!({
        "status": "ok"
    }))
}

pub async fn read_log_tail(
    log_path: &std::path::Path,
    max_bytes: u64,
    max_lines: usize,
) -> std::io::Result<String> {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    let mut file = tokio::fs::File::open(log_path).await?;
    let file_len = file.metadata().await?.len();
    let seek_offset = file_len.saturating_sub(max_bytes);
    if seek_offset > 0 {
        file.seek(std::io::SeekFrom::Start(seek_offset)).await?;
    }

    let to_read = (file_len - seek_offset) as usize;
    let mut buf = Vec::with_capacity(to_read);
    file.take(max_bytes).read_to_end(&mut buf).await?;

    let content = String::from_utf8_lossy(&buf);
    let mut lines: Vec<&str> = content.lines().collect();
    // If we seeked into the middle of the file, drop the first potentially partial line
    if seek_offset > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    let start = lines.len().saturating_sub(max_lines);
    Ok(lines[start..].join("\n"))
}

pub async fn shutdown_daemon(State(state): State<Arc<ServerState>>) -> impl IntoResponse {
    let controls = Arc::clone(&state.torrent_controls);
    tokio::spawn(async move {
        controls.broadcast_command(TorrentCommand::Shutdown).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        std::process::exit(0);
    });

    Json(serde_json::json!({
        "status": "ok",
        "message": "Hydroxy Torrent daemon is shutting down..."
    }))
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SystemStats {
    pub system_total_bytes: u64,
    pub system_used_bytes: u64,
    pub process_rss_bytes: u64,
}

pub async fn get_system_stats() -> impl IntoResponse {
    let mut total_bytes = 0u64;
    let mut available_bytes = 0u64;
    let mut process_rss_bytes = 0u64;

    #[cfg(target_os = "linux")]
    {
        if let Ok(meminfo) = tokio::fs::read_to_string("/proc/meminfo").await {
            for line in meminfo.lines() {
                if let Some(rest) = line.strip_prefix("MemTotal:") {
                    if let Some(val_str) = rest.trim().split_whitespace().next() {
                        if let Ok(kb) = val_str.parse::<u64>() {
                            total_bytes = kb * 1024;
                        }
                    }
                } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
                    if let Some(val_str) = rest.trim().split_whitespace().next() {
                        if let Ok(kb) = val_str.parse::<u64>() {
                            available_bytes = kb * 1024;
                        }
                    }
                }
            }
        }

        if let Ok(status) = tokio::fs::read_to_string("/proc/self/status").await {
            for line in status.lines() {
                if let Some(rest) = line.strip_prefix("VmRSS:") {
                    if let Some(val_str) = rest.trim().split_whitespace().next() {
                        if let Ok(kb) = val_str.parse::<u64>() {
                            process_rss_bytes = kb * 1024;
                        }
                    }
                    break;
                }
            }
        }
    }

    let used_bytes = total_bytes.saturating_sub(available_bytes);

    Json(SystemStats {
        system_total_bytes: total_bytes,
        system_used_bytes: used_bytes,
        process_rss_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hydroxy_config::RemoteConfig;
    use std::net::Ipv4Addr;

    fn create_test_state() -> ServerState {
        let remote_config = RemoteConfig {
            rpc_enabled: true,
            web_ui_enabled: true,
            listen_address: std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            listen_port: 10000,
            username: "admin".to_string(),
            password: hydroxy_config::auth::hash_password("secret123"),
        };
        ServerState::new(
            Arc::new(StatsRegistry::new()),
            remote_config,
            Arc::new(TorrentControlRegistry::new()),
        )
    }

    #[test]
    fn test_unauthenticated_request() {
        let state = create_test_state();
        let req = Request::builder()
            .uri("/api/torrents")
            .body(axum::body::Body::empty())
            .unwrap();

        assert!(!is_authenticated(&state, &req));
    }

    #[test]
    fn test_bearer_token_authentication() {
        let state = create_test_state();
        let token = "valid_test_token_12345";
        state.add_session(token);

        let req = Request::builder()
            .uri("/api/torrents")
            .header("Authorization", format!("Bearer {}", token))
            .body(axum::body::Body::empty())
            .unwrap();

        assert!(is_authenticated(&state, &req));

        // Invalid token
        let req_invalid = Request::builder()
            .uri("/api/torrents")
            .header("Authorization", "Bearer invalid_token")
            .body(axum::body::Body::empty())
            .unwrap();

        assert!(!is_authenticated(&state, &req_invalid));
    }

    #[test]
    fn test_cookie_authentication() {
        let state = create_test_state();
        let token = "cookie_token_abc";
        state.add_session(token);

        let req = Request::builder()
            .uri("/api/torrents")
            .header(
                "Cookie",
                format!("other=123; hydroxy_session={}; test=1", token),
            )
            .body(axum::body::Body::empty())
            .unwrap();

        assert!(is_authenticated(&state, &req));
    }

    #[tokio::test]
    async fn test_max_active_sessions_cap_of_five() {
        let state = Arc::new(create_test_state());

        let mut tokens = Vec::new();
        for _ in 1..=6 {
            let payload = LoginPayload {
                username: "admin".into(),
                password: "secret123".into(),
            };
            let resp = login_handler(axum::extract::State(state.clone()), Json(payload))
                .await
                .expect("Login failed")
                .into_response();

            let set_cookie = resp
                .headers()
                .get(axum::http::header::SET_COOKIE)
                .unwrap()
                .to_str()
                .unwrap();
            assert!(set_cookie.contains("Max-Age=2592000"));

            let token = state
                .active_sessions
                .read()
                .unwrap()
                .back()
                .cloned()
                .unwrap();
            tokens.push(token);
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }

        // Only at most 5 sessions should be active
        assert_eq!(state.active_sessions.read().unwrap().len(), 5);

        // First session must be evicted
        let req_first = Request::builder()
            .uri("/api/torrents")
            .header("Authorization", format!("Bearer {}", tokens[0]))
            .body(axum::body::Body::empty())
            .unwrap();
        assert!(
            !is_authenticated(&state, &req_first),
            "First session should have been evicted"
        );

        // The remaining 5 sessions (tokens 1..=5) should be valid
        for (i, token) in tokens[1..].iter().enumerate() {
            let req = Request::builder()
                .uri("/api/torrents")
                .header("Authorization", format!("Bearer {}", token))
                .body(axum::body::Body::empty())
                .unwrap();
            assert!(
                is_authenticated(&state, &req),
                "Session {} should be active",
                i + 2
            );
        }
    }

    #[test]
    fn test_basic_auth_authentication() {
        let state = create_test_state();
        use base64::Engine;
        let creds_valid = base64::engine::general_purpose::STANDARD.encode("admin:secret123");
        let req = Request::builder()
            .uri("/api/torrents")
            .header("Authorization", format!("Basic {}", creds_valid))
            .body(axum::body::Body::empty())
            .unwrap();

        assert!(is_authenticated(&state, &req));

        let creds_wrong = base64::engine::general_purpose::STANDARD.encode("admin:wrongpass");
        let req_wrong = Request::builder()
            .uri("/api/torrents")
            .header("Authorization", format!("Basic {}", creds_wrong))
            .body(axum::body::Body::empty())
            .unwrap();

        assert!(!is_authenticated(&state, &req_wrong));
    }

    #[tokio::test]
    async fn test_remove_state_file() {
        let temp_dir = std::env::temp_dir().join(format!(
            "hydroxy_test_state_{}",
            std::time::SystemTime::now().elapsed().unwrap().as_nanos()
        ));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let info_hash = "0123456789abcdef0123456789abcdef01234567";
        let state_file = temp_dir.join(format!("{}.hot_state", info_hash));
        tokio::fs::write(&state_file, b"test state data")
            .await
            .unwrap();
        assert!(state_file.exists());

        remove_state_file(&temp_dir, info_hash).await;
        assert!(!state_file.exists());

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_remove_dottorrent_from_dir_by_name() {
        let temp_dir = std::env::temp_dir().join(format!(
            "hydroxy_test_watch_{}",
            std::time::SystemTime::now().elapsed().unwrap().as_nanos()
        ));
        let sub_dir = temp_dir.join("anime").join("movies");
        tokio::fs::create_dir_all(&sub_dir).await.unwrap();

        let info_hash = "abcdef1234567890abcdef1234567890abcdef12";
        let matching_torrent = sub_dir.join(format!("{}.torrent", info_hash));
        let non_matching_torrent = sub_dir.join("other.torrent");

        tokio::fs::write(&matching_torrent, b"dummy content")
            .await
            .unwrap();
        tokio::fs::write(&non_matching_torrent, b"other content")
            .await
            .unwrap();

        assert!(matching_torrent.exists());
        assert!(non_matching_torrent.exists());

        remove_dottorrent_from_dir(&temp_dir, info_hash).await;

        assert!(!matching_torrent.exists());
        assert!(non_matching_torrent.exists());

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_remove_downloaded_content() {
        let temp_dir = std::env::temp_dir().join(format!(
            "hydroxy_test_dl_{}",
            std::time::SystemTime::now().elapsed().unwrap().as_nanos()
        ));
        let category_dir = temp_dir.join("category_a");
        let torrent_folder = category_dir.join("MyTorrentFolder");
        tokio::fs::create_dir_all(&torrent_folder).await.unwrap();

        let file1 = torrent_folder.join("file1.mp4");
        tokio::fs::write(&file1, b"video data").await.unwrap();
        assert!(file1.exists());

        // Call remove_downloaded_content
        remove_downloaded_content(
            &temp_dir,
            "category_a",
            "MyTorrentFolder",
            &[file1.to_string_lossy().to_string()],
        )
        .await;

        assert!(!torrent_folder.exists());
        assert!(!file1.exists());

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_remove_torrent_handler() {
        let state = Arc::new(create_test_state());
        let info_hash = "1122334455667788990011223344556677889900".to_string();

        state.stats.register_torrent(
            &info_hash,
            "TestTorrent",
            "General",
            10,
            1024,
            10240,
            vec![],
            vec![],
        );

        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::channel(4);
        state.torrent_controls.register(&info_hash, cmd_tx);

        assert!(state.stats.snapshot_torrent(&info_hash).is_some());

        let response = remove_torrent(
            Path(info_hash.clone()),
            State(state.clone()),
            Some(Json(RemoveTorrentPayload {
                delete_files: false,
            })),
        )
        .await;

        assert!(response.is_ok());

        // Control command received
        let cmd = cmd_rx.try_recv().unwrap();
        assert_eq!(cmd, TorrentCommand::Remove);

        // Stats unregistered
        assert!(state.stats.snapshot_torrent(&info_hash).is_none());
    }

    #[tokio::test]
    async fn test_read_log_tail_bounded() {
        let temp_file = std::env::temp_dir().join(format!(
            "hydroxy_test_log_{}.log",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut content = String::new();
        for i in 1..=1000 {
            content.push_str(&format!("2026-10-27 INFO line number {}\n", i));
        }
        tokio::fs::write(&temp_file, content.as_bytes())
            .await
            .unwrap();

        // Read only last 4 KB and at most 50 lines
        let tail = read_log_tail(&temp_file, 4096, 50).await.unwrap();
        let lines: Vec<&str> = tail.lines().collect();
        assert_eq!(lines.len(), 50);
        assert!(lines.last().unwrap().ends_with("line number 1000"));
        assert!(lines.first().unwrap().ends_with("line number 951"));

        let _ = tokio::fs::remove_file(&temp_file).await;
    }
}
