use sqlx::SqlitePool;

use crate::error::AppError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Settings {
    pub local_asn: i64,
    pub bird_template_name: String,
    pub bird_router_id: String,
    pub wg_default_listen_port: i64,
    pub dn42_ipv4_prefix: String,
    pub dn42_ipv6_prefix: String,
    pub wg_table: String,
}

#[derive(Clone)]
pub struct SettingsRepository {
    pool: SqlitePool,
}

impl SettingsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn load(&self) -> Result<Settings, AppError> {
        let row = sqlx::query_as::<_, Settings>(
            "SELECT local_asn, bird_template_name, bird_router_id,
             wg_default_listen_port, dn42_ipv4_prefix, dn42_ipv6_prefix, wg_table
             FROM settings WHERE id = 1",
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    pub async fn save(&self, settings: &Settings) -> Result<Settings, AppError> {
        sqlx::query(
            "UPDATE settings SET
             local_asn = ?, bird_template_name = ?, bird_router_id = ?,
             wg_default_listen_port = ?, dn42_ipv4_prefix = ?, dn42_ipv6_prefix = ?,
             wg_table = ?
             WHERE id = 1",
        )
        .bind(settings.local_asn)
        .bind(&settings.bird_template_name)
        .bind(&settings.bird_router_id)
        .bind(settings.wg_default_listen_port)
        .bind(&settings.dn42_ipv4_prefix)
        .bind(&settings.dn42_ipv6_prefix)
        .bind(&settings.wg_table)
        .execute(&self.pool)
        .await?;

        self.load().await
    }
}
