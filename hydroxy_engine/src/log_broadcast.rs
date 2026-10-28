use std::{
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use dashmap::DashMap;
use tokio::sync::broadcast;

const BROADCAST_CAPACITY: usize = 512;
const SUBSCRIBER_IDLE_TTL: Duration = Duration::from_secs(6);
const MAX_DRAIN_PER_POLL: usize = 500;

struct SubscriberEntry {
    rx: Mutex<broadcast::Receiver<String>>,
    last_poll: Mutex<Instant>,
}

struct MinorLogHub {
    tx: broadcast::Sender<String>,
    subscribers: DashMap<String, SubscriberEntry>,
    cleanup_running: AtomicBool,
}

static MINOR_LOG_HUB: OnceLock<Arc<MinorLogHub>> = OnceLock::new();

fn hub() -> &'static Arc<MinorLogHub> {
    MINOR_LOG_HUB.get_or_init(|| {
        let (tx, _) = broadcast::channel(BROADCAST_CAPACITY);
        Arc::new(MinorLogHub {
            tx,
            subscribers: DashMap::new(),
            cleanup_running: AtomicBool::new(false),
        })
    })
}

/// Returns true only when at least one live viewer is subscribed to minor logs.
/// When false, minor logs are neither formatted nor stored in memory or disk.
#[inline]
pub fn has_subscribers() -> bool {
    hub().tx.receiver_count() > 0
}

/// Broadcast a formatted minor log line to active live viewers, if any.
/// If no viewers are subscribed, the message is immediately dropped without buffering.
pub fn broadcast_minor_log(line: String) {
    let h = hub();
    if h.tx.receiver_count() > 0 {
        let _ = h.tx.send(line);
    }
}

/// Subscribe directly to the live minor log broadcast channel.
pub fn subscribe_receiver() -> broadcast::Receiver<String> {
    hub().tx.subscribe()
}

fn evict_expired(h: &MinorLogHub) {
    let now = Instant::now();
    h.subscribers.retain(|_, entry| {
        entry
            .last_poll
            .lock()
            .map(|ts| now.duration_since(*ts) <= SUBSCRIBER_IDLE_TTL)
            .unwrap_or(false)
    });
}

fn ensure_cleanup_task(h: &Arc<MinorLogHub>) {
    if h.cleanup_running
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        let hub_clone = Arc::clone(h);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(2));
                loop {
                    interval.tick().await;
                    evict_expired(&hub_clone);
                    if hub_clone.subscribers.is_empty() {
                        hub_clone.cleanup_running.store(false, Ordering::Release);
                        break;
                    }
                }
            });
        } else {
            h.cleanup_running.store(false, Ordering::Release);
        }
    }
}

/// Poll live minor logs for a given subscriber ID.
/// Creates a live subscription on first call and automatically expires if not polled within 6s.
pub fn poll_minor_logs(sub_id: &str) -> Vec<String> {
    let h = hub();
    evict_expired(h);

    let key = if sub_id.trim().is_empty() {
        "default"
    } else {
        sub_id.trim()
    };

    if !h.subscribers.contains_key(key) {
        let rx = h.tx.subscribe();
        h.subscribers.insert(
            key.to_string(),
            SubscriberEntry {
                rx: Mutex::new(rx),
                last_poll: Mutex::new(Instant::now()),
            },
        );
        ensure_cleanup_task(h);
    }

    let mut lines = Vec::new();
    if let Some(entry) = h.subscribers.get(key) {
        if let Ok(mut ts) = entry.last_poll.lock() {
            *ts = Instant::now();
        }
        if let Ok(mut rx) = entry.rx.lock() {
            while lines.len() < MAX_DRAIN_PER_POLL {
                match rx.try_recv() {
                    Ok(line) => lines.push(line),
                    Err(broadcast::error::TryRecvError::Lagged(skipped)) => {
                        lines.push(format!(
                            "WARN Live stream skipped {} minor log events due to high volume",
                            skipped
                        ));
                    }
                    Err(
                        broadcast::error::TryRecvError::Empty
                        | broadcast::error::TryRecvError::Closed,
                    ) => break,
                }
            }
        }
    }

    lines
}

/// Immediately unsubscribe a live viewer and drop its broadcast receiver.
pub fn unsubscribe_minor_logs(sub_id: &str) {
    let key = if sub_id.trim().is_empty() {
        "default"
    } else {
        sub_id.trim()
    };
    hub().subscribers.remove(key);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minor_log_not_stored_without_subscribers() {
        // Ensure clean state for test subscriber
        unsubscribe_minor_logs("test_sub_1");
        broadcast_minor_log("should_be_dropped_1".to_string());
        broadcast_minor_log("should_be_dropped_2".to_string());

        // First poll registers subscription; prior logs must NOT be in memory
        let initial = poll_minor_logs("test_sub_1");
        assert!(initial.is_empty());

        // Logs emitted while subscribed ARE received live
        broadcast_minor_log("live_minor_event_1".to_string());
        let live = poll_minor_logs("test_sub_1");
        assert_eq!(live, vec!["live_minor_event_1".to_string()]);

        // Unsubscribing drops the receiver immediately
        unsubscribe_minor_logs("test_sub_1");
        broadcast_minor_log("dropped_after_unsub".to_string());
        let after = poll_minor_logs("test_sub_1");
        assert!(after.is_empty());
        unsubscribe_minor_logs("test_sub_1");
    }
}
