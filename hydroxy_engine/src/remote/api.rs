use std::{
    collections::HashSet,
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

pub struct ServerState {
    pub stats: Arc<StatsRegistry>,
    pub remote_config: RemoteConfig,
    pub active_sessions: RwLock<HashSet<String>>,
    pub torrent_controls: Arc<TorrentControlRegistry>,
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
            active_sessions: RwLock::new(HashSet::new()),
            torrent_controls,
        }
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
    if !state.remote_config.verify_credentials(&payload.username, &payload.password) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "Invalid username or password" })),
        ));
    }

    let token = hydroxy_config::auth::generate_session_token();
    {
        let mut sessions = state.active_sessions.write().unwrap();
        sessions.insert(token.clone());
    }

    let cookie_val = format!("hydroxy_session={}; Path=/; SameSite=Lax; HttpOnly", token);

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::SET_COOKIE,
        cookie_val.parse().unwrap(),
    );
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
    if let Some(token) = extract_token(&req) {
        let mut sessions = state.active_sessions.write().unwrap();
        sessions.remove(&token);
    }

    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::SET_COOKIE,
        "hydroxy_session=; Path=/; Max-Age=0; SameSite=Lax; HttpOnly".parse().unwrap(),
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
) -> impl IntoResponse {
    Json(serde_json::json!({
        "authenticated": true,
        "username": state.remote_config.username
    }))
}

pub fn extract_token(req: &Request) -> Option<String> {
    let headers = req.headers();

    // Authorization: Bearer <token>
    if let Some(auth_val) = headers.get(axum::http::header::AUTHORIZATION).and_then(|h| h.to_str().ok()) {
        if let Some(token) = auth_val.strip_prefix("Bearer ") {
            return Some(token.trim().to_string());
        }
    }

    // X-Hydroxy-Token: <token>
    if let Some(token_val) = headers.get("x-hydroxy-token").and_then(|h| h.to_str().ok()) {
        return Some(token_val.trim().to_string());
    }

    // Cookie: hydroxy_session=<token>
    if let Some(cookie_val) = headers.get(axum::http::header::COOKIE).and_then(|h| h.to_str().ok()) {
        for cookie in cookie_val.split(';') {
            let cookie = cookie.trim();
            if let Some(token) = cookie.strip_prefix("hydroxy_session=") {
                return Some(token.trim().to_string());
            }
        }
    }

    None
}

pub fn is_authenticated(state: &ServerState, req: &Request) -> bool {
    // Check token sessions
    if let Some(token) = extract_token(req) {
        if state.active_sessions.read().unwrap().contains(&token) {
            return true;
        }
    }

    // Check HTTP Basic Auth (Authorization: Basic <base64>)
    if let Some(auth_val) = req.headers().get(axum::http::header::AUTHORIZATION).and_then(|h| h.to_str().ok()) {
        if let Some(basic_b64) = auth_val.strip_prefix("Basic ") {
            use base64::Engine;
            if let Ok(decoded_bytes) = base64::engine::general_purpose::STANDARD.decode(basic_b64.trim()) {
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
        state.stats.set_status(&info_hash, Some("paused".to_string()));
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
        .send_command(&info_hash, TorrentCommand::SetSequential(payload.sequential))
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
            let name = torrent_snap
                .as_ref()
                .map(|s| s.name.as_str())
                .unwrap_or("");
            remove_downloaded_content(&download_dir, category, name, &file_paths).await;

            let fallback_dl = std::path::PathBuf::from("downloads");
            if fallback_dl != download_dir {
                remove_downloaded_content(&fallback_dl, category, name, &file_paths).await;
            }
        }
    }

    // Unregister from stats and control registry
    state.stats.unregister_torrent(&info_hash);
    state.torrent_controls.unregister(&info_hash);

    tracing::info!(
        "Torrent {} removed successfully (delete_files: {})",
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

    match tokio::fs::read_to_string(&log_path).await {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            let start = if lines.len() > 500 { lines.len() - 500 } else { 0 };
            let trimmed = lines[start..].join("\n");
            Ok(Json(serde_json::json!({
                "logs": trimmed
            })))
        }
        Err(e) => Ok(Json(serde_json::json!({
            "logs": format!("Error reading log file: {}", e)
        }))),
    }
}

pub async fn shutdown_daemon() -> impl IntoResponse {
    tokio::spawn(async {
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
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
        if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
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

        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
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
        state.active_sessions.write().unwrap().insert(token.to_string());

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
        state.active_sessions.write().unwrap().insert(token.to_string());

        let req = Request::builder()
            .uri("/api/torrents")
            .header("Cookie", format!("other=123; hydroxy_session={}; test=1", token))
            .body(axum::body::Body::empty())
            .unwrap();

        assert!(is_authenticated(&state, &req));
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
        let temp_dir = std::env::temp_dir().join(format!("hydroxy_test_state_{}", std::time::SystemTime::now().elapsed().unwrap().as_nanos()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let info_hash = "0123456789abcdef0123456789abcdef01234567";
        let state_file = temp_dir.join(format!("{}.hot_state", info_hash));
        tokio::fs::write(&state_file, b"test state data").await.unwrap();
        assert!(state_file.exists());

        remove_state_file(&temp_dir, info_hash).await;
        assert!(!state_file.exists());

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_remove_dottorrent_from_dir_by_name() {
        let temp_dir = std::env::temp_dir().join(format!("hydroxy_test_watch_{}", std::time::SystemTime::now().elapsed().unwrap().as_nanos()));
        let sub_dir = temp_dir.join("anime").join("movies");
        tokio::fs::create_dir_all(&sub_dir).await.unwrap();

        let info_hash = "abcdef1234567890abcdef1234567890abcdef12";
        let matching_torrent = sub_dir.join(format!("{}.torrent", info_hash));
        let non_matching_torrent = sub_dir.join("other.torrent");

        tokio::fs::write(&matching_torrent, b"dummy content").await.unwrap();
        tokio::fs::write(&non_matching_torrent, b"other content").await.unwrap();

        assert!(matching_torrent.exists());
        assert!(non_matching_torrent.exists());

        remove_dottorrent_from_dir(&temp_dir, info_hash).await;

        assert!(!matching_torrent.exists());
        assert!(non_matching_torrent.exists());

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }

    #[tokio::test]
    async fn test_remove_downloaded_content() {
        let temp_dir = std::env::temp_dir().join(format!("hydroxy_test_dl_{}", std::time::SystemTime::now().elapsed().unwrap().as_nanos()));
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
        ).await;

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
            Some(Json(RemoveTorrentPayload { delete_files: false })),
        ).await;

        assert!(response.is_ok());

        // Control command received
        let cmd = cmd_rx.try_recv().unwrap();
        assert_eq!(cmd, TorrentCommand::Remove);

        // Stats unregistered
        assert!(state.stats.snapshot_torrent(&info_hash).is_none());
    }
}
