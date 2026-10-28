use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use hydroxy_config::HydroxyConfig;
use hydroxy_engine::{
    HydroxyEngine, control::TorrentControlRegistry, remote::server::start_server,
    stats::stats_registry::StatsRegistry,
};
use tracing::{error, info, warn};

const MAX_LOG_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB

struct RotatingFileState {
    path: PathBuf,
    file: std::io::BufWriter<File>,
    bytes_written: u64,
}

#[derive(Clone)]
struct RotatingLogWriter {
    tx: std::sync::mpsc::SyncSender<Vec<u8>>,
}

impl RotatingLogWriter {
    fn open(path: &Path) -> std::io::Result<Self> {
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.len() > 0 {
                let backup = path.with_extension("log.1");
                let _ = std::fs::rename(path, backup);
            }
        }
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;

        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(4096);
        let mut state = RotatingFileState {
            path: path.to_path_buf(),
            file: std::io::BufWriter::with_capacity(64 * 1024, file),
            bytes_written: 0,
        };

        std::thread::Builder::new()
            .name("hydroxy-log-writer".into())
            .spawn(move || {
                while let Ok(buf) = rx.recv() {
                    Self::write_chunk(&mut state, &buf);
                    while let Ok(more) = rx.try_recv() {
                        Self::write_chunk(&mut state, &more);
                    }
                    let _ = state.file.flush();
                }
            })?;

        Ok(Self { tx })
    }

    fn write_chunk(state: &mut RotatingFileState, buf: &[u8]) {
        if state.bytes_written + (buf.len() as u64) > MAX_LOG_FILE_SIZE {
            let _ = state.file.flush();
            let backup = state.path.with_extension("log.1");
            let _ = std::fs::rename(&state.path, backup);
            if let Ok(new_file) = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&state.path)
            {
                state.file = std::io::BufWriter::with_capacity(64 * 1024, new_file);
                state.bytes_written = 0;
            }
        }
        if let Ok(n) = state.file.write(buf) {
            state.bytes_written += n as u64;
        }
    }
}

impl Write for RotatingLogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = self.tx.try_send(buf.to_vec());
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Routes major events (`target: "major"`) to the persistent rotating log file on disk,
/// and broadcasts minor events live without storing them on disk or in memory.
#[derive(Clone)]
struct SplitLogMakeWriter {
    major_file: Option<RotatingLogWriter>,
    foreground: bool,
}

enum LogDestination {
    Major {
        file: Option<RotatingLogWriter>,
        foreground: bool,
    },
    MinorBroadcast(Vec<u8>),
    Discard,
}

impl Write for LogDestination {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            LogDestination::Major { file, foreground } => {
                if let Some(w) = file {
                    let _ = w.write(buf);
                }
                if *foreground {
                    let _ = std::io::stdout().write_all(buf);
                }
                Ok(buf.len())
            }
            LogDestination::MinorBroadcast(vec) => {
                vec.extend_from_slice(buf);
                Ok(buf.len())
            }
            LogDestination::Discard => Ok(buf.len()),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for LogDestination {
    fn drop(&mut self) {
        if let LogDestination::MinorBroadcast(buf) = self {
            if !buf.is_empty() {
                let text = String::from_utf8_lossy(buf);
                let trimmed = text.trim_end_matches(&['\r', '\n'][..]);
                if !trimmed.is_empty() {
                    hydroxy_engine::log_broadcast::broadcast_minor_log(trimmed.to_string());
                }
            }
        }
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SplitLogMakeWriter {
    type Writer = LogDestination;

    fn make_writer(&'a self) -> Self::Writer {
        if hydroxy_engine::log_broadcast::has_subscribers() {
            LogDestination::MinorBroadcast(Vec::with_capacity(128))
        } else {
            LogDestination::Discard
        }
    }

    fn make_writer_for(&'a self, meta: &tracing::Metadata<'_>) -> Self::Writer {
        if meta.target() == "major" {
            LogDestination::Major {
                file: self.major_file.clone(),
                foreground: self.foreground,
            }
        } else if hydroxy_engine::log_broadcast::has_subscribers() {
            LogDestination::MinorBroadcast(Vec::with_capacity(128))
        } else {
            LogDestination::Discard
        }
    }
}

fn check_already_running(config: &HydroxyConfig) -> Option<String> {
    if config.remote_config.rpc_enabled {
        let rpc_addr = (
            config.remote_config.listen_address,
            config.remote_config.listen_port,
        );
        if std::net::TcpListener::bind(rpc_addr).is_err() {
            return Some(format!(
                "Hydroxy Torrent Daemon is already running (RPC port {}:{} is already in use).",
                config.remote_config.listen_address, config.remote_config.listen_port
            ));
        }
    }

    let peer_addr = (
        config.network_config.listen_address,
        config.network_config.listen_port,
    );
    if std::net::TcpListener::bind(peer_addr).is_err() {
        return Some(format!(
            "Hydroxy Torrent Daemon is already running (Peer port {}:{} is already in use).",
            config.network_config.listen_address, config.network_config.listen_port
        ));
    }

    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let is_foreground = args.iter().any(|a| a == "--foreground" || a == "-f");
    let is_child =
        args.iter().any(|a| a == "--daemon-child") || std::env::var("HYDROXY_DAEMON_CHILD").is_ok();

    if !is_child {
        let config = match HydroxyConfig::init() {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("Configuration initialization error: {}", e);
                std::process::exit(1);
            }
        };

        if let Some(msg) = check_already_running(&config) {
            eprintln!("{}", msg);
            std::process::exit(1);
        }

        if !is_foreground {
            // Parent process: display URL, then fork into background
            let log_path = match HydroxyConfig::log_file_path() {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Failed to determine logs path: {}", e);
                    std::process::exit(1);
                }
            };

            println!("Hydroxy Torrent Daemon starting...");
            if let Ok(cfg_path) = HydroxyConfig::config_file_path() {
                println!("Config file: {}", cfg_path.display());
            }
            println!("Peer Listen Port: {}", config.network_config.listen_port);
            println!("RPC Port: {}", config.remote_config.listen_port);
            if config.remote_config.web_ui_enabled {
                let web_ui_url = format!(
                    "http://{}:{}/",
                    config.remote_config.listen_address, config.remote_config.listen_port
                );
                println!("Web UI URL:   {}", web_ui_url);
            }

            println!("Log file:     {}", log_path.display());

            if let Some(ref creds) = config.newly_generated_credentials {
                println!("============================================================");
                println!("🔑 Web UI & RPC Access Credentials Generated:");
                println!("   Username: {}", creds.username);
                println!("   Password: {}", creds.password);
                println!();
                println!("   The password was auto-hashed and saved securely in:");
                if let Ok(cfg_path) = HydroxyConfig::config_file_path() {
                    println!("   {}", cfg_path.display());
                }
                println!("============================================================");
            }

            let exe = match std::env::current_exe() {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("Failed to get executable path: {}", e);
                    std::process::exit(1);
                }
            };

            if let Some(parent) = log_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let mut cmd = std::process::Command::new(exe);
            cmd.arg("--daemon-child")
                .env("HYDROXY_DAEMON_CHILD", "1")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());

            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                cmd.process_group(0);
            }

            match cmd.spawn() {
                Ok(child) => {
                    println!("Daemon started in background (PID: {}).", child.id());
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("Failed to spawn daemon process: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

    // Initialize tracing: major events are saved to disk log file; minor events are broadcast live only when watched
    let major_file = HydroxyConfig::log_file_path()
        .ok()
        .and_then(|p| RotatingLogWriter::open(&p).ok());
    let split_writer = SplitLogMakeWriter {
        major_file,
        foreground: is_foreground,
    };
    tracing_subscriber::fmt()
        .with_writer(split_writer)
        .with_target(false)
        .with_ansi(false)
        .init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .max_blocking_threads(24)
        .thread_keep_alive(std::time::Duration::from_secs(10))
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime");

    rt.block_on(run_daemon());
}

async fn run_daemon() {
    info!(target: "major", "Hydroxy Torrent Initialized (Daemon Mode)...");
    if let Ok(cfg_path) = HydroxyConfig::config_file_path() {
        info!(target: "major", "Config File:   {}", cfg_path.display());
    }

    let stats_registry = Arc::new(StatsRegistry::new());
    let control_registry = Arc::new(TorrentControlRegistry::new());

    let config = match HydroxyConfig::init() {
        Ok(cfg) => cfg,
        Err(e) => {
            error!(target: "major", "Configuration initialization error: {}", e);
            return;
        }
    };

    if let Some(ref creds) = config.newly_generated_credentials {
        info!(target: "major", "============================================================");
        info!(target: "major", "🔑 Web UI & RPC Access Credentials Generated:");
        info!(target: "major", "   Username: {}", creds.username);
        info!(target: "major", "   Password: {}", creds.password);
        info!(target: "major", "============================================================");
    }

    info!(target: "major", "Peer Listen Port: {}", config.network_config.listen_port);
    info!(target: "major", "Watch Dir: {}", config.storage_config.watch_directory);
    info!(target: "major", "Download Dir: {}", config.storage_config.download_directory);

    let _ = std::fs::create_dir_all(&config.storage_config.watch_directory);
    let _ = std::fs::create_dir_all(&config.storage_config.download_directory);

    // Spawn Engine in its own task
    let engine_stats_registry = Arc::clone(&stats_registry);
    let engine_control_registry = Arc::clone(&control_registry);
    tokio::spawn(async move {
        if let Err(e) = HydroxyEngine::start(engine_stats_registry, engine_control_registry).await {
            warn!(target: "major", "Engine error -> {}", e);
        }
    });

    if config.remote_config.rpc_enabled {
        let server_stats = Arc::clone(&stats_registry);
        let server_controls = Arc::clone(&control_registry);
        let remote_cfg = config.remote_config.clone();
        // Run RPC / Web-UI Server on a dedicated isolated OS thread + current_thread runtime
        // so heavy peer traffic or piece hashing on the engine runtime can never delay RPC responses.
        if let Err(e) = std::thread::Builder::new()
            .name("hydroxy-rpc".into())
            .spawn(move || {
                let rpc_rt = tokio::runtime::Builder::new_current_thread()
                    .max_blocking_threads(4)
                    .thread_keep_alive(std::time::Duration::from_secs(10))
                    .enable_all()
                    .build()
                    .expect("Failed to initialize RPC Tokio runtime");

                rpc_rt.block_on(async move {
                    let addr = remote_cfg.listen_address;
                    let port = remote_cfg.listen_port;

                    if let Err(e) = start_server(
                        &addr,
                        &port,
                        remote_cfg,
                        server_stats,
                        server_controls,
                    )
                    .await
                    {
                        error!(target: "major", "RPC server error: {}", e);
                        std::process::exit(1);
                    }
                });
            })
        {
            error!(target: "major", "Failed to spawn RPC server thread: {}", e);
            std::process::exit(1);
        }
    } else {
        info!(target: "major", "RPC is disabled. Web UI automatically disabled.");
    }

    // Keep process alive until Ctrl+C / SIGTERM
    tokio::signal::ctrl_c().await.unwrap();
    info!(target: "major", "Shutdown signal received, closing Hydroxy Torrent...");
    control_registry
        .broadcast_command(hydroxy_engine::control::TorrentCommand::Shutdown)
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
}
