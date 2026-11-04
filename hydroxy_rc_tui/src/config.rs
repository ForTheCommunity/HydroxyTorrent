use std::{fs, path::PathBuf};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TuiConfig {
    #[serde(default)]
    pub server_url: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
}

impl TuiConfig {
    pub fn project_dirs() -> Option<ProjectDirs> {
        ProjectDirs::from("org", "forthecommunity", "hydroxy_rc_tui")
    }

    pub fn config_dir() -> Option<PathBuf> {
        let p = Self::project_dirs()?.config_dir().to_path_buf();
        let _ = fs::create_dir_all(&p);
        Some(p)
    }

    pub fn data_dir() -> Option<PathBuf> {
        let p = Self::project_dirs()?.data_dir().to_path_buf();
        let _ = fs::create_dir_all(&p);
        Some(p)
    }

    pub fn config_file_path() -> Option<PathBuf> {
        Some(Self::config_dir()?.join("config.toml"))
    }

    pub fn session_file_path() -> Option<PathBuf> {
        Some(Self::data_dir()?.join("session.json"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::config_file_path() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = toml::from_str(&content) {
                    return cfg;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::config_file_path() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Ok(s) = toml::to_string_pretty(self) {
                let _ = fs::write(path, s);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_config_and_data_paths() {
        let config_path = TuiConfig::config_file_path().expect("config path should resolve");
        assert!(
            config_path.ends_with("hydroxy_rc_tui/config.toml"),
            "Expected config path to end with hydroxy_rc_tui/config.toml, got: {:?}",
            config_path
        );

        let session_path = TuiConfig::session_file_path().expect("session path should resolve");
        assert!(
            session_path.ends_with("hydroxy_rc_tui/session.json"),
            "Expected session path to end with hydroxy_rc_tui/session.json, got: {:?}",
            session_path
        );
    }
}
