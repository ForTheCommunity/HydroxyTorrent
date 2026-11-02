use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::{Notify, mpsc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TorrentCommand {
    Pause,
    Resume,
    Recheck,
    Reannounce,
    SetSequential(bool),
    Remove,
    Shutdown,
}

pub struct TorrentControlRegistry {
    controllers: DashMap<String, mpsc::Sender<TorrentCommand>>,
    shutting_down: AtomicBool,
    shutdown_notify: Arc<Notify>,
    empty_notify: Arc<Notify>,
}

impl Default for TorrentControlRegistry {
    fn default() -> Self {
        Self {
            controllers: DashMap::new(),
            shutting_down: AtomicBool::new(false),
            shutdown_notify: Arc::new(Notify::new()),
            empty_notify: Arc::new(Notify::new()),
        }
    }
}

impl TorrentControlRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    pub fn shutdown_notify(&self) -> Arc<Notify> {
        Arc::clone(&self.shutdown_notify)
    }

    pub fn active_count(&self) -> usize {
        self.controllers.len()
    }

    pub fn register(&self, info_hash_hex: &str, sender: mpsc::Sender<TorrentCommand>) {
        self.controllers.insert(info_hash_hex.to_string(), sender);
    }

    pub fn unregister(&self, info_hash_hex: &str) {
        self.controllers.remove(info_hash_hex);
        if self.controllers.is_empty() {
            self.empty_notify.notify_waiters();
        }
    }

    pub async fn send_command(&self, info_hash_hex: &str, cmd: TorrentCommand) -> bool {
        if let Some(entry) = self.controllers.get(info_hash_hex) {
            entry.value().send(cmd).await.is_ok()
        } else {
            false
        }
    }

    pub async fn broadcast_command(&self, cmd: TorrentCommand) {
        if cmd == TorrentCommand::Shutdown {
            self.shutting_down.store(true, Ordering::SeqCst);
            self.shutdown_notify.notify_waiters();
        }
        let senders: Vec<mpsc::Sender<TorrentCommand>> = self
            .controllers
            .iter()
            .map(|entry| entry.value().clone())
            .collect();
        for tx in senders {
            let _ = tx.send(cmd.clone()).await;
        }
    }

    /// Broadcasts Shutdown and waits until all active torrent tasks unregister,
    /// or until `timeout` has elapsed.
    /// Returns 0 if all tasks shut down cleanly, or the remaining pending count on timeout.
    pub async fn shutdown_and_wait(&self, timeout: std::time::Duration) -> usize {
        self.shutting_down.store(true, Ordering::SeqCst);
        self.shutdown_notify.notify_waiters();
        self.broadcast_command(TorrentCommand::Shutdown).await;

        if self.controllers.is_empty() {
            return 0;
        }

        let empty_notify = Arc::clone(&self.empty_notify);
        let _ = tokio::time::timeout(timeout, async {
            while !self.controllers.is_empty() {
                empty_notify.notified().await;
            }
        })
        .await;

        self.controllers.len()
    }
}
