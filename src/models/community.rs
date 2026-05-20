use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommunityRule {
    pub id: String,
    pub description: Option<String>,
    pub max_latency_ms: f64,
    pub max_packet_loss_pct: f64,
    pub community_ipv4: String,
    pub community_ipv6: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone)]
pub struct CommunityRuleRepository {
    pool: SqlitePool,
}

impl CommunityRuleRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_all(&self) -> Result<Vec<CommunityRule>, AppError> {
        sqlx::query_as::<_, CommunityRule>(
            "SELECT id, description, max_latency_ms, max_packet_loss_pct,
             community_ipv4, community_ipv6, enabled, created_at, updated_at
             FROM community_rules ORDER BY max_latency_ms ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn list_enabled(&self) -> Result<Vec<CommunityRule>, AppError> {
        sqlx::query_as::<_, CommunityRule>(
            "SELECT id, description, max_latency_ms, max_packet_loss_pct,
             community_ipv4, community_ipv6, enabled, created_at, updated_at
             FROM community_rules WHERE enabled = 1
             ORDER BY max_latency_ms ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn save(&self, rule: &CommunityRule) -> Result<CommunityRule, AppError> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query_as::<_, CommunityRule>(
            "INSERT INTO community_rules
             (id, description, max_latency_ms, max_packet_loss_pct,
              community_ipv4, community_ipv6, enabled, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
             description = excluded.description,
             max_latency_ms = excluded.max_latency_ms,
             max_packet_loss_pct = excluded.max_packet_loss_pct,
             community_ipv4 = excluded.community_ipv4,
             community_ipv6 = excluded.community_ipv6,
             enabled = excluded.enabled,
             updated_at = excluded.updated_at
             RETURNING id, description, max_latency_ms, max_packet_loss_pct,
             community_ipv4, community_ipv6, enabled, created_at, updated_at",
        )
        .bind(&rule.id)
        .bind(&rule.description)
        .bind(rule.max_latency_ms)
        .bind(rule.max_packet_loss_pct)
        .bind(&rule.community_ipv4)
        .bind(&rule.community_ipv6)
        .bind(rule.enabled)
        .bind(&now)
        .bind(&now)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM community_rules WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("CommunityRule {id} not found")));
        }
        Ok(())
    }

    /// Seed default rules on first run.
    pub async fn seed_defaults(&self, existing_count: i64) -> Result<(), AppError> {
        if existing_count > 0 {
            return Ok(());
        }

        let defaults = vec![
            ("Metro (<5ms)", 5.0, 1.0, "<asn>,10", "<asn>,610"),
            ("Regional (5-20ms)", 20.0, 1.0, "<asn>,20", "<asn>,620"),
            ("Continental (20-50ms)", 50.0, 2.0, "<asn>,30", "<asn>,630"),
            ("Intercontinental (50-150ms)", 150.0, 5.0, "<asn>,40", "<asn>,640"),
            ("High latency (>150ms)", 100_000.0, 100.0, "<asn>,50", "<asn>,650"),
        ];

        for (desc, max_lat, max_loss, c4, c6) in defaults {
            let rule = CommunityRule {
                id: Uuid::new_v4().to_string(),
                description: Some(desc.to_string()),
                max_latency_ms: max_lat,
                max_packet_loss_pct: max_loss,
                community_ipv4: c4.to_string(),
                community_ipv6: c6.to_string(),
                enabled: true,
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            };
            self.save(&rule).await?;
        }
        Ok(())
    }
}
