use sqlx::SqlitePool;

use crate::error::AppError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FlapEvent {
    pub id: String,
    pub prefix: String,
    pub prefix_type: String,
    pub node_id: String,
    pub change_count: i32,
    pub window_start: String,
    pub window_end: String,
    pub source: String,
    pub active: bool,
    pub detected_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FlapStats {
    pub active_count: i32,
    pub total_today: i32,
    pub avg_changes_per_hour: f64,
}

#[derive(Clone)]
pub struct FlapEventRepository {
    pool: SqlitePool,
}

impl FlapEventRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, event: &FlapEvent) -> Result<FlapEvent, AppError> {
        sqlx::query(
            "INSERT INTO flap_events (id, prefix, prefix_type, node_id,
             change_count, window_start, window_end, source,
             active, detected_at, resolved_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&event.id)
        .bind(&event.prefix)
        .bind(&event.prefix_type)
        .bind(&event.node_id)
        .bind(event.change_count)
        .bind(&event.window_start)
        .bind(&event.window_end)
        .bind(&event.source)
        .bind(event.active)
        .bind(&event.detected_at)
        .bind(&event.resolved_at)
        .execute(&self.pool)
        .await?;

        Ok(event.clone())
    }

    pub async fn list_active(&self) -> Result<Vec<FlapEvent>, AppError> {
        sqlx::query_as::<_, FlapEvent>(
            "SELECT id, prefix, prefix_type, node_id,
             change_count, window_start, window_end, source,
             active, detected_at, resolved_at
             FROM flap_events
             WHERE active = 1
             ORDER BY detected_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn list_recent(&self, limit: i32) -> Result<Vec<FlapEvent>, AppError> {
        let limit = if limit <= 0 { 50 } else { limit as i64 };

        sqlx::query_as::<_, FlapEvent>(
            "SELECT id, prefix, prefix_type, node_id,
             change_count, window_start, window_end, source,
             active, detected_at, resolved_at
             FROM flap_events
             ORDER BY detected_at DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn update_change_count(
        &self,
        id: &str,
        change_count: i32,
        window_end: &str,
    ) -> Result<(), AppError> {
        sqlx::query("UPDATE flap_events SET change_count = ?, window_end = ? WHERE id = ?")
            .bind(change_count)
            .bind(window_end)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn resolve(&self, id: &str) -> Result<(), AppError> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE flap_events SET active = 0, resolved_at = ? WHERE id = ?")
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn stats(&self) -> Result<FlapStats, AppError> {
        let active_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM flap_events WHERE active = 1",
        )
        .fetch_one(&self.pool)
        .await?;

        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let total_today: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM flap_events WHERE detected_at >= ?",
        )
        .bind(&today)
        .fetch_one(&self.pool)
        .await?;

        let total_changes: Option<f64> = sqlx::query_scalar(
            "SELECT SUM(change_count) FROM flap_events WHERE detected_at >= ?",
        )
        .bind(&today)
        .fetch_optional(&self.pool)
        .await?;

        let midnight = chrono::Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .and_then(|t| t.and_local_timezone(chrono::Utc).latest())
            .unwrap_or_else(|| chrono::Utc::now());
        let hours_since_midnight = (chrono::Utc::now() - midnight).num_minutes() as f64 / 60.0;
        let elapsed = if hours_since_midnight < 1.0 {
            1.0
        } else {
            hours_since_midnight
        };
        let avg_changes_per_hour = total_changes.unwrap_or(0.0) / elapsed;

        Ok(FlapStats {
            active_count: active_count as i32,
            total_today: total_today as i32,
            avg_changes_per_hour,
        })
    }

    pub async fn find_active_by_prefix_node(
        &self,
        prefix: &str,
        node_id: &str,
    ) -> Result<Option<FlapEvent>, AppError> {
        sqlx::query_as::<_, FlapEvent>(
            "SELECT id, prefix, prefix_type, node_id,
             change_count, window_start, window_end, source,
             active, detected_at, resolved_at
             FROM flap_events
             WHERE prefix = ? AND node_id = ? AND active = 1
             ORDER BY detected_at DESC LIMIT 1",
        )
        .bind(prefix)
        .bind(node_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Into::into)
    }
}
