use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs::{self};
use std::net::IpAddr;
use std::path::PathBuf;
use tracing::info;

use crate::errors::ConfigError;

pub mod auth;
pub mod errors;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HydroxyConfig {
    pub network_config: NetworkConfig,
    #[serde(default, alias = "Limitations", alias = "limitations")]
    pub session_config: SessionConfig,
    pub storage_config: StorageConfig,
    pub remote_config: RemoteConfig,
    #[serde(skip)]
    pub newly_generated_credentials: Option<GeneratedCredentials>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkConfig {
    pub listen_address: IpAddr,
    pub listen_port: u16,
}

fn default_max_active_downloads() -> usize {
    15
}

fn default_max_active_uploads() -> usize {
    5
}

fn default_watch_directory_max_depth() -> usize {
    3
}

fn default_zero_u64() -> u64 {
    0
}

fn default_max_peers_per_torrent() -> usize {
    50
}

fn default_max_total_peers() -> usize {
    300
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SessionConfig {
    #[serde(default = "default_max_active_downloads")]
    pub max_active_downloads: usize,
    #[serde(default = "default_max_active_uploads")]
    pub max_active_uploads: usize,
    #[serde(default = "default_zero_u64")]
    pub max_download_speed_kbps: u64,
    #[serde(default = "default_zero_u64")]
    pub max_upload_speed_kbps: u64,
    #[serde(default = "default_max_peers_per_torrent")]
    pub max_peers_per_torrent: usize,
    #[serde(default = "default_max_total_peers")]
    pub max_total_peers: usize,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_active_downloads: default_max_active_downloads(),
            max_active_uploads: default_max_active_uploads(),
            max_download_speed_kbps: default_zero_u64(),
            max_upload_speed_kbps: default_zero_u64(),
            max_peers_per_torrent: default_max_peers_per_torrent(),
            max_total_peers: default_max_total_peers(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    pub watch_directory: String,
    pub download_directory: String,
    #[serde(default)]
    pub reflection: bool,
    #[serde(default = "default_watch_directory_max_depth")]
    pub watch_directory_max_depth: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_active_downloads: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RemoteConfig {
    pub rpc_enabled: bool,
    pub web_ui_enabled: bool,
    pub listen_address: IpAddr,
    pub listen_port: u16,
    pub username: String,
    pub password: String,
}

impl RemoteConfig {
    pub fn verify_credentials(&self, username: &str, password: &str) -> bool {
        if self.username != username {
            return false;
        }
        crate::auth::verify_password(password, &self.password)
    }
}

#[cfg(unix)]
pub fn secure_file_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
pub fn secure_file_permissions(_path: &std::path::Path) {}

impl HydroxyConfig {
    fn default() -> &'static str {
        let default_config_str = include_str!("../hydroxy_config.toml");
        default_config_str
    }

    pub fn init() -> Result<Self, ConfigError> {
        let config_dir = Self::config_dir()?;

        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).map_err(|e| {
                ConfigError::Error(format!("Failed to create config directory: {}", e))
            })?;
        }

        let config_file_name = "hydroxy_config.toml";
        let config_file = config_dir.join(config_file_name);

        if !config_file.exists() {
            // First start: generate default username and a secure random password (qBittorrent style)
            let default_username = "admin".to_string();
            let plain_password = auth::generate_random_password(16);
            let hashed_password = auth::hash_password(&plain_password);

            let initial_toml = auth::update_credentials_in_toml_content(
                Self::default(),
                &default_username,
                &hashed_password,
            );

            fs::write(&config_file, &initial_toml)
                .map_err(|e| ConfigError::Error(e.to_string()))?;
            secure_file_permissions(&config_file);

            let mut config: HydroxyConfig =
                toml::from_str(&initial_toml).map_err(|e| ConfigError::Error(e.to_string()))?;
            config.newly_generated_credentials = Some(GeneratedCredentials {
                username: default_username,
                password: plain_password,
            });

            return Ok(config);
        }

        let config_str =
            fs::read_to_string(&config_file).map_err(|e| ConfigError::Error(e.to_string()))?;

        let mut config: HydroxyConfig =
            toml::from_str(&config_str).map_err(|e| ConfigError::Error(e.to_string()))?;

        let mut needs_file_update = false;
        let mut newly_generated = None;

        // Ensure username is not empty
        if config.remote_config.username.trim().is_empty() {
            config.remote_config.username = "admin".to_string();
            needs_file_update = true;
        }

        let current_pwd = config.remote_config.password.trim();
        if current_pwd.is_empty() {
            // No password was set: generate a random password (qBittorrent style)
            let plain_password = auth::generate_random_password(16);
            let hashed_password = auth::hash_password(&plain_password);

            config.remote_config.password = hashed_password;
            newly_generated = Some(GeneratedCredentials {
                username: config.remote_config.username.clone(),
                password: plain_password,
            });
            needs_file_update = true;
        } else if !auth::is_hashed(current_pwd) {
            // Plain text password provided in config: auto-hash it (Transmission style)
            let hashed_password = auth::hash_password(current_pwd);
            info!("Auto-hashing plain-text password from config file...");
            config.remote_config.password = hashed_password;
            needs_file_update = true;
        }

        if needs_file_update {
            let updated_toml = auth::update_credentials_in_toml_content(
                &config_str,
                &config.remote_config.username,
                &config.remote_config.password,
            );
            let _ = fs::write(&config_file, updated_toml);
            secure_file_permissions(&config_file);
        }

        config.newly_generated_credentials = newly_generated;
        Ok(config)
    }

    /// Returns the effective max active downloads limit, respecting legacy `storage_config.max_active_downloads`
    /// if set while prioritizing `session_config.max_active_downloads`.
    pub fn effective_max_active_downloads(&self) -> usize {
        if let Some(storage_limit) = self.storage_config.max_active_downloads {
            if self.session_config.max_active_downloads == default_max_active_downloads()
                && storage_limit != default_max_active_downloads()
            {
                return storage_limit;
            }
        }
        self.session_config.max_active_downloads
    }

    pub fn config_file_path() -> Result<PathBuf, ConfigError> {
        let config_dir = Self::config_dir()?;
        Ok(config_dir.join("hydroxy_config.toml"))
    }

    pub fn state_dir() -> Result<PathBuf, ConfigError> {
        let data_dir = Self::data_dir()?;
        Ok(data_dir.join("state").to_path_buf())
    }

    pub fn data_dir() -> Result<PathBuf, ConfigError> {
        let project_dir = Self::project_dirs()?;
        let data = project_dir.data_dir().to_path_buf();
        if !data.exists() {
            let _ = fs::create_dir_all(&data);
        }
        Ok(data)
    }

    pub fn logs_dir() -> Result<PathBuf, ConfigError> {
        let data_dir = Self::data_dir()?;
        let logs = data_dir.join("logs");
        if !logs.exists() {
            let _ = fs::create_dir_all(&logs);
        }
        Ok(logs)
    }

    pub fn log_file_path() -> Result<PathBuf, ConfigError> {
        let logs = Self::logs_dir()?;
        Ok(logs.join("hydroxy.log"))
    }

    fn config_dir() -> Result<PathBuf, ConfigError> {
        let project_dir = Self::project_dirs()?;
        Ok(project_dir.config_dir().to_path_buf())
    }

    pub fn project_dirs() -> Result<ProjectDirs, ConfigError> {
        ProjectDirs::from("org", "forthecommunity", "hydroxy_torrent").ok_or_else(|| {
            ConfigError::Error(
                "No valid home directory path could be retrieved from the operating system.".into(),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_file_path() {
        let path = HydroxyConfig::config_file_path().expect("Failed to get config path");
        assert!(path.ends_with(".config/hydroxy_torrent/hydroxy_config.toml"));
    }

    #[test]
    fn test_deserialize_legacy_config_without_session_config() {
        let toml_str = r#"
[network_config]
listen_address = "0.0.0.0"
listen_port = 9999

[storage_config]
watch_directory = "/tmp/watch"
download_directory = "/tmp/downloads"
max_active_downloads = 7
watch_directory_max_depth = 2

[remote_config]
rpc_enabled = true
web_ui_enabled = false
listen_address = "127.0.0.1"
listen_port = 10000
username = "admin"
password = "hashed_password"
"#;
        let config: HydroxyConfig = toml::from_str(toml_str).expect("Failed to deserialize legacy config");
        assert_eq!(config.effective_max_active_downloads(), 7);
        assert_eq!(config.session_config.max_active_uploads, 5);
        assert_eq!(config.session_config.max_peers_per_torrent, 50);
        assert_eq!(config.session_config.max_total_peers, 300);
        assert_eq!(config.storage_config.watch_directory_max_depth, 2);
    }

    #[test]
    fn test_deserialize_new_config_with_session_config() {
        let toml_str = r#"
[network_config]
listen_address = "0.0.0.0"
listen_port = 9999

[session_config]
max_active_downloads = 25
max_active_uploads = 10
max_download_speed_kbps = 5000
max_upload_speed_kbps = 1000
max_peers_per_torrent = 60
max_total_peers = 400

[storage_config]
watch_directory = "/tmp/watch"
download_directory = "/tmp/downloads"
watch_directory_max_depth = 3

[remote_config]
rpc_enabled = true
web_ui_enabled = false
listen_address = "127.0.0.1"
listen_port = 10000
username = "admin"
password = "hashed_password"
"#;
        let config: HydroxyConfig = toml::from_str(toml_str).expect("Failed to deserialize new config");
        assert_eq!(config.session_config.max_active_downloads, 25);
        assert_eq!(config.session_config.max_active_uploads, 10);
        assert_eq!(config.session_config.max_download_speed_kbps, 5000);
        assert_eq!(config.session_config.max_upload_speed_kbps, 1000);
        assert_eq!(config.session_config.max_peers_per_torrent, 60);
        assert_eq!(config.session_config.max_total_peers, 400);
        assert_eq!(config.effective_max_active_downloads(), 25);
    }

    #[test]
    fn test_deserialize_limitations_section() {
        let toml_str = r#"
[network_config]
listen_address = "0.0.0.0"
listen_port = 9999

[Limitations]
max_active_downloads = 15

[storage_config]
watch_directory = "/tmp/watch"
download_directory = "/tmp/downloads"

[remote_config]
rpc_enabled = true
web_ui_enabled = false
listen_address = "127.0.0.1"
listen_port = 10000
username = "admin"
password = "hashed_password"
"#;
        let config: HydroxyConfig = toml::from_str(toml_str).expect("Failed to deserialize limitations config");
        assert_eq!(config.session_config.max_active_downloads, 15);
        assert_eq!(config.effective_max_active_downloads(), 15);
        assert_eq!(config.session_config.max_active_uploads, 5);
    }
}
