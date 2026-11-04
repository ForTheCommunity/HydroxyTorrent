use std::{
    collections::{BTreeSet, HashSet},
    time::{Duration, Instant},
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    api::{ApiClient, SavedSession},
    config::TuiConfig,
    models::{
        LogLevel, ParsedLogLine, StorageTargetItem, SystemStats, TorrentDetail, TorrentItem,
    },
    theme::ThemeKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Sidebar,
    Torrents,
    Details,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    General,
    Trackers,
    Peers,
    Files,
}

impl DetailTab {
    pub fn all() -> [Self; 4] {
        [Self::General, Self::Trackers, Self::Peers, Self::Files]
    }

    pub fn index(self) -> usize {
        match self {
            Self::General => 0,
            Self::Trackers => 1,
            Self::Peers => 2,
            Self::Files => 3,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::General => "1:General",
            Self::Trackers => "2:Trackers",
            Self::Peers => "3:Peers",
            Self::Files => "4:Content",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::General => Self::Trackers,
            Self::Trackers => Self::Peers,
            Self::Peers => Self::Files,
            Self::Files => Self::General,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::General => Self::Files,
            Self::Trackers => Self::General,
            Self::Peers => Self::Trackers,
            Self::Files => Self::Peers,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Name,
    Size,
    Progress,
    Status,
    Peers,
    DownSpeed,
    UpSpeed,
    Ratio,
}

impl SortColumn {
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Size => "Size",
            Self::Progress => "Progress",
            Self::Status => "Status",
            Self::Peers => "Peers",
            Self::DownSpeed => "Down Speed",
            Self::UpSpeed => "Up Speed",
            Self::Ratio => "Ratio",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Name => Self::Size,
            Self::Size => Self::Progress,
            Self::Progress => Self::Status,
            Self::Status => Self::Peers,
            Self::Peers => Self::DownSpeed,
            Self::DownSpeed => Self::UpSpeed,
            Self::UpSpeed => Self::Ratio,
            Self::Ratio => Self::Name,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginField {
    ServerUrl,
    Username,
    Password,
}

impl LoginField {
    pub fn next(self) -> Self {
        match self {
            Self::ServerUrl => Self::Username,
            Self::Username => Self::Password,
            Self::Password => Self::ServerUrl,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::ServerUrl => Self::Password,
            Self::Username => Self::ServerUrl,
            Self::Password => Self::Username,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalState {
    None,
    Login,
    SearchInput,
    RemoveConfirm {
        delete_files: bool,
        targets: Vec<(String, String)>, // (info_hash, name)
    },
    ShutdownConfirm,
    Logs,
    Help,
}

#[derive(Debug, Clone)]
pub struct SidebarEntry {
    pub label: String,
    pub filter_key: Option<String>, // None = All, Some("status:downloading"), Some("category:foo")
    pub count: usize,
    pub is_header: bool,
    pub depth: usize,
}

pub struct App {
    pub api: ApiClient,
    pub should_quit: bool,
    pub connection_state: ConnectionState,
    pub theme: ThemeKind,
    pub focus: FocusPane,
    pub show_sidebar: bool,
    pub show_details: bool,
    pub detail_tab: DetailTab,
    pub detail_scroll: usize,

    // Login state
    pub is_authenticated: bool,
    pub login_field: LoginField,
    pub login_url: String,
    pub login_user: String,
    pub login_pass: String,
    pub login_show_pass: bool,
    pub login_error: Option<String>,

    // Filtering & Search
    pub active_filter: Option<String>,
    pub sidebar_index: usize,
    pub search_query: String,

    // Sorting
    pub sort_col: SortColumn,
    pub sort_asc: bool,

    // Torrents & Selection
    pub all_torrents: Vec<TorrentItem>,
    pub storages: Vec<StorageTargetItem>,
    pub selected_index: usize,
    pub multi_selected: HashSet<String>,
    pub current_detail: Option<TorrentDetail>,
    pub system_stats: Option<SystemStats>,

    // Logs Modal State
    pub raw_logs: String,
    pub parsed_logs: Vec<ParsedLogLine>,
    pub log_level_filter: LogLevel,
    pub log_search_query: String,
    pub log_search_editing: bool,
    pub log_scroll: usize,
    pub log_auto_scroll: bool,

    // Modal & Status Notification
    pub modal: ModalState,
    pub toast: Option<(String, Instant)>,
}

impl App {
    pub fn new(cli_url: Option<String>, cli_user: Option<String>, cli_pass: Option<String>) -> Self {
        let tui_cfg = TuiConfig::load();

        let default_url = tui_cfg
            .server_url
            .clone()
            .unwrap_or_else(|| "http://127.0.0.1:10000".to_string());

        let default_user = tui_cfg
            .username
            .clone()
            .unwrap_or_else(|| "admin".to_string());

        let saved = SavedSession::load();

        let base_url = cli_url
            .or_else(|| saved.as_ref().map(|s| s.base_url.clone()))
            .unwrap_or(default_url);
        let username = cli_user
            .or_else(|| saved.as_ref().map(|s| s.username.clone()))
            .unwrap_or(default_user);
        let saved_token = saved.map(|s| s.token);

        let initial_theme = tui_cfg
            .theme
            .as_deref()
            .and_then(ThemeKind::from_str)
            .unwrap_or(ThemeKind::Dark);

        let api = ApiClient::new(base_url.clone(), saved_token.clone(), Some(username.clone()));

        Self {
            api,
            should_quit: false,
            connection_state: ConnectionState::Connecting,
            theme: initial_theme,
            focus: FocusPane::Torrents,
            show_sidebar: true,
            show_details: true,
            detail_tab: DetailTab::General,
            detail_scroll: 0,

            is_authenticated: saved_token.is_some(),
            login_field: if username.is_empty() {
                LoginField::Username
            } else {
                LoginField::Password
            },
            login_url: base_url,
            login_user: username,
            login_pass: cli_pass.unwrap_or_default(),
            login_show_pass: false,
            login_error: None,

            active_filter: None,
            sidebar_index: 0,
            search_query: String::new(),

            sort_col: SortColumn::DownSpeed,
            sort_asc: false,

            all_torrents: Vec::new(),
            storages: Vec::new(),
            selected_index: 0,
            multi_selected: HashSet::new(),
            current_detail: None,
            system_stats: None,

            raw_logs: String::new(),
            parsed_logs: Vec::new(),
            log_level_filter: LogLevel::All,
            log_search_query: String::new(),
            log_search_editing: false,
            log_scroll: 0,
            log_auto_scroll: true,

            modal: if saved_token.is_some() {
                ModalState::None
            } else {
                ModalState::Login
            },
            toast: None,
        }
    }

    pub fn set_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    pub fn active_toast(&self) -> Option<&str> {
        if let Some((ref msg, ts)) = self.toast {
            if ts.elapsed() < Duration::from_secs(4) {
                return Some(msg.as_str());
            }
        }
        None
    }

    pub fn sidebar_entries(&self) -> Vec<SidebarEntry> {
        let mut downloading = 0usize;
        let mut stalled = 0usize;
        let mut queued = 0usize;
        let mut seeding = 0usize;
        let mut completed = 0usize;
        let mut paused = 0usize;
        let mut checking = 0usize;

        for t in &self.all_torrents {
            let st = t.status.to_ascii_lowercase();
            match st.as_str() {
                "downloading" => downloading += 1,
                "stalled" => stalled += 1,
                "queued" => queued += 1,
                "seeding" => {
                    seeding += 1;
                    completed += 1;
                }
                "completed" => completed += 1,
                "paused" | "stopped" => paused += 1,
                "checking" => checking += 1,
                _ => {}
            }
        }

        let mut entries = vec![
            SidebarEntry {
                label: "All Torrents".to_string(),
                filter_key: None,
                count: self.all_torrents.len(),
                is_header: false,
                depth: 0,
            },
            SidebarEntry {
                label: "STATUS".to_string(),
                filter_key: None,
                count: 0,
                is_header: true,
                depth: 0,
            },
            SidebarEntry {
                label: "Downloading".to_string(),
                filter_key: Some("status:downloading".to_string()),
                count: downloading,
                is_header: false,
                depth: 1,
            },
            SidebarEntry {
                label: "Stalled".to_string(),
                filter_key: Some("status:stalled".to_string()),
                count: stalled,
                is_header: false,
                depth: 1,
            },
            SidebarEntry {
                label: "Queued".to_string(),
                filter_key: Some("status:queued".to_string()),
                count: queued,
                is_header: false,
                depth: 1,
            },
            SidebarEntry {
                label: "Seeding".to_string(),
                filter_key: Some("status:seeding".to_string()),
                count: seeding,
                is_header: false,
                depth: 1,
            },
            SidebarEntry {
                label: "Completed".to_string(),
                filter_key: Some("status:completed".to_string()),
                count: completed,
                is_header: false,
                depth: 1,
            },
            SidebarEntry {
                label: "Paused".to_string(),
                filter_key: Some("status:paused".to_string()),
                count: paused,
                is_header: false,
                depth: 1,
            },
        ];

        if checking > 0 {
            entries.push(SidebarEntry {
                label: "Checking".to_string(),
                filter_key: Some("status:checking".to_string()),
                count: checking,
                is_header: false,
                depth: 1,
            });
        }

        entries.push(SidebarEntry {
            label: "STORAGE DISKS".to_string(),
            filter_key: None,
            count: 0,
            is_header: true,
            depth: 0,
        });

        // Collect unique storage targets
        let mut target_names: Vec<String> = self.storages.iter().map(|s| s.name.clone()).collect();
        for t in &self.all_torrents {
            let sname = if t.storage.is_empty() { "default" } else { t.storage.as_str() };
            if !target_names.iter().any(|n| n == sname) {
                target_names.push(sname.to_string());
            }
        }
        if target_names.is_empty() {
            target_names.push("default".to_string());
        }

        for sname in &target_names {
            let storage_torrents: Vec<&TorrentItem> = self
                .all_torrents
                .iter()
                .filter(|t| {
                    let st = if t.storage.is_empty() { "default" } else { t.storage.as_str() };
                    st == sname
                })
                .collect();

            entries.push(SidebarEntry {
                label: format!("💾 {}", sname),
                filter_key: Some(format!("storage:{}", sname)),
                count: storage_torrents.len(),
                is_header: false,
                depth: 1,
            });

            // Gather all categories for this storage:
            // 1) From daemon's discovered categories (includes empty ones like "anime")
            let mut cats: BTreeSet<String> = BTreeSet::new();
            if let Some(target) = self.storages.iter().find(|s| &s.name == sname) {
                for c in &target.categories {
                    let trimmed = c.trim();
                    if !trimmed.is_empty() {
                        cats.insert(trimmed.to_string());
                    }
                }
            }
            // 2) From actual torrents assigned to this storage
            for t in &storage_torrents {
                let cat = t.category.trim();
                if !cat.is_empty() {
                    cats.insert(cat.to_string());
                }
            }

            for cat in &cats {
                let cat_count = storage_torrents
                    .iter()
                    .filter(|t| {
                        let tc = t.category.trim();
                        tc == cat || tc.starts_with(&format!("{}/", cat))
                    })
                    .count();

                entries.push(SidebarEntry {
                    label: format!("📁 {}", cat),
                    filter_key: Some(format!("storage:{}/category:{}", sname, cat)),
                    count: cat_count,
                    is_header: false,
                    depth: 2,
                });
            }

            let uncategorized_count = storage_torrents
                .iter()
                .filter(|t| t.category.trim().is_empty())
                .count();
            if uncategorized_count > 0 {
                entries.push(SidebarEntry {
                    label: "📁 Uncategorized".to_string(),
                    filter_key: Some(format!("storage:{}/category:", sname)),
                    count: uncategorized_count,
                    is_header: false,
                    depth: 2,
                });
            }
        }

        entries
    }

    pub fn filtered_torrents(&self) -> Vec<TorrentItem> {
        let q = self.search_query.trim().to_ascii_lowercase();

        let mut list: Vec<TorrentItem> = self
            .all_torrents
            .iter()
            .filter(|t| {
                if let Some(ref f) = self.active_filter {
                    if let Some(status_f) = f.strip_prefix("status:") {
                        let st = t.status.to_ascii_lowercase();
                        match status_f {
                            "completed" => {
                                if st != "completed" && st != "seeding" && t.progress < 1.0 {
                                    return false;
                                }
                            }
                            "paused" => {
                                if st != "paused" && st != "stopped" {
                                    return false;
                                }
                            }
                            other => {
                                if st != other {
                                    return false;
                                }
                            }
                        }
                    } else if let Some(storage_filter) = f.strip_prefix("storage:") {
                        let (target_storage, cat_filter) = if let Some((st, cat)) = storage_filter.split_once("/category:") {
                            (st, Some(cat))
                        } else {
                            (storage_filter, None)
                        };

                        let t_storage = if t.storage.is_empty() { "default" } else { t.storage.as_str() };
                        if t_storage != target_storage {
                            return false;
                        }

                        if let Some(cat_f) = cat_filter {
                            let tc = t.category.trim();
                            if cat_f.is_empty() {
                                if !tc.is_empty() {
                                    return false;
                                }
                            } else if tc != cat_f && !tc.starts_with(&format!("{}/", cat_f)) {
                                return false;
                            }
                        }
                    } else if let Some(cat_f) = f.strip_prefix("category:") {
                        let tc = t.category.trim();
                        if cat_f.is_empty() {
                            if !tc.is_empty() {
                                return false;
                            }
                        } else if tc != cat_f && !tc.starts_with(&format!("{}/", cat_f)) {
                            return false;
                        }
                    }
                }

                if !q.is_empty() {
                    let matches_name = t.name.to_ascii_lowercase().contains(&q);
                    let matches_hash = t.info_hash.to_ascii_lowercase().contains(&q);
                    let matches_cat = t.category.to_ascii_lowercase().contains(&q);
                    if !matches_name && !matches_hash && !matches_cat {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        list.sort_by(|a, b| {
            let ord = match self.sort_col {
                SortColumn::Name => a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()),
                SortColumn::Size => a.size.cmp(&b.size),
                SortColumn::Progress => a
                    .progress
                    .partial_cmp(&b.progress)
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Status => a
                    .status
                    .to_ascii_lowercase()
                    .cmp(&b.status.to_ascii_lowercase()),
                SortColumn::Peers => a.peers.cmp(&b.peers),
                SortColumn::DownSpeed => a
                    .download_speed
                    .partial_cmp(&b.download_speed)
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::UpSpeed => a
                    .upload_speed
                    .partial_cmp(&b.upload_speed)
                    .unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Ratio => a
                    .ratio()
                    .partial_cmp(&b.ratio())
                    .unwrap_or(std::cmp::Ordering::Equal),
            };
            if self.sort_asc {
                ord
            } else {
                ord.reverse()
            }
        });

        list
    }

    pub fn selected_torrent(&self) -> Option<TorrentItem> {
        let list = self.filtered_torrents();
        if list.is_empty() {
            None
        } else {
            let idx = self.selected_index.min(list.len() - 1);
            list.into_iter().nth(idx)
        }
    }

    pub fn action_targets(&self) -> Vec<(String, String)> {
        let list = self.filtered_torrents();
        if !self.multi_selected.is_empty() {
            list.into_iter()
                .filter(|t| self.multi_selected.contains(&t.info_hash))
                .map(|t| (t.info_hash, t.name))
                .collect()
        } else if let Some(t) = self.selected_torrent() {
            vec![(t.info_hash, t.name)]
        } else {
            Vec::new()
        }
    }

    pub fn clamp_selection(&mut self) {
        let len = self.filtered_torrents().len();
        if len == 0 {
            self.selected_index = 0;
        } else if self.selected_index >= len {
            self.selected_index = len - 1;
        }
    }

    pub fn update_logs(&mut self, logs: String) {
        self.raw_logs = if logs.trim().is_empty() {
            "No logs recorded yet.".to_string()
        } else {
            logs
        };
        self.parsed_logs = self
            .raw_logs
            .lines()
            .enumerate()
            .map(|(idx, line)| ParsedLogLine {
                number: idx + 1,
                text: line.to_string(),
                level: LogLevel::detect(line),
            })
            .collect();
        if self.log_auto_scroll {
            let count = self.filtered_logs().len();
            self.log_scroll = count.saturating_sub(1);
        }
    }

    pub fn filtered_logs(&self) -> Vec<&ParsedLogLine> {
        let q = self.log_search_query.trim().to_ascii_lowercase();
        self.parsed_logs
            .iter()
            .filter(|l| {
                if self.log_level_filter != LogLevel::All && l.level != self.log_level_filter {
                    return false;
                }
                if !q.is_empty() && !l.text.to_ascii_lowercase().contains(&q) {
                    return false;
                }
                true
            })
            .collect()
    }

    pub async fn refresh_all(&mut self) {
        if !self.is_authenticated {
            if !self.login_pass.is_empty() {
                let url = self.login_url.clone();
                let user = self.login_user.clone();
                let pass = self.login_pass.clone();
                self.api.set_base_url(&url);
                if self.api.login(&user, &pass).await.is_ok() {
                    self.is_authenticated = true;
                    self.modal = ModalState::None;
                    self.login_error = None;
                }
            }
            if !self.is_authenticated {
                return;
            }
        }

        match self.api.get_torrents().await {
            Ok((torrents, unauthorized)) => {
                if unauthorized {
                    self.is_authenticated = false;
                    self.modal = ModalState::Login;
                    self.login_error =
                        Some("Session expired or unauthorized. Please log in.".to_string());
                    return;
                }
                self.connection_state = ConnectionState::Connected;
                self.all_torrents = torrents;
                self.clamp_selection();
            }
            Err(_) => {
                self.connection_state = ConnectionState::Disconnected;
                return;
            }
        }

        if let Ok(sys) = self.api.get_system_stats().await {
            self.system_stats = Some(sys);
        }

        if let Ok(storages) = self.api.get_storages().await {
            self.storages = storages;
        }

        if self.show_details {
            if let Some(selected) = self.selected_torrent() {
                if let Ok(detail) = self.api.get_torrent_detail(&selected.info_hash).await {
                    self.current_detail = Some(detail);
                }
            } else {
                self.current_detail = None;
            }
        }

        if self.modal == ModalState::Logs {
            if let Ok(logs) = self.api.get_logs().await {
                self.update_logs(logs);
            }
        }
    }

    pub async fn handle_key(&mut self, key: KeyEvent) {
        // Ctrl+C always exits cleanly
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        match self.modal.clone() {
            ModalState::Login => {
                self.handle_login_key(key).await;
                return;
            }
            ModalState::SearchInput => {
                self.handle_search_key(key);
                return;
            }
            ModalState::RemoveConfirm {
                delete_files,
                targets,
            } => {
                self.handle_remove_confirm_key(key, delete_files, targets)
                    .await;
                return;
            }
            ModalState::ShutdownConfirm => {
                self.handle_shutdown_confirm_key(key).await;
                return;
            }
            ModalState::Logs => {
                self.handle_logs_key(key).await;
                return;
            }
            ModalState::Help => {
                if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Char('h')
                ) {
                    self.modal = ModalState::None;
                }
                return;
            }
            ModalState::None => {}
        }

        // Global shortcuts in main dashboard
        match key.code {
            KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Char('Q') => {
                self.modal = ModalState::ShutdownConfirm;
            }
            KeyCode::Char('O') => {
                self.api.logout().await;
                self.is_authenticated = false;
                self.login_pass.clear();
                self.modal = ModalState::Login;
                self.set_toast("Logged out from daemon");
            }
            KeyCode::Char('?') | KeyCode::Char('h') => {
                self.modal = ModalState::Help;
            }
            KeyCode::Char('t') => {
                self.theme = self.theme.next();
                let mut cfg = TuiConfig::load();
                cfg.theme = Some(self.theme.name().to_string());
                cfg.save();
                self.set_toast(format!("Theme: {}", self.theme.name()));
            }
            KeyCode::Char('b') => {
                self.show_sidebar = !self.show_sidebar;
                if !self.show_sidebar && self.focus == FocusPane::Sidebar {
                    self.focus = FocusPane::Torrents;
                }
            }
            KeyCode::Char('d') => {
                self.show_details = !self.show_details;
                if !self.show_details && self.focus == FocusPane::Details {
                    self.focus = FocusPane::Torrents;
                }
            }
            KeyCode::Char('/') => {
                self.modal = ModalState::SearchInput;
            }
            KeyCode::Char('l') => {
                self.modal = ModalState::Logs;
                if let Ok(logs) = self.api.get_logs().await {
                    self.update_logs(logs);
                }
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    FocusPane::Sidebar => FocusPane::Torrents,
                    FocusPane::Torrents => {
                        if self.show_details {
                            FocusPane::Details
                        } else if self.show_sidebar {
                            FocusPane::Sidebar
                        } else {
                            FocusPane::Torrents
                        }
                    }
                    FocusPane::Details => {
                        if self.show_sidebar {
                            FocusPane::Sidebar
                        } else {
                            FocusPane::Torrents
                        }
                    }
                };
            }
            KeyCode::BackTab => {
                self.focus = match self.focus {
                    FocusPane::Torrents => {
                        if self.show_sidebar {
                            FocusPane::Sidebar
                        } else if self.show_details {
                            FocusPane::Details
                        } else {
                            FocusPane::Torrents
                        }
                    }
                    FocusPane::Details => FocusPane::Torrents,
                    FocusPane::Sidebar => {
                        if self.show_details {
                            FocusPane::Details
                        } else {
                            FocusPane::Torrents
                        }
                    }
                };
            }
            KeyCode::Char('1') => {
                self.detail_tab = DetailTab::General;
                self.detail_scroll = 0;
                self.show_details = true;
            }
            KeyCode::Char('2') => {
                self.detail_tab = DetailTab::Trackers;
                self.detail_scroll = 0;
                self.show_details = true;
            }
            KeyCode::Char('3') => {
                self.detail_tab = DetailTab::Peers;
                self.detail_scroll = 0;
                self.show_details = true;
            }
            KeyCode::Char('4') => {
                self.detail_tab = DetailTab::Files;
                self.detail_scroll = 0;
                self.show_details = true;
            }
            KeyCode::Char('[') => {
                self.detail_tab = self.detail_tab.prev();
                self.detail_scroll = 0;
                self.show_details = true;
            }
            KeyCode::Char(']') => {
                self.detail_tab = self.detail_tab.next();
                self.detail_scroll = 0;
                self.show_details = true;
            }
            KeyCode::Char('s') => {
                self.sort_col = self.sort_col.next();
                self.set_toast(format!("Sort by: {}", self.sort_col.label()));
            }
            KeyCode::Char('S') => {
                self.sort_asc = !self.sort_asc;
                self.set_toast(format!(
                    "Sort {}: {}",
                    self.sort_col.label(),
                    if self.sort_asc { "Ascending ▲" } else { "Descending ▼" }
                ));
            }
            KeyCode::Char('p') => {
                let targets = self.action_targets();
                if !targets.is_empty() {
                    let mut ok = 0usize;
                    for (ih, _) in &targets {
                        if self.api.pause_torrent(ih).await.is_ok() {
                            ok += 1;
                        }
                    }
                    self.set_toast(format!("Paused {}/{} torrent(s)", ok, targets.len()));
                    self.refresh_all().await;
                }
            }
            KeyCode::Char('r') => {
                let targets = self.action_targets();
                if !targets.is_empty() {
                    let mut ok = 0usize;
                    for (ih, _) in &targets {
                        if self.api.resume_torrent(ih).await.is_ok() {
                            ok += 1;
                        }
                    }
                    self.set_toast(format!("Resumed {}/{} torrent(s)", ok, targets.len()));
                    self.refresh_all().await;
                }
            }
            KeyCode::Char('c') => {
                let targets = self.action_targets();
                if !targets.is_empty() {
                    let mut ok = 0usize;
                    for (ih, _) in &targets {
                        if self.api.recheck_torrent(ih).await.is_ok() {
                            ok += 1;
                        }
                    }
                    self.set_toast(format!("Recheck started for {}/{} torrent(s)", ok, targets.len()));
                    self.refresh_all().await;
                }
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let list = self.filtered_torrents();
                if self.multi_selected.len() == list.len() {
                    self.multi_selected.clear();
                    self.set_toast("Cleared selection");
                } else {
                    for t in list {
                        self.multi_selected.insert(t.info_hash);
                    }
                    self.set_toast(format!("Selected {} torrent(s)", self.multi_selected.len()));
                }
            }
            KeyCode::Char('a') => {
                let targets = self.action_targets();
                if !targets.is_empty() {
                    let mut ok = 0usize;
                    for (ih, _) in &targets {
                        if self.api.reannounce_torrent(ih).await.is_ok() {
                            ok += 1;
                        }
                    }
                    self.set_toast(format!(
                        "Force reannounced {}/{} torrent(s)",
                        ok,
                        targets.len()
                    ));
                    self.refresh_all().await;
                }
            }
            KeyCode::Char('o') => {
                if let Some(t) = self.selected_torrent() {
                    let next_seq = !t.sequential;
                    if self.api.set_sequential(&t.info_hash, next_seq).await.is_ok() {
                        self.set_toast(format!(
                            "Sequential download {} for {}",
                            if next_seq { "enabled" } else { "disabled" },
                            t.name
                        ));
                        self.refresh_all().await;
                    }
                }
            }
            KeyCode::Delete | KeyCode::Char('x') => {
                let targets = self.action_targets();
                if !targets.is_empty() {
                    self.modal = ModalState::RemoveConfirm {
                        delete_files: false,
                        targets,
                    };
                }
            }
            KeyCode::Esc => {
                if !self.multi_selected.is_empty() {
                    self.multi_selected.clear();
                    self.set_toast("Cleared multi-selection");
                } else if !self.search_query.is_empty() {
                    self.search_query.clear();
                    self.clamp_selection();
                } else if self.active_filter.is_some() {
                    self.active_filter = None;
                    self.sidebar_index = 0;
                    self.clamp_selection();
                }
            }
            _ => {
                // Pane-specific navigation
                match self.focus {
                    FocusPane::Sidebar => self.handle_sidebar_nav(key).await,
                    FocusPane::Torrents => self.handle_torrents_nav(key).await,
                    FocusPane::Details => self.handle_details_nav(key),
                }
            }
        }
    }

    async fn handle_sidebar_nav(&mut self, key: KeyEvent) {
        let entries = self.sidebar_entries();
        if entries.is_empty() {
            return;
        }
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                let mut next = self.sidebar_index + 1;
                while next < entries.len() && entries[next].is_header {
                    next += 1;
                }
                if next < entries.len() {
                    self.sidebar_index = next;
                    self.active_filter = entries[next].filter_key.clone();
                    self.selected_index = 0;
                    self.refresh_all().await;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let mut prev = self.sidebar_index;
                while prev > 0 {
                    prev -= 1;
                    if !entries[prev].is_header {
                        self.sidebar_index = prev;
                        self.active_filter = entries[prev].filter_key.clone();
                        self.selected_index = 0;
                        self.refresh_all().await;
                        break;
                    }
                }
            }
            KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => {
                if let Some(entry) = entries.get(self.sidebar_index) {
                    if !entry.is_header {
                        self.active_filter = entry.filter_key.clone();
                        self.selected_index = 0;
                        self.focus = FocusPane::Torrents;
                        self.refresh_all().await;
                    }
                }
            }
            _ => {}
        }
    }

    async fn handle_torrents_nav(&mut self, key: KeyEvent) {
        let list = self.filtered_torrents();
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if !list.is_empty() && self.selected_index + 1 < list.len() {
                    self.selected_index += 1;
                    self.detail_scroll = 0;
                    self.refresh_all().await;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                    self.detail_scroll = 0;
                    self.refresh_all().await;
                }
            }
            KeyCode::Home | KeyCode::Char('g') => {
                if !list.is_empty() {
                    self.selected_index = 0;
                    self.detail_scroll = 0;
                    self.refresh_all().await;
                }
            }
            KeyCode::End | KeyCode::Char('G') => {
                if !list.is_empty() {
                    self.selected_index = list.len() - 1;
                    self.detail_scroll = 0;
                    self.refresh_all().await;
                }
            }
            KeyCode::Char(' ') => {
                if let Some(t) = list.get(self.selected_index) {
                    if !self.multi_selected.remove(&t.info_hash) {
                        self.multi_selected.insert(t.info_hash.clone());
                    }
                }
            }
            KeyCode::Enter => {
                self.show_details = !self.show_details;
                if self.show_details {
                    self.refresh_all().await;
                }
            }
            _ => {}
        }
    }

    fn handle_details_nav(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.detail_scroll = self.detail_scroll.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.detail_scroll = self.detail_scroll.saturating_sub(1);
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.detail_tab = self.detail_tab.prev();
                self.detail_scroll = 0;
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.detail_tab = self.detail_tab.next();
                self.detail_scroll = 0;
            }
            KeyCode::PageDown => {
                self.detail_scroll = self.detail_scroll.saturating_add(8);
            }
            KeyCode::PageUp => {
                self.detail_scroll = self.detail_scroll.saturating_sub(8);
            }
            _ => {}
        }
    }

    async fn handle_login_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.should_quit = true;
            }
            KeyCode::Tab | KeyCode::Down => {
                self.login_field = self.login_field.next();
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.login_field = self.login_field.prev();
            }
            KeyCode::F(2) => {
                self.login_show_pass = !self.login_show_pass;
            }
            KeyCode::Enter => {
                let url = self.login_url.trim().to_string();
                let user = self.login_user.trim().to_string();
                let pass = self.login_pass.clone();
                if url.is_empty() || user.is_empty() || pass.is_empty() {
                    self.login_error =
                        Some("Server URL, username, and password are required.".to_string());
                    return;
                }
                self.api.set_base_url(&url);
                match self.api.login(&user, &pass).await {
                    Ok(_) => {
                        self.is_authenticated = true;
                        self.login_error = None;
                        self.modal = ModalState::None;
                        self.set_toast(format!("Connected as {}", user));
                        self.refresh_all().await;
                    }
                    Err(e) => {
                        self.login_error = Some(e.to_string());
                    }
                }
            }
            KeyCode::Backspace => match self.login_field {
                LoginField::ServerUrl => {
                    self.login_url.pop();
                }
                LoginField::Username => {
                    self.login_user.pop();
                }
                LoginField::Password => {
                    self.login_pass.pop();
                }
            },
            KeyCode::Char(c) => match self.login_field {
                LoginField::ServerUrl => self.login_url.push(c),
                LoginField::Username => self.login_user.push(c),
                LoginField::Password => self.login_pass.push(c),
            },
            _ => {}
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Esc => {
                self.modal = ModalState::None;
                self.clamp_selection();
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.clamp_selection();
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.clamp_selection();
            }
            _ => {}
        }
    }

    async fn handle_remove_confirm_key(
        &mut self,
        key: KeyEvent,
        delete_files: bool,
        targets: Vec<(String, String)>,
    ) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                self.modal = ModalState::None;
            }
            KeyCode::Tab | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => {
                self.modal = ModalState::RemoveConfirm {
                    delete_files: !delete_files,
                    targets,
                };
            }
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                let mut ok = 0usize;
                for (ih, _) in &targets {
                    if self.api.remove_torrent(ih, delete_files).await.is_ok() {
                        self.multi_selected.remove(ih);
                        ok += 1;
                    }
                }
                self.modal = ModalState::None;
                self.set_toast(format!(
                    "Removed {}/{} torrent(s){}",
                    ok,
                    targets.len(),
                    if delete_files { " + deleted files" } else { "" }
                ));
                self.refresh_all().await;
            }
            _ => {}
        }
    }

    async fn handle_shutdown_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                self.modal = ModalState::None;
            }
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                let _ = self.api.shutdown_daemon().await;
                self.modal = ModalState::None;
                self.connection_state = ConnectionState::Disconnected;
                self.set_toast("Shutdown signal sent to hydroxy_daemon");
            }
            _ => {}
        }
    }

    async fn handle_logs_key(&mut self, key: KeyEvent) {
        if self.log_search_editing {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => {
                    self.log_search_editing = false;
                }
                KeyCode::Backspace => {
                    self.log_search_query.pop();
                }
                KeyCode::Char(c) => {
                    self.log_search_query.push(c);
                }
                _ => {}
            }
            return;
        }

        let total = self.filtered_logs().len();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('l') => {
                self.modal = ModalState::None;
            }
            KeyCode::Char('/') => {
                self.log_search_editing = true;
            }
            KeyCode::Char('f') => {
                self.log_level_filter = self.log_level_filter.next_filter();
                self.log_scroll = 0;
            }
            KeyCode::Char('a') => {
                self.log_auto_scroll = !self.log_auto_scroll;
            }
            KeyCode::Char('r') => {
                if let Ok(logs) = self.api.get_logs().await {
                    self.update_logs(logs);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.log_auto_scroll = false;
                if self.log_scroll + 1 < total {
                    self.log_scroll += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.log_auto_scroll = false;
                self.log_scroll = self.log_scroll.saturating_sub(1);
            }
            KeyCode::PageDown => {
                self.log_auto_scroll = false;
                self.log_scroll = (self.log_scroll + 15).min(total.saturating_sub(1));
            }
            KeyCode::PageUp => {
                self.log_auto_scroll = false;
                self.log_scroll = self.log_scroll.saturating_sub(15);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.log_auto_scroll = false;
                self.log_scroll = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.log_auto_scroll = true;
                self.log_scroll = total.saturating_sub(1);
            }
            _ => {}
        }
    }
}
