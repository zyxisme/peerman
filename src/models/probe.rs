use sqlx::SqlitePool;

use crate::error::AppError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProbeResult {
    pub id: String,
    pub from_node_id: String,
    pub to_node_id: String,
    pub avg_latency_ms: f64,
    pub min_latency_ms: f64,
    pub max_latency_ms: f64,
    pub packet_loss_pct: f64,
    pub packets_sent: i32,
    pub packets_received: i32,
    pub probed_at: String,
}

#[derive(Clone)]
pub struct ProbeResultRepository {
    pool: SqlitePool,
}

impl ProbeResultRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, result: &ProbeResult) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO probe_results (id, from_node_id, to_node_id,
             avg_latency_ms, min_latency_ms, max_latency_ms,
             packet_loss_pct, packets_sent, packets_received, probed_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&result.id)
        .bind(&result.from_node_id)
        .bind(&result.to_node_id)
        .bind(result.avg_latency_ms)
        .bind(result.min_latency_ms)
        .bind(result.max_latency_ms)
        .bind(result.packet_loss_pct)
        .bind(result.packets_sent)
        .bind(result.packets_received)
        .bind(&result.probed_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_by_filters(
        &self,
        from_node_id: &str,
        to_node_id: &str,
        limit: i32,
    ) -> Result<Vec<ProbeResult>, AppError> {
        let limit = if limit <= 0 { 100 } else { limit as i64 };

        let mut builder = sqlx::QueryBuilder::new(
            "SELECT id, from_node_id, to_node_id,
             avg_latency_ms, min_latency_ms, max_latency_ms,
             packet_loss_pct, packets_sent, packets_received, probed_at
             FROM probe_results",
        );

        let mut has_where = false;
        if !from_node_id.is_empty() {
            builder.push(" WHERE from_node_id = ");
            builder.push_bind(from_node_id);
            has_where = true;
        }
        if !to_node_id.is_empty() {
            if has_where {
                builder.push(" AND to_node_id = ");
            } else {
                builder.push(" WHERE to_node_id = ");
            }
            builder.push_bind(to_node_id);
        }

        builder.push(" ORDER BY probed_at DESC LIMIT ");
        builder.push_bind(limit);

        builder
            .build_query_as::<ProbeResult>()
            .fetch_all(&self.pool)
            .await
            .map_err(Into::into)
    }

    pub async fn latest_between(
        &self,
        from_node_id: &str,
        to_node_id: &str,
    ) -> Result<Option<ProbeResult>, AppError> {
        sqlx::query_as::<_, ProbeResult>(
            "SELECT id, from_node_id, to_node_id,
             avg_latency_ms, min_latency_ms, max_latency_ms,
             packet_loss_pct, packets_sent, packets_received, probed_at
             FROM probe_results
             WHERE from_node_id = ? AND to_node_id = ?
             ORDER BY probed_at DESC LIMIT 1",
        )
        .bind(from_node_id)
        .bind(to_node_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Into::into)
    }

}
