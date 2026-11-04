use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result, anyhow};
use hydroxy_config::HydroxyConfig;
use reqwest::{Client, RequestBuilder, StatusCode};
use serde::{Deserialize, Serialize};

use crate::models::{
    LoginPayload, LoginResponse, StorageTargetItem, SystemStats, TorrentDetail, TorrentItem,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SavedSession {
    pub base_url: String,
    pub username: String,
    pub token: String,
}

impl SavedSession {
    fn session_file_path() -> Option<PathBuf> {
        HydroxyConfig::data_dir()
            .ok()
            .map(|d| d.join("tui_session.json"))
    }

    pub fn load() -> Option<Self> {
        let path = Self::session_file_path()?;
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save(&self) {
        if let Some(path) = Self::session_file_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    pub fn clear() {
        if let Some(path) = Self::session_file_path() {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[derive(Clone)]
pub struct ApiClient {
    client: Client,
    pub base_url: String,
    pub token: Option<String>,
    pub username: Option<String>,
}

impl ApiClient {
    pub fn new(base_url: String, token: Option<String>, username: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .connect_timeout(Duration::from_secs(3))
            .pool_max_idle_per_host(4)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            base_url: Self::normalize_url(&base_url),
            token,
            username,
        }
    }

    pub fn normalize_url(raw: &str) -> String {
        let mut s = raw.trim().to_string();
        if !s.starts_with("http://") && !s.starts_with("https://") {
            s = format!("http://{}", s);
        }
        while s.ends_with('/') {
            s.pop();
        }
        s
    }

    pub fn set_base_url(&mut self, url: &str) {
        self.base_url = Self::normalize_url(url);
    }

    fn apply_auth(&self, req: RequestBuilder) -> RequestBuilder {
        if let Some(ref token) = self.token {
            req.header("Authorization", format!("Bearer {}", token))
                .header("X-Hydroxy-Token", token)
        } else {
            req
        }
    }

    pub async fn login(&mut self, username: &str, password: &str) -> Result<LoginResponse> {
        let url = format!("{}/api/auth/login", self.base_url);
        let payload = LoginPayload {
            username: username.to_string(),
            password: password.to_string(),
        };

        let resp = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| anyhow!("Could not connect to daemon at {}: {}", self.base_url, e))?;

        if resp.status() == StatusCode::UNAUTHORIZED {
            return Err(anyhow!("Invalid username or password"));
        }
        if !resp.status().is_success() {
            return Err(anyhow!("Login failed with HTTP {}", resp.status()));
        }

        let data: LoginResponse = resp
            .json()
            .await
            .context("Invalid login response from daemon")?;
        self.token = Some(data.token.clone());
        self.username = Some(data.username.clone());

        SavedSession {
            base_url: self.base_url.clone(),
            username: data.username.clone(),
            token: data.token.clone(),
        }
        .save();

        Ok(data)
    }

    pub async fn logout(&mut self) {
        let url = format!("{}/api/auth/logout", self.base_url);
        let _ = self.apply_auth(self.client.post(&url)).send().await;
        self.token = None;
        SavedSession::clear();
    }

    pub async fn get_torrents(&self) -> Result<(Vec<TorrentItem>, bool)> {
        let url = format!("{}/api/torrents", self.base_url);
        let resp = self.apply_auth(self.client.get(&url)).send().await?;
        if resp.status() == StatusCode::UNAUTHORIZED {
            return Ok((Vec::new(), true));
        }
        if !resp.status().is_success() {
            return Err(anyhow!("HTTP {}", resp.status()));
        }
        let items: Vec<TorrentItem> = resp.json().await?;
        Ok((items, false))
    }

    pub async fn get_storages(&self) -> Result<Vec<StorageTargetItem>> {
        let url = format!("{}/api/storages", self.base_url);
        let resp = self.apply_auth(self.client.get(&url)).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("Storages failed: HTTP {}", resp.status()));
        }
        Ok(resp.json().await?)
    }

    pub async fn get_torrent_detail(&self, info_hash: &str) -> Result<TorrentDetail> {
        let url = format!("{}/api/torrent/{}", self.base_url, info_hash);
        let resp = self.apply_auth(self.client.get(&url)).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("HTTP {}", resp.status()));
        }
        Ok(resp.json().await?)
    }

    pub async fn pause_torrent(&self, info_hash: &str) -> Result<()> {
        let url = format!("{}/api/torrent/{}/pause", self.base_url, info_hash);
        let resp = self.apply_auth(self.client.post(&url)).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("Pause failed: HTTP {}", resp.status()));
        }
        Ok(())
    }

    pub async fn resume_torrent(&self, info_hash: &str) -> Result<()> {
        let url = format!("{}/api/torrent/{}/resume", self.base_url, info_hash);
        let resp = self.apply_auth(self.client.post(&url)).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("Resume failed: HTTP {}", resp.status()));
        }
        Ok(())
    }

    pub async fn recheck_torrent(&self, info_hash: &str) -> Result<()> {
        let url = format!("{}/api/torrent/{}/recheck", self.base_url, info_hash);
        let resp = self.apply_auth(self.client.post(&url)).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("Recheck failed: HTTP {}", resp.status()));
        }
        Ok(())
    }

    pub async fn reannounce_torrent(&self, info_hash: &str) -> Result<()> {
        let url = format!("{}/api/torrent/{}/reannounce", self.base_url, info_hash);
        let resp = self.apply_auth(self.client.post(&url)).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("Reannounce failed: HTTP {}", resp.status()));
        }
        Ok(())
    }

    pub async fn set_sequential(&self, info_hash: &str, sequential: bool) -> Result<()> {
        let url = format!("{}/api/torrent/{}/sequential", self.base_url, info_hash);
        let resp = self
            .apply_auth(self.client.post(&url))
            .json(&serde_json::json!({ "sequential": sequential }))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("Sequential toggle failed: HTTP {}", resp.status()));
        }
        Ok(())
    }

    pub async fn remove_torrent(&self, info_hash: &str, delete_files: bool) -> Result<()> {
        let url = format!("{}/api/torrent/{}/remove", self.base_url, info_hash);
        let resp = self
            .apply_auth(self.client.post(&url))
            .json(&serde_json::json!({ "delete_files": delete_files }))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("Remove failed: HTTP {}", resp.status()));
        }
        Ok(())
    }

    pub async fn get_logs(&self) -> Result<String> {
        let url = format!("{}/api/logs", self.base_url);
        let resp = self.apply_auth(self.client.get(&url)).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("Logs failed: HTTP {}", resp.status()));
        }
        let val: serde_json::Value = resp.json().await?;
        Ok(val
            .get("logs")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string())
    }

    pub async fn get_system_stats(&self) -> Result<SystemStats> {
        let url = format!("{}/api/system/stats", self.base_url);
        let resp = self.apply_auth(self.client.get(&url)).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("Stats failed: HTTP {}", resp.status()));
        }
        Ok(resp.json().await?)
    }

    pub async fn shutdown_daemon(&self) -> Result<()> {
        let url = format!("{}/api/shutdown", self.base_url);
        // Ignore connection reset if daemon exits immediately
        let _ = self.apply_auth(self.client.post(&url)).send().await;
        Ok(())
    }
}
