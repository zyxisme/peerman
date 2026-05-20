use std::collections::HashMap;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::flap_event::{FlapEvent, FlapEventRepository};

const DEFAULT_WINDOW_SECS: i64 = 60;
const DEFAULT_CHANGE_THRESHOLD: u32 = 10;
const DEFAULT_RESOLVE_AFTER_SECS: i64 = 300;
const CHECK_INTERVAL_SECS: u64 = 30;
const TRACKER_TTL_SECS: i64 = 600;

struct PrefixTracker {
    prefix: String,
    prefix_type: String,
    changes: Vec<i64>,
    last_path_hash: u64,
    last_change_ts: i64,
}

pub struct FlapDetector {
    window_secs: i64,
    change_threshold: u32,
    resolve_after_secs: i64,
    node_id: String,
    repo: FlapEventRepository,
    trackers: HashMap<String, PrefixTracker>,
}

impl FlapDetector {
    pub fn new(node_id: String, repo: FlapEventRepository) -> Self {
        Self {
            window_secs: DEFAULT_WINDOW_SECS,
            change_threshold: DEFAULT_CHANGE_THRESHOLD,
            resolve_after_secs: DEFAULT_RESOLVE_AFTER_SECS,
            node_id,
            repo,
            trackers: HashMap::new(),
        }
    }

    pub async fn run(&mut self, mut rx: mpsc::Receiver<super::bgp_listener::PathChange>, token: CancellationToken) {
        loop {
            let tick = tokio::time::sleep(std::time::Duration::from_secs(CHECK_INTERVAL_SECS));
            tokio::select! {
                _ = token.cancelled() => {
                    tracing::info!("Flap detector shutting down");
                    return;
                }
                maybe_change = rx.recv() => {
                    match maybe_change {
                        Some(change) => {
                            if let Err(e) = self.process_change(change).await {
                                tracing::warn!("Flap detector error: {e}");
                            }
                        }
                        None => {
                            tracing::info!("BGP channel closed, flap detector exiting");
                            return;
                        }
                    }
                }
                _ = tick => {
                    if let Err(e) = self.resolve_stale().await {
                        tracing::warn!("Flap detector resolve error: {e}");
                    }
                    self.evict_trackers();
                }
            }
        }
    }

    fn evict_trackers(&mut self) {
        let now = chrono::Utc::now().timestamp();
        self.trackers.retain(|_, t| now - t.last_change_ts <= TRACKER_TTL_SECS);
    }

    async fn process_change(
        &mut self,
        change: super::bgp_listener::PathChange,
    ) -> Result<(), AppError> {
        let prefix_key = format!("{}:{}", change.prefix, change.node_id);
        let now = chrono::Utc::now().timestamp();

        struct Action {
            prefix: String,
            prefix_type: String,
            change_count: u32,
        }

        let action = if let Some(tracker) = self.trackers.get_mut(&prefix_key) {
            if tracker.last_path_hash == change.path_hash {
                return Ok(());
            }
            tracker.last_path_hash = change.path_hash;
            tracker.last_change_ts = now;
            tracker.changes.push(now);
            tracker.changes.retain(|t| now - t <= self.window_secs);

            let count = tracker.changes.len() as u32;
            if count >= self.change_threshold {
                Some(Action {
                    prefix: tracker.prefix.clone(),
                    prefix_type: tracker.prefix_type.clone(),
                    change_count: count,
                })
            } else {
                None
            }
        } else {
            let prefix_type = if change.prefix.contains(':') {
                "ipv6"
            } else {
                "ipv4"
            };
            self.trackers.insert(
                prefix_key,
                PrefixTracker {
                    prefix: change.prefix.clone(),
                    prefix_type: prefix_type.to_string(),
                    changes: vec![now],
                    last_path_hash: change.path_hash,
                    last_change_ts: now,
                },
            );
            None
        };

        if let Some(action) = action {
            let window_start =
                (chrono::Utc::now() - chrono::Duration::seconds(self.window_secs)).to_rfc3339();
            let window_end = chrono::Utc::now().to_rfc3339();

            if let Some(existing) = self
                .repo
                .find_active_by_prefix_node(&action.prefix, &self.node_id)
                .await?
            {
                self.repo
                    .update_change_count(&existing.id, action.change_count as i32, &window_end)
                    .await?;
            } else {
                let event = FlapEvent {
                    id: Uuid::new_v4().to_string(),
                    prefix: action.prefix.clone(),
                    prefix_type: action.prefix_type,
                    node_id: self.node_id.clone(),
                    change_count: action.change_count as i32,
                    window_start,
                    window_end,
                    source: "ibgp".to_string(),
                    active: true,
                    detected_at: chrono::Utc::now().to_rfc3339(),
                    resolved_at: None,
                };
                self.repo.create(&event).await?;
                tracing::warn!(
                    "Flap detected: {} ({} changes in {}s)",
                    event.prefix,
                    event.change_count,
                    self.window_secs,
                );
            }
        }

        Ok(())
    }

    async fn resolve_stale(&mut self) -> Result<(), AppError> {
        let now = chrono::Utc::now().timestamp();
        let active = self.repo.list_active().await?;

        for event in active {
            let key = format!("{}:{}", event.prefix, event.node_id);
            let should_resolve = if let Some(tracker) = self.trackers.get(&key) {
                let latest = tracker.changes.last().copied().unwrap_or(0);
                now - latest > self.resolve_after_secs
            } else {
                let detected = chrono::DateTime::parse_from_rfc3339(&event.detected_at)
                    .map(|d| d.timestamp())
                    .unwrap_or(0);
                now - detected > self.resolve_after_secs
            };

            if should_resolve {
                self.repo.resolve(&event.id).await?;
                tracing::info!("Flap resolved: {} (id={})", event.prefix, event.id);
            }
        }

        Ok(())
    }
}
