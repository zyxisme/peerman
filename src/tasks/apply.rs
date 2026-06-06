use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sqlx::SqlitePool;
use tokio_util::sync::CancellationToken;

use crate::app_state::PeerState;

/// Spawn the debounced WG+BIRD config apply task.
///
/// Checks the dirty flag every 5 seconds. When set, applies WG and BIRD
/// configurations and clears the flag.
pub fn spawn_config_apply(
    config_dirty: Arc<AtomicBool>,
    peer_state: PeerState,
    listen_addr: String,
    pool: SqlitePool,
    shutdown: CancellationToken,
    apply_status: crate::app_state::ApplyStatus,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("Config apply task shutting down");
                    return;
                }
                _ = interval.tick() => {}
            }
            if config_dirty.swap(false, Ordering::Relaxed) {
                tracing::info!("Config dirty flag set, applying WG+BIRD configs...");
                if let Err(e) =
                    crate::grpc::peer_service::apply_wg_bird(&peer_state, &listen_addr, &pool, &apply_status).await
                {
                    tracing::warn!("Auto-apply WG+BIRD failed: {e}");
                }
            }
        }
    });
}
