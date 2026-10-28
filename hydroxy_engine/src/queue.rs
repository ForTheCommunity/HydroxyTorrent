use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};
use tokio::sync::{mpsc, Mutex};
use tracing::info;

pub struct DownloadQueueManager {
    inner: Arc<Mutex<QueueInner>>,
}

struct QueueInner {
    max_active: usize,
    active: HashSet<[u8; 20]>,
    stalled: HashSet<[u8; 20]>,
    waiting: VecDeque<[u8; 20]>,
    notifiers: HashMap<[u8; 20], mpsc::Sender<()>>,
}

impl DownloadQueueManager {
    pub fn new(max_active: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(QueueInner {
                max_active,
                active: HashSet::new(),
                stalled: HashSet::new(),
                waiting: VecDeque::new(),
                notifiers: HashMap::new(),
            })),
        }
    }

    pub async fn set_max_active(&self, max_active: usize) {
        let mut inner = self.inner.lock().await;
        inner.max_active = max_active;
        Self::promote_waiting(&mut inner).await;
    }

    /// Attempt to acquire an active download slot.
    /// Returns (is_immediate_active, slot_receiver).
    pub async fn acquire_download_slot(&self, info_hash: [u8; 20]) -> (bool, mpsc::Receiver<()>) {
        let mut inner = self.inner.lock().await;

        if inner.active.contains(&info_hash) || inner.stalled.contains(&info_hash) {
            let (_, rx) = mpsc::channel(1);
            return (true, rx);
        }

        // Clean up from waiting if present
        inner.waiting.retain(|h| h != &info_hash);
        inner.notifiers.remove(&info_hash);

        let under_active_limit = inner.max_active == 0 || inner.active.len() < inner.max_active;

        if under_active_limit {
            inner.active.insert(info_hash);
            let (_, rx) = mpsc::channel(1);
            info!(
                "Torrent {} acquired active download slot ({}/{} active, {} stalled)",
                hydroxy_utils::torrent_info::OtherInfos::to_sha1(&info_hash),
                inner.active.len(),
                inner.max_active,
                inner.stalled.len()
            );
            (true, rx)
        } else {
            let (tx, rx) = mpsc::channel(1);
            inner.waiting.push_back(info_hash);
            inner.notifiers.insert(info_hash, tx);
            info!(
                "Torrent {} queued for download (queue position: {}, active: {}/{}, stalled: {})",
                hydroxy_utils::torrent_info::OtherInfos::to_sha1(&info_hash),
                inner.waiting.len(),
                inner.active.len(),
                inner.max_active,
                inner.stalled.len()
            );
            (false, rx)
        }
    }

    /// Mark an active torrent as stalled (0 B/s download speed).
    /// Releases its quota against `max_active` so waiting torrents can be promoted.
    pub async fn mark_stalled(&self, info_hash: &[u8; 20]) {
        let mut inner = self.inner.lock().await;
        if inner.active.remove(info_hash) {
            inner.stalled.insert(*info_hash);
            info!(
                "Torrent {} marked stalled (0 B/s). Releasing active quota: {}/{} active, {} stalled",
                hydroxy_utils::torrent_info::OtherInfos::to_sha1(info_hash),
                inner.active.len(),
                inner.max_active,
                inner.stalled.len()
            );
            Self::promote_waiting(&mut inner).await;
        }
    }

    /// Mark a previously stalled torrent as active again because it resumed downloading data.
    pub async fn mark_active(&self, info_hash: &[u8; 20]) {
        let mut inner = self.inner.lock().await;
        if inner.stalled.remove(info_hash) {
            inner.active.insert(*info_hash);
            info!(
                "Torrent {} resumed active downloading. Active: {}/{}, Stalled: {}",
                hydroxy_utils::torrent_info::OtherInfos::to_sha1(info_hash),
                inner.active.len(),
                inner.max_active,
                inner.stalled.len()
            );
        }
    }

    /// Release an active or stalled download slot, or remove from the waiting queue.
    pub async fn release_download_slot(&self, info_hash: &[u8; 20]) {
        let mut inner = self.inner.lock().await;
        let was_active = inner.active.remove(info_hash);
        let was_stalled = inner.stalled.remove(info_hash);
        inner.waiting.retain(|h| h != info_hash);
        inner.notifiers.remove(info_hash);

        if was_active || was_stalled {
            info!(
                "Torrent {} released download slot ({}/{} active, {} stalled)",
                hydroxy_utils::torrent_info::OtherInfos::to_sha1(info_hash),
                inner.active.len(),
                inner.max_active,
                inner.stalled.len()
            );
            Self::promote_waiting(&mut inner).await;
        }
    }

    async fn promote_waiting(inner: &mut QueueInner) {
        while (inner.max_active == 0 || inner.active.len() < inner.max_active)
            && !inner.waiting.is_empty()
        {
            if let Some(next_hash) = inner.waiting.pop_front() {
                if let Some(notifier) = inner.notifiers.remove(&next_hash) {
                    inner.active.insert(next_hash);
                    info!(
                        "Promoted queued torrent {} to active download ({}/{} active, {} stalled)",
                        hydroxy_utils::torrent_info::OtherInfos::to_sha1(&next_hash),
                        inner.active.len(),
                        inner.max_active,
                        inner.stalled.len()
                    );
                    let _ = notifier.send(()).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_queue_concurrency_and_promotion() {
        let q = DownloadQueueManager::new(2);

        let hash1 = [1u8; 20];
        let hash2 = [2u8; 20];
        let hash3 = [3u8; 20];

        // First two should acquire immediately
        let (active1, _rx1) = q.acquire_download_slot(hash1).await;
        assert!(active1);

        let (active2, _rx2) = q.acquire_download_slot(hash2).await;
        assert!(active2);

        // Third should be queued
        let (active3, mut rx3) = q.acquire_download_slot(hash3).await;
        assert!(!active3);

        // Releasing hash1 should promote hash3
        q.release_download_slot(&hash1).await;

        tokio::time::timeout(std::time::Duration::from_millis(100), rx3.recv())
            .await
            .expect("timeout waiting for promotion")
            .expect("channel closed");

        // Now hash3 should be active
        let inner = q.inner.lock().await;
        assert!(inner.active.contains(&hash3));
        assert!(inner.active.contains(&hash2));
        assert!(!inner.active.contains(&hash1));
        assert_eq!(inner.waiting.len(), 0);
    }

    #[tokio::test]
    async fn test_stalled_torrent_releases_active_quota_and_promotes_waiting() {
        let q = DownloadQueueManager::new(2);

        let hash1 = [1u8; 20];
        let hash2 = [2u8; 20];
        let hash3 = [3u8; 20];

        let (active1, _rx1) = q.acquire_download_slot(hash1).await;
        assert!(active1);

        let (active2, _rx2) = q.acquire_download_slot(hash2).await;
        assert!(active2);

        let (active3, mut rx3) = q.acquire_download_slot(hash3).await;
        assert!(!active3);

        // Mark hash1 as stalled (0 B/s)
        q.mark_stalled(&hash1).await;

        // hash3 should now be automatically promoted without dropping hash1
        tokio::time::timeout(std::time::Duration::from_millis(100), rx3.recv())
            .await
            .expect("timeout waiting for promotion")
            .expect("channel closed");

        {
            let inner = q.inner.lock().await;
            assert!(inner.stalled.contains(&hash1));
            assert!(inner.active.contains(&hash2));
            assert!(inner.active.contains(&hash3));
            assert_eq!(inner.active.len(), 2);
            assert_eq!(inner.stalled.len(), 1);
            assert_eq!(inner.waiting.len(), 0);
        }

        // Now if hash1 resumes downloading speed, it is marked active again
        q.mark_active(&hash1).await;
        {
            let inner = q.inner.lock().await;
            assert!(inner.active.contains(&hash1));
            assert!(!inner.stalled.contains(&hash1));
        }

        // When hash1 finishes downloading, it releases cleanly
        q.release_download_slot(&hash1).await;
        {
            let inner = q.inner.lock().await;
            assert!(!inner.active.contains(&hash1));
        }
    }

    #[tokio::test]
    async fn test_queue_cancel_waiting_torrent() {
        let q = DownloadQueueManager::new(1);
        let hash1 = [1u8; 20];
        let hash2 = [2u8; 20];

        let (active1, _rx1) = q.acquire_download_slot(hash1).await;
        assert!(active1);

        let (active2, mut rx2) = q.acquire_download_slot(hash2).await;
        assert!(!active2);

        // Cancel hash2 while waiting
        q.release_download_slot(&hash2).await;

        {
            let inner = q.inner.lock().await;
            assert_eq!(inner.waiting.len(), 0);
            assert!(!inner.notifiers.contains_key(&hash2));
        }

        // Release hash1; hash2 channel was closed and dropped
        q.release_download_slot(&hash1).await;

        let res = rx2.recv().await;
        assert_eq!(res, None, "channel should be closed because notifier was dropped on cancel");
    }

    #[tokio::test]
    async fn test_queue_unlimited_active() {
        let q = DownloadQueueManager::new(0); // 0 = unlimited

        for i in 0..10 {
            let hash = [i as u8; 20];
            let (active, _rx) = q.acquire_download_slot(hash).await;
            assert!(active);
        }

        let inner = q.inner.lock().await;
        assert_eq!(inner.active.len(), 10);
        assert_eq!(inner.waiting.len(), 0);
    }

    #[tokio::test]
    async fn test_stalled_torrents_do_not_block_active_download_quota() {
        let max_active = 2;
        let q = DownloadQueueManager::new(max_active);

        // Fill active slots and mark them stalled repeatedly to simulate 20 stalled torrents
        for i in 0..20 {
            let mut hash = [0u8; 20];
            hash[0] = (i + 1) as u8;
            let (active, _rx) = q.acquire_download_slot(hash).await;
            assert!(active);
            q.mark_stalled(&hash).await;
        }

        {
            let inner = q.inner.lock().await;
            assert_eq!(inner.stalled.len(), 20);
            assert_eq!(inner.active.len(), 0);
        }

        // Now queue two more torrents. They should both acquire active slots immediately
        let hash_a = [100u8; 20];
        let hash_b = [101u8; 20];
        let hash_c = [102u8; 20];

        let (active_a, _rx_a) = q.acquire_download_slot(hash_a).await;
        let (active_b, _rx_b) = q.acquire_download_slot(hash_b).await;
        let (active_c, mut rx_c) = q.acquire_download_slot(hash_c).await;

        assert!(active_a, "Torrent A should be active");
        assert!(active_b, "Torrent B should be active");
        assert!(!active_c, "Torrent C should be queued since active == 2");

        // Release Torrent A; Torrent C must be promoted despite 20 stalled torrents
        q.release_download_slot(&hash_a).await;

        tokio::time::timeout(std::time::Duration::from_millis(100), rx_c.recv())
            .await
            .expect("timeout waiting for promotion")
            .expect("channel closed");

        let inner = q.inner.lock().await;
        assert_eq!(inner.active.len(), 2);
        assert!(inner.active.contains(&hash_b));
        assert!(inner.active.contains(&hash_c));
        assert_eq!(inner.stalled.len(), 20);
        assert_eq!(inner.waiting.len(), 0);
    }
}
