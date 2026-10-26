use std::sync::Arc;

use hydroxy_config::HydroxyConfig;
use hydroxy_engine::{
    HydroxyEngine, control::TorrentControlRegistry, remote::server::start_server,
    stats::stats_registry::StatsRegistry,
};
use tracing::{error, info, warn};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let is_foreground = args.iter().any(|a| a == "--foreground" || a == "-f");
    let is_child =
        args.iter().any(|a| a == "--daemon-child") || std::env::var("HYDROXY_DAEMON_CHILD").is_ok();

    if !is_foreground && !is_child {
        // Parent process: read config to display URL, then fork into background
        let config = match HydroxyConfig::init() {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("Configuration initialization error: {}", e);
                std::process::exit(1);
            }
        };

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

        let log_file_out = match std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)
        {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to open log file {}: {}", log_path.display(), e);
                std::process::exit(1);
            }
        };

        let log_file_err = match log_file_out.try_clone() {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to clone log file handle: {}", e);
                std::process::exit(1);
            }
        };

        let mut cmd = std::process::Command::new(exe);
        cmd.arg("--daemon-child")
            .env("HYDROXY_DAEMON_CHILD", "1")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::from(log_file_out))
            .stderr(std::process::Stdio::from(log_file_err));

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

    // Initialize tracing: write to log file in daemon mode, stdout + log file in foreground
    if is_foreground {
        if let Ok(log_path) = HydroxyConfig::log_file_path() {
            if let Ok(file) = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&log_path)
            {
                use tracing_subscriber::fmt::writer::MakeWriterExt;
                tracing_subscriber::fmt()
                    .with_writer(std::io::stdout.and(file))
                    .init();
            } else {
                tracing_subscriber::fmt::init();
            }
        } else {
            tracing_subscriber::fmt::init();
        }
    } else if let Ok(log_path) = HydroxyConfig::log_file_path() {
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&log_path)
        {
            tracing_subscriber::fmt()
                .with_writer(file)
                .with_ansi(false)
                .init();
        } else {
            tracing_subscriber::fmt::init();
        }
    } else {
        tracing_subscriber::fmt::init();
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to initialize Tokio runtime");

    rt.block_on(run_daemon());
}

async fn run_daemon() {
    info!("Hydroxy Torrent Initialized (Daemon Mode)...");
    if let Ok(cfg_path) = HydroxyConfig::config_file_path() {
        info!("Config File:   {}", cfg_path.display());
    }

    let stats_registry = Arc::new(StatsRegistry::new());
    let control_registry = Arc::new(TorrentControlRegistry::new());

    let config = match HydroxyConfig::init() {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Configuration initialization error: {}", e);
            return;
        }
    };

    if let Some(ref creds) = config.newly_generated_credentials {
        info!("============================================================");
        info!("🔑 Web UI & RPC Access Credentials Generated:");
        info!("   Username: {}", creds.username);
        info!("   Password: {}", creds.password);
        info!("============================================================");
    }

    info!("Watch Dir: {}", config.storage_config.watch_directory);
    info!("Download Dir: {}", config.storage_config.download_directory);

    let _ = std::fs::create_dir_all(&config.storage_config.watch_directory);
    let _ = std::fs::create_dir_all(&config.storage_config.download_directory);

    // Spawn Engine in its own task
    let engine_stats_registry = Arc::clone(&stats_registry);
    let engine_control_registry = Arc::clone(&control_registry);
    tokio::spawn(async move {
        if let Err(e) = HydroxyEngine::start(engine_stats_registry, engine_control_registry).await {
            warn!("Engine error -> {}", e);
        }
    });

    if config.remote_config.rpc_enabled {
        let server_stats = Arc::clone(&stats_registry);
        let server_controls = Arc::clone(&control_registry);
        // Spawn RPC Server
        tokio::spawn(async move {
            let addr = config.remote_config.listen_address;
            let port = config.remote_config.listen_port;

            if let Err(e) = start_server(
                &addr,
                &port,
                config.remote_config.clone(),
                server_stats,
                server_controls,
            )
            .await
            {
                error!("RPC server error: {}", e);
            }
        });
    } else {
        info!("RPC is disabled. Web UI automatically disabled.");
    }

    // Keep process alive until Ctrl+C / SIGTERM
    tokio::signal::ctrl_c().await.unwrap();
    info!("Shutdown signal received, closing Hydroxy Torrent...");
}
