use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub asn: i64,
    pub local_asn: i64,
    pub wg_private_key: Option<String>,
    pub wg_public_key: Option<String>,
    pub wg_remote_address: String,
    pub wg_remote_port: i64,
    pub wg_listen_port: i64,
    pub wg_interface_name: String,
    pub ipv4_tunnel_local: Option<String>,
    pub ipv4_tunnel_remote: Option<String>,
    pub ipv6_tunnel_local: Option<String>,
    pub ipv6_tunnel_remote: Option<String>,
    pub multiprotocol: bool,
    pub extended_nexthop: bool,
    pub sessions: i32,
    pub passive: bool,
    pub import_max_prefix: Option<i64>,
    pub export_max_prefix: Option<i64>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone)]
pub struct PeerRepository {
    pool: SqlitePool,
}

impl PeerRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_all(&self) -> Result<Vec<Peer>, AppError> {
        let peers = sqlx::query_as::<_, Peer>(
            "SELECT id, name, description, asn, local_asn,
             wg_private_key, wg_public_key, wg_remote_address, wg_remote_port, wg_listen_port, wg_interface_name,
             ipv4_tunnel_local, ipv4_tunnel_remote, ipv6_tunnel_local, ipv6_tunnel_remote,
             multiprotocol, extended_nexthop, sessions, passive,
             import_max_prefix, export_max_prefix,
             enabled, created_at, updated_at
             FROM peers ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(peers)
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Peer, AppError> {
        let peer = sqlx::query_as::<_, Peer>(
            "SELECT id, name, description, asn, local_asn,
             wg_private_key, wg_public_key, wg_remote_address, wg_remote_port, wg_listen_port, wg_interface_name,
             ipv4_tunnel_local, ipv4_tunnel_remote, ipv6_tunnel_local, ipv6_tunnel_remote,
             multiprotocol, extended_nexthop, sessions, passive,
             import_max_prefix, export_max_prefix,
             enabled, created_at, updated_at
             FROM peers WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Peer {id} not found")))?;

        Ok(peer)
    }

    pub async fn create(&self, name: &str) -> Result<Peer, AppError> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let settings = crate::models::settings::SettingsRepository::new(self.pool.clone())
            .load()
            .await?;

        let peer = sqlx::query_as::<_, Peer>(
            "INSERT INTO peers (id, name, asn, local_asn, wg_remote_address, wg_remote_port, wg_listen_port, wg_interface_name, created_at, updated_at)
             VALUES (?, ?, 0, ?, '', 0, ?, '', ?, ?)
             RETURNING id, name, description, asn, local_asn,
             wg_private_key, wg_public_key, wg_remote_address, wg_remote_port, wg_listen_port, wg_interface_name,
             ipv4_tunnel_local, ipv4_tunnel_remote, ipv6_tunnel_local, ipv6_tunnel_remote,
             multiprotocol, extended_nexthop, sessions, passive,
             import_max_prefix, export_max_prefix,
             enabled, created_at, updated_at",
        )
        .bind(&id)
        .bind(name)
        .bind(settings.local_asn)
        .bind(settings.wg_default_listen_port)
        .bind(&now)
        .bind(&now)
        .fetch_one(&self.pool)
        .await?;

        Ok(peer)
    }

    pub async fn update(&self, peer: &Peer) -> Result<Peer, AppError> {
        let now = Utc::now().to_rfc3339();

        let updated = sqlx::query_as::<_, Peer>(
            "UPDATE peers SET
             name = ?, description = ?, asn = ?, local_asn = ?,
             wg_private_key = ?, wg_public_key = ?, wg_remote_address = ?, wg_remote_port = ?, wg_listen_port = ?, wg_interface_name = ?,
             ipv4_tunnel_local = ?, ipv4_tunnel_remote = ?, ipv6_tunnel_local = ?, ipv6_tunnel_remote = ?,
             multiprotocol = ?, extended_nexthop = ?, sessions = ?, passive = ?,
             import_max_prefix = ?, export_max_prefix = ?,
             updated_at = ?
             WHERE id = ?
             RETURNING id, name, description, asn, local_asn,
             wg_private_key, wg_public_key, wg_remote_address, wg_remote_port, wg_listen_port, wg_interface_name,
             ipv4_tunnel_local, ipv4_tunnel_remote, ipv6_tunnel_local, ipv6_tunnel_remote,
             multiprotocol, extended_nexthop, sessions, passive,
             import_max_prefix, export_max_prefix,
             enabled, created_at, updated_at",
        )
        .bind(&peer.name)
        .bind(&peer.description)
        .bind(peer.asn)
        .bind(peer.local_asn)
        .bind(&peer.wg_private_key)
        .bind(&peer.wg_public_key)
        .bind(&peer.wg_remote_address)
        .bind(peer.wg_remote_port)
        .bind(peer.wg_listen_port)
        .bind(&peer.wg_interface_name)
        .bind(&peer.ipv4_tunnel_local)
        .bind(&peer.ipv4_tunnel_remote)
        .bind(&peer.ipv6_tunnel_local)
        .bind(&peer.ipv6_tunnel_remote)
        .bind(peer.multiprotocol)
        .bind(peer.extended_nexthop)
        .bind(peer.sessions)
        .bind(peer.passive)
        .bind(peer.import_max_prefix)
        .bind(peer.export_max_prefix)
        .bind(&now)
        .bind(&peer.id)
        .fetch_one(&self.pool)
        .await?;

        Ok(updated)
    }

    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM peers WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Peer {id} not found")));
        }
        Ok(())
    }

    pub async fn toggle_enabled(&self, id: &str) -> Result<Peer, AppError> {
        let peer = self.find_by_id(id).await?;
        let new_enabled = !peer.enabled;
        let now = Utc::now().to_rfc3339();

        let updated = sqlx::query_as::<_, Peer>(
            "UPDATE peers SET enabled = ?, updated_at = ? WHERE id = ?
             RETURNING id, name, description, asn, local_asn,
             wg_private_key, wg_public_key, wg_remote_address, wg_remote_port, wg_listen_port, wg_interface_name,
             ipv4_tunnel_local, ipv4_tunnel_remote, ipv6_tunnel_local, ipv6_tunnel_remote,
             multiprotocol, extended_nexthop, sessions, passive,
             import_max_prefix, export_max_prefix,
             enabled, created_at, updated_at",
        )
        .bind(new_enabled)
        .bind(&now)
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(updated)
    }
}
