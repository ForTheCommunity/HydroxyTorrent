use dashmap::DashMap;
use tokio::sync::mpsc;

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

#[derive(Default)]
pub struct TorrentControlRegistry {
    controllers: DashMap<String, mpsc::Sender<TorrentCommand>>,
}

impl TorrentControlRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, info_hash_hex: &str, sender: mpsc::Sender<TorrentCommand>) {
        self.controllers.insert(info_hash_hex.to_string(), sender);
    }

    pub fn unregister(&self, info_hash_hex: &str) {
        self.controllers.remove(info_hash_hex);
    }

    pub async fn send_command(&self, info_hash_hex: &str, cmd: TorrentCommand) -> bool {
        if let Some(entry) = self.controllers.get(info_hash_hex) {
            entry.value().send(cmd).await.is_ok()
        } else {
            false
        }
    }

    pub async fn broadcast_command(&self, cmd: TorrentCommand) {
        let senders: Vec<mpsc::Sender<TorrentCommand>> = self
            .controllers
            .iter()
            .map(|entry| entry.value().clone())
            .collect();
        for tx in senders {
            let _ = tx.send(cmd.clone()).await;
        }
    }
}
