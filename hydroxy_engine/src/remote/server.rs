use std::{net::IpAddr, sync::Arc};

use axum::{Json, Router, response::IntoResponse, routing::{get, post}};
use hydroxy_config::{HydroxyConfig, RemoteConfig};
use serde_json::json;
use tokio::{fs, net::TcpListener};
use tower_http::{
    cors::CorsLayer,
    services::{ServeDir, ServeFile},
};
use tracing::info;

use crate::{
    control::TorrentControlRegistry,
    remote::api::{
        auth_middleware, auth_status_handler, get_logs, get_system_stats, get_torrent_detail,
        get_torrents, login_handler, logout_handler, pause_torrent, reannounce_torrent,
        recheck_torrent, remove_torrent, resume_torrent, set_sequential_download, shutdown_daemon,
        ServerState,
    },
    stats::stats_registry::StatsRegistry,
};

pub async fn start_server(
    listen_address: &IpAddr,
    listen_port: &u16,
    remote_config: RemoteConfig,
    stats_registry: Arc<StatsRegistry>,
    torrent_controls: Arc<TorrentControlRegistry>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let web_ui_enabled = remote_config.web_ui_enabled;
    let server_state = Arc::new(ServerState::new(stats_registry, remote_config, torrent_controls));

    // Public API routes (no auth required)
    let public_api = Router::new()
        .route("/test", get(test_endpoint))
        .route("/auth/login", post(login_handler));

    // Protected API routes (requires valid session token or Basic auth)
    let protected_api = Router::new()
        .route("/auth/logout", post(logout_handler))
        .route("/auth/status", get(auth_status_handler))
        .route("/torrents", get(get_torrents))
        .route("/torrent/:info_hash", get(get_torrent_detail).delete(remove_torrent))
        .route("/torrent/:info_hash/pause", post(pause_torrent))
        .route("/torrent/:info_hash/resume", post(resume_torrent))
        .route("/torrent/:info_hash/recheck", post(recheck_torrent))
        .route("/torrent/:info_hash/reannounce", post(reannounce_torrent))
        .route("/torrent/:info_hash/sequential", post(set_sequential_download))
        .route("/torrent/:info_hash/remove", post(remove_torrent).delete(remove_torrent))
        .route("/logs", get(get_logs))
        .route("/shutdown", post(shutdown_daemon).get(shutdown_daemon))
        .route("/system/stats", get(get_system_stats))
        .route_layer(axum::middleware::from_fn_with_state(
            server_state.clone(),
            auth_middleware,
        ));

    let api_routes = Router::new()
        .merge(public_api)
        .merge(protected_api);

    // tmp
    let cors = CorsLayer::very_permissive();

    // Base Router
    let mut app = Router::new()
        .nest("/api", api_routes)
        .layer(cors)
        .with_state(server_state);

    if web_ui_enabled {
        let web_ui_path = if std::path::Path::new("hydroxy_rc/build/web/index.html").exists() {
            std::path::PathBuf::from("hydroxy_rc/build/web")
        } else {
            let project_data_dir =
                HydroxyConfig::data_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
            let p = project_data_dir.join("web_ui");
            if !p.exists() {
                let _ = fs::create_dir_all(&p).await;
            }
            p
        };

        let index_file = web_ui_path.join("index.html");
        if index_file.exists() {
            let root_service =
                ServeDir::new(&web_ui_path).not_found_service(ServeFile::new(&index_file));

            let fallback_service =
                ServeDir::new(&web_ui_path).not_found_service(ServeFile::new(&index_file));

            // serving web-ui at "/"
            app = app.route_service("/", root_service);
            // serves web-ui as a fallback for invalid path requests..
            app = app.fallback_service(fallback_service);
        }
    }

    let bind_address = format!("{}:{}", listen_address, listen_port);
    let listener = TcpListener::bind(&bind_address).await?;
    info!("Serving Web UI at http://{}/", bind_address);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn test_endpoint() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "message": "Hello, from RPC Server."
    }))
}
