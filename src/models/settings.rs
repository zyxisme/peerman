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
    pub wg_mtu: i64,
    pub wg_fwmark: i64,
    pub wg_post_up: String,
    pub wg_post_down: String,
    pub roa_mode: String,
    pub roa_static_v4_url: String,
    pub roa_static_v6_url: String,
    pub roa_rtr_address: String,
    pub roa_rtr_port: i64,
    pub bird_import_limit: i64,
    pub bird_export_filter: String,
    pub bird_import_filter: String,
    pub enable_community_filters: bool,
    pub enable_bfd: bool,
    pub bfd_interval_ms: i64,
    pub bfd_multiplier: i64,
    pub cluster_tunnel_ipv6_range: String,
    pub enable_confederation: bool,
    pub confederation_local_asn: i64,
}

#[derive(Clone)]
pub struct SettingsRepository {
    pool: SqlitePool,
}

const SETTINGS_COLUMNS: &str = "local_asn, bird_template_name, bird_router_id, \
    wg_default_listen_port, dn42_ipv4_prefix, dn42_ipv6_prefix, wg_table, \
    wg_mtu, wg_fwmark, wg_post_up, wg_post_down, \
    roa_mode, roa_static_v4_url, roa_static_v6_url, roa_rtr_address, roa_rtr_port, \
    bird_import_limit, bird_export_filter, bird_import_filter, \
    enable_community_filters, enable_bfd, bfd_interval_ms, bfd_multiplier, \
    cluster_tunnel_ipv6_range, enable_confederation, confederation_local_asn";

impl SettingsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn load(&self) -> Result<Settings, AppError> {
        let row = sqlx::query_as::<_, Settings>(&format!(
            "SELECT {SETTINGS_COLUMNS} FROM settings WHERE id = 1"
        ))
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    pub async fn save(&self, settings: &Settings) -> Result<Settings, AppError> {
        sqlx::query(
            "UPDATE settings SET
             local_asn = ?, bird_template_name = ?, bird_router_id = ?,
             wg_default_listen_port = ?, dn42_ipv4_prefix = ?, dn42_ipv6_prefix = ?,
             wg_table = ?,
             wg_mtu = ?, wg_fwmark = ?, wg_post_up = ?, wg_post_down = ?,
             roa_mode = ?, roa_static_v4_url = ?, roa_static_v6_url = ?,
             roa_rtr_address = ?, roa_rtr_port = ?,
             bird_import_limit = ?, bird_export_filter = ?, bird_import_filter = ?,
             enable_community_filters = ?, enable_bfd = ?, bfd_interval_ms = ?,
             bfd_multiplier = ?, cluster_tunnel_ipv6_range = ?,
             enable_confederation = ?, confederation_local_asn = ?
             WHERE id = 1",
        )
        .bind(settings.local_asn)
        .bind(&settings.bird_template_name)
        .bind(&settings.bird_router_id)
        .bind(settings.wg_default_listen_port)
        .bind(&settings.dn42_ipv4_prefix)
        .bind(&settings.dn42_ipv6_prefix)
        .bind(&settings.wg_table)
        .bind(settings.wg_mtu)
        .bind(settings.wg_fwmark)
        .bind(&settings.wg_post_up)
        .bind(&settings.wg_post_down)
        .bind(&settings.roa_mode)
        .bind(&settings.roa_static_v4_url)
        .bind(&settings.roa_static_v6_url)
        .bind(&settings.roa_rtr_address)
        .bind(settings.roa_rtr_port)
        .bind(settings.bird_import_limit)
        .bind(&settings.bird_export_filter)
        .bind(&settings.bird_import_filter)
        .bind(settings.enable_community_filters)
        .bind(settings.enable_bfd)
        .bind(settings.bfd_interval_ms)
        .bind(settings.bfd_multiplier)
        .bind(&settings.cluster_tunnel_ipv6_range)
        .bind(settings.enable_confederation)
        .bind(settings.confederation_local_asn)
        .execute(&self.pool)
        .await?;

        self.load().await
    }
}
