use std::time::Duration;

use sqlx::SqlitePool;
use tokio_util::sync::CancellationToken;

/// Spawn the data retention cleanup task.
///
/// Periodically deletes old probe results (>7 days) and resolved flap events (>30 days).
pub fn spawn_retention_cleanup(pool: SqlitePool, shutdown: CancellationToken) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => return,
                _ = interval.tick() => {}
            }
            // Clean probe results older than 7 days
            match sqlx::query(
                "DELETE FROM probe_results WHERE probed_at < datetime('now', '-7 days')",
            )
            .execute(&pool)
            .await
            {
                Ok(r) if r.rows_affected() > 0 => {
                    tracing::info!(
                        "Retention cleanup: deleted {} old probe results",
                        r.rows_affected()
                    );
                }
                Err(e) => tracing::warn!("Probe retention cleanup failed: {e}"),
                _ => {}
            }
            // Clean resolved flap events older than 30 days
            match sqlx::query("DELETE FROM flap_events WHERE active = 0 AND resolved_at IS NOT NULL AND resolved_at < datetime('now', '-30 days')")
                .execute(&pool).await
            {
                Ok(r) if r.rows_affected() > 0 => {
                    tracing::info!("Retention cleanup: deleted {} old flap events", r.rows_affected());
                }
                Err(e) => tracing::warn!("Flap retention cleanup failed: {e}"),
                _ => {}
            }
        }
    });
}
