use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, Mutex};
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TorrentActivityClassification {
    /// Newly started torrent in its trial grace period, or running at/above the speed threshold
    FastOrTesting,
    /// Low-speed torrent (< threshold for >= inactive_timer)
    Slow,
}

pub struct DownloadQueueManager {
    inner: Arc<Mutex<QueueInner>>,
}

struct QueueInner {
    max_active: usize,
    dont_count_slow_torrents: bool,
    slow_dl_threshold: f64,
    slow_inactive_duration: Duration,

    /// All torrents currently holding an active running task
    active: HashMap<[u8; 20], TorrentActivityClassification>,
    /// Untried queued torrents (FIFO)
    waiting: VecDeque<[u8; 20]>,
    /// Completely stalled torrents (0 B/s for > inactive_timer) yielding slot to rotate
    stalled_waiting: VecDeque<([u8; 20], Instant)>,
    /// Channel notifiers to wake up tasks waiting for a slot
    notifiers: HashMap<[u8; 20], mpsc::Sender<()>>,
}

impl QueueInner {
    /// Count of active torrents that consume a fast download slot
    fn fast_count(&self) -> usize {
        if !self.dont_count_slow_torrents {
            self.active.len()
        } else {
            self.active
                .values()
                .filter(|c| **c == TorrentActivityClassification::FastOrTesting)
                .count()
        }
    }

    /// Check if another torrent can be promoted to active download
    fn can_promote(&self) -> bool {
        if self.max_active == 0 {
            return true;
        }

        // Safety ceiling on total concurrent running torrent tasks when slow torrents don't count
        let total_ceiling = if self.dont_count_slow_torrents {
            (self.max_active * 3).max(45)
        } else {
            self.max_active
        };

        if self.active.len() >= total_ceiling {
            return false;
        }

        self.fast_count() < self.max_active
    }
}

impl DownloadQueueManager {
    pub fn new(max_active: usize) -> Self {
        Self::with_config(max_active, true, 10_240.0, Duration::from_secs(60))
    }

    pub fn with_config(
        max_active: usize,
        dont_count_slow_torrents: bool,
        slow_dl_threshold: f64,
        slow_inactive_duration: Duration,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(QueueInner {
                max_active,
                dont_count_slow_torrents,
                slow_dl_threshold,
                slow_inactive_duration,
                active: HashMap::new(),
                waiting: VecDeque::new(),
                stalled_waiting: VecDeque::new(),
                notifiers: HashMap::new(),
            })),
        }
    }

    pub async fn set_max_active(&self, max_active: usize) {
        let mut inner = self.inner.lock().await;
        inner.max_active = max_active;
        Self::promote_waiting(&mut inner).await;
    }

    pub async fn update_config(
        &self,
        max_active: usize,
        dont_count_slow_torrents: bool,
        slow_dl_threshold: f64,
        slow_inactive_duration: Duration,
    ) {
        let mut inner = self.inner.lock().await;
        inner.max_active = max_active;
        inner.dont_count_slow_torrents = dont_count_slow_torrents;
        inner.slow_dl_threshold = slow_dl_threshold;
        inner.slow_inactive_duration = slow_inactive_duration;
        Self::promote_waiting(&mut inner).await;
    }

    pub async fn slow_dl_threshold(&self) -> f64 {
        let inner = self.inner.lock().await;
        inner.slow_dl_threshold
    }

    pub async fn slow_inactive_duration(&self) -> Duration {
        let inner = self.inner.lock().await;
        inner.slow_inactive_duration
    }

    pub async fn dont_count_slow_torrents(&self) -> bool {
        let inner = self.inner.lock().await;
        inner.dont_count_slow_torrents
    }

    /// Check if there are candidate torrents waiting for a download slot (either untried or cooled-down stalled).
    pub async fn has_waiting(&self) -> bool {
        let inner = self.inner.lock().await;
        if !inner.waiting.is_empty() {
            return true;
        }
        if let Some((_, stalled_at)) = inner.stalled_waiting.front() {
            return stalled_at.elapsed() >= Duration::from_secs(60);
        }
        false
    }

    /// Attempt to acquire an active download slot.
    /// Returns (is_immediate_active, slot_receiver).
    pub async fn acquire_download_slot(&self, info_hash: [u8; 20]) -> (bool, mpsc::Receiver<()>) {
        let mut inner = self.inner.lock().await;

        if inner.active.contains_key(&info_hash) {
            let (_, rx) = mpsc::channel(1);
            return (true, rx);
        }

        // Clean up from waiting or stalled queues if present
        inner.waiting.retain(|h| h != &info_hash);
        inner.stalled_waiting.retain(|(h, _)| h != &info_hash);
        inner.notifiers.remove(&info_hash);

        let can_promote = inner.can_promote();

        if can_promote {
            inner.active.insert(info_hash, TorrentActivityClassification::FastOrTesting);
            let (_, rx) = mpsc::channel(1);
            info!(
                "Torrent {} acquired active download slot (fast active: {}/{}, total active: {})",
                hydroxy_utils::torrent_info::OtherInfos::to_sha1(&info_hash),
                inner.fast_count(),
                inner.max_active,
                inner.active.len()
            );
            (true, rx)
        } else {
            let (tx, rx) = mpsc::channel(1);
            inner.waiting.push_back(info_hash);
            inner.notifiers.insert(info_hash, tx);
            info!(
                "Torrent {} queued for download (queue position: {}, fast active: {}/{})",
                hydroxy_utils::torrent_info::OtherInfos::to_sha1(&info_hash),
                inner.waiting.len(),
                inner.fast_count(),
                inner.max_active
            );
            (false, rx)
        }
    }

    /// Report current speed of an active torrent.
    /// If speed drops below the slow threshold for >= slow_inactive_duration, it transitions to Slow,
    /// freeing a fast slot for waiting torrents.
    /// If speed recovers >= slow threshold, it returns to FastOrTesting.
    pub async fn report_speed(&self, info_hash: &[u8; 20], speed: f64, is_below_threshold_for_timer: bool) {
        let mut inner = self.inner.lock().await;
        let slow_dl_threshold = inner.slow_dl_threshold;
        let dont_count_slow_torrents = inner.dont_count_slow_torrents;

        let should_promote = if let Some(classification) = inner.active.get_mut(info_hash) {
            if speed >= slow_dl_threshold {
                if *classification == TorrentActivityClassification::Slow {
                    *classification = TorrentActivityClassification::FastOrTesting;
                    info!(
                        "Torrent {} restored to fast active slot ({:.1} KB/s >= {:.1} KB/s threshold)",
                        hydroxy_utils::torrent_info::OtherInfos::to_sha1(info_hash),
                        speed / 1024.0,
                        slow_dl_threshold / 1024.0
                    );
                }
                false
            } else if dont_count_slow_torrents && is_below_threshold_for_timer {
                if *classification == TorrentActivityClassification::FastOrTesting {
                    *classification = TorrentActivityClassification::Slow;
                    info!(
                        "Torrent {} marked as slow ({:.1} KB/s < {:.1} KB/s threshold) — freeing fast quota slot for waiting torrents",
                        hydroxy_utils::torrent_info::OtherInfos::to_sha1(info_hash),
                        speed / 1024.0,
                        slow_dl_threshold / 1024.0
                    );
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        if should_promote {
            Self::promote_waiting(&mut inner).await;
        }
    }

    /// Yield an active download slot from a stalled torrent and register it to wait for a future slot.
    /// Promotes the next waiting torrent to fill the active quota.
    /// Returns a new Receiver to be notified when promoted again.
    pub async fn yield_stalled_slot(&self, info_hash: [u8; 20]) -> mpsc::Receiver<()> {
        let mut inner = self.inner.lock().await;
        inner.active.remove(&info_hash);
        inner.waiting.retain(|h| h != &info_hash);
        inner.stalled_waiting.retain(|(h, _)| h != &info_hash);
        inner.notifiers.remove(&info_hash);

        let (tx, rx) = mpsc::channel(1);
        inner.stalled_waiting.push_back((info_hash, Instant::now()));
        inner.notifiers.insert(info_hash, tx);

        info!(
            "Torrent {} yielded active download slot due to stall (fast active: {}/{}, {} queued, {} stalled waiting)",
            hydroxy_utils::torrent_info::OtherInfos::to_sha1(&info_hash),
            inner.fast_count(),
            inner.max_active,
            inner.waiting.len(),
            inner.stalled_waiting.len()
        );

        Self::promote_waiting(&mut inner).await;
        rx
    }

    /// Release an active download slot or remove from the waiting/stalled queue.
    pub async fn release_download_slot(&self, info_hash: &[u8; 20]) {
        let mut inner = self.inner.lock().await;
        let was_active = inner.active.remove(info_hash).is_some();
        inner.waiting.retain(|h| h != info_hash);
        inner.stalled_waiting.retain(|(h, _)| h != info_hash);
        inner.notifiers.remove(info_hash);

        if was_active {
            info!(
                "Torrent {} released download slot (fast active: {}/{}, total active: {})",
                hydroxy_utils::torrent_info::OtherInfos::to_sha1(info_hash),
                inner.fast_count(),
                inner.max_active,
                inner.active.len()
            );
            Self::promote_waiting(&mut inner).await;
        }
    }

    async fn promote_waiting(inner: &mut QueueInner) {
        while inner.can_promote() {
            // First draw from untried queued torrents; if exhausted, draw from cooled-down stalled waiting pool
            let next_hash = if let Some(h) = inner.waiting.pop_front() {
                h
            } else if let Some((h, stalled_at)) = inner.stalled_waiting.front().cloned() {
                if stalled_at.elapsed() >= Duration::from_secs(60) {
                    inner.stalled_waiting.pop_front();
                    h
                } else {
                    break;
                }
            } else {
                break;
            };

            if let Some(notifier) = inner.notifiers.remove(&next_hash) {
                inner.active.insert(next_hash, TorrentActivityClassification::FastOrTesting);
                info!(
                    "Promoted queued torrent {} to active download (fast active: {}/{}, total active: {})",
                    hydroxy_utils::torrent_info::OtherInfos::to_sha1(&next_hash),
                    inner.fast_count(),
                    inner.max_active,
                    inner.active.len()
                );
                let _ = notifier.send(()).await;
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
        assert!(inner.active.contains_key(&hash3));
        assert!(inner.active.contains_key(&hash2));
        assert!(!inner.active.contains_key(&hash1));
        assert_eq!(inner.waiting.len(), 0);
    }

    #[tokio::test]
    async fn test_stalled_slot_yielding_and_rotation() {
        let q = DownloadQueueManager::new(2);

        let hash1 = [1u8; 20];
        let hash2 = [2u8; 20];
        let hash3 = [3u8; 20];
        let hash4 = [4u8; 20];

        let (active1, _) = q.acquire_download_slot(hash1).await;
        let (active2, _) = q.acquire_download_slot(hash2).await;
        let (active3, mut rx3) = q.acquire_download_slot(hash3).await;
        let (active4, mut rx4) = q.acquire_download_slot(hash4).await;

        assert!(active1);
        assert!(active2);
        assert!(!active3);
        assert!(!active4);

        assert!(q.has_waiting().await);

        // hash1 is stalled and yields its slot
        let mut rx1_new = q.yield_stalled_slot(hash1).await;

        // hash3 should be promoted immediately
        tokio::time::timeout(std::time::Duration::from_millis(100), rx3.recv())
            .await
            .expect("timeout waiting for hash3 promotion")
            .expect("channel closed");

        {
            let inner = q.inner.lock().await;
            assert_eq!(inner.active.len(), 2, "Active count must strictly be 2");
            assert!(inner.active.contains_key(&hash2));
            assert!(inner.active.contains_key(&hash3));
            assert!(!inner.active.contains_key(&hash1));
            assert_eq!(inner.waiting.len(), 1); // hash4 still untried
            assert_eq!(inner.stalled_waiting.len(), 1); // hash1 in stalled_waiting
        }

        // hash2 is stalled and yields its slot
        let _rx2_new = q.yield_stalled_slot(hash2).await;

        // hash4 (untried) should be promoted next before hash1
        tokio::time::timeout(std::time::Duration::from_millis(100), rx4.recv())
            .await
            .expect("timeout waiting for hash4 promotion")
            .expect("channel closed");

        {
            let inner = q.inner.lock().await;
            assert_eq!(inner.active.len(), 2, "Active count must strictly be 2");
            assert!(inner.active.contains_key(&hash3));
            assert!(inner.active.contains_key(&hash4));
            assert_eq!(inner.waiting.len(), 0); // All untried exhausted
            assert_eq!(inner.stalled_waiting.len(), 2); // hash1 and hash2
        }

        // Now if hash3 finishes downloading and releases its slot:
        q.release_download_slot(&hash3).await;

        // hash1 has not cooled down 60s yet, so it won't be drawn immediately in short test without sleep
        {
            let inner = q.inner.lock().await;
            assert_eq!(inner.active.len(), 1);
            assert!(inner.active.contains_key(&hash4));
        }

        // Clear cooldown timestamp to simulate 60s elapsed
        {
            let mut inner = q.inner.lock().await;
            if let Some((_, ts)) = inner.stalled_waiting.front_mut() {
                *ts = Instant::now() - Duration::from_secs(61);
            }
        }
        // Trigger promotion
        q.set_max_active(2).await;

        tokio::time::timeout(std::time::Duration::from_millis(100), rx1_new.recv())
            .await
            .expect("timeout waiting for hash1 re-promotion")
            .expect("channel closed");

        {
            let inner = q.inner.lock().await;
            assert_eq!(inner.active.len(), 2, "Active count must strictly be 2");
            assert!(inner.active.contains_key(&hash4));
            assert!(inner.active.contains_key(&hash1));
        }
    }

    #[tokio::test]
    async fn test_dont_count_slow_torrents_qbittorrent_behavior() {
        let q = DownloadQueueManager::with_config(2, true, 10_240.0, Duration::from_secs(60));

        let hash1 = [1u8; 20];
        let hash2 = [2u8; 20];
        let hash3 = [3u8; 20];

        // 2 slots available
        let (active1, _) = q.acquire_download_slot(hash1).await;
        let (active2, _) = q.acquire_download_slot(hash2).await;
        let (active3, mut rx3) = q.acquire_download_slot(hash3).await;

        assert!(active1);
        assert!(active2);
        assert!(!active3, "Third torrent must wait when 2 fast slots are occupied");

        {
            let inner = q.inner.lock().await;
            assert_eq!(inner.fast_count(), 2);
            assert_eq!(inner.active.len(), 2);
        }

        // hash1 reports speed of only 2 KB/s (below 10 KB/s threshold) after timer expires
        q.report_speed(&hash1, 2048.0, true).await;

        // hash3 should be immediately promoted to active!
        tokio::time::timeout(std::time::Duration::from_millis(100), rx3.recv())
            .await
            .expect("timeout waiting for hash3 promotion after hash1 became slow")
            .expect("channel closed");

        {
            let inner = q.inner.lock().await;
            // hash1 is still active and downloading!
            assert!(inner.active.contains_key(&hash1));
            assert_eq!(inner.active.get(&hash1), Some(&TorrentActivityClassification::Slow));
            // hash2 and hash3 are fast active
            assert_eq!(inner.active.get(&hash2), Some(&TorrentActivityClassification::FastOrTesting));
            assert_eq!(inner.active.get(&hash3), Some(&TorrentActivityClassification::FastOrTesting));
            // Fast count is 2 (hash2 + hash3), total active is 3 (hash1 + hash2 + hash3)
            assert_eq!(inner.fast_count(), 2);
            assert_eq!(inner.active.len(), 3);
            assert_eq!(inner.waiting.len(), 0);
        }

        // If hash1 speeds up to 50 KB/s (above threshold), it is re-classified as Fast
        q.report_speed(&hash1, 51_200.0, false).await;
        {
            let inner = q.inner.lock().await;
            assert_eq!(inner.active.get(&hash1), Some(&TorrentActivityClassification::FastOrTesting));
            assert_eq!(inner.fast_count(), 3);
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
    async fn test_strict_max_active_limit_enforced() {
        // Test with dont_count_slow_torrents = false for strict limit enforcement
        let max_active = 15;
        let q = DownloadQueueManager::with_config(max_active, false, 10_240.0, Duration::from_secs(60));

        // Queue 50 torrents
        let mut rxs = Vec::new();
        for i in 0..50 {
            let mut hash = [0u8; 20];
            hash[0] = (i + 1) as u8;
            let (active, rx) = q.acquire_download_slot(hash).await;
            if i < 15 {
                assert!(active, "First 15 torrents must acquire active slot immediately");
            } else {
                assert!(!active, "Torrents beyond 15 must be queued");
                rxs.push((hash, rx));
            }
        }

        {
            let inner = q.inner.lock().await;
            assert_eq!(inner.active.len(), 15, "Active count must strictly equal 15");
            assert_eq!(inner.waiting.len(), 35, "Waiting count must equal 35");
        }

        // Release one active slot
        let mut finished_hash = [0u8; 20];
        finished_hash[0] = 1;
        q.release_download_slot(&finished_hash).await;

        // Exactly the 16th torrent should be promoted
        let (promoted_hash, mut rx) = rxs.remove(0);
        tokio::time::timeout(std::time::Duration::from_millis(100), rx.recv())
            .await
            .expect("timeout waiting for promotion")
            .expect("channel closed");

        {
            let inner = q.inner.lock().await;
            assert_eq!(inner.active.len(), 15, "Active count must remain strictly 15");
            assert!(inner.active.contains_key(&promoted_hash));
            assert!(!inner.active.contains_key(&finished_hash));
            assert_eq!(inner.waiting.len(), 34);
        }
    }
}
