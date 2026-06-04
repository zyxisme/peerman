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
    pub origin_node_id: Option<String>,
}

impl Peer {
    /// Apply proto Peer fields to this model.
    pub fn apply_proto(&mut self, proto: &crate::grpc::generated::Peer) {
        self.name = proto.name.clone();
        self.description = opt_string(&proto.description);
        self.asn = proto.asn;
        self.local_asn = proto.local_asn;
        self.wg_private_key = opt_string(&proto.wg_private_key);
        self.wg_public_key = opt_string(&proto.wg_public_key);
        self.wg_remote_address = proto.wg_remote_address.clone();
        self.wg_remote_port = proto.wg_remote_port as i64;
        self.wg_listen_port = proto.wg_listen_port as i64;
        self.wg_interface_name = proto.wg_interface_name.clone();
        self.ipv4_tunnel_local = opt_string(&proto.ipv4_tunnel_local);
        self.ipv4_tunnel_remote = opt_string(&proto.ipv4_tunnel_remote);
        self.ipv6_tunnel_local = opt_string(&proto.ipv6_tunnel_local);
        self.ipv6_tunnel_remote = opt_string(&proto.ipv6_tunnel_remote);
        self.multiprotocol = proto.multiprotocol;
        self.extended_nexthop = proto.extended_nexthop;
        self.sessions = proto.sessions;
        self.passive = proto.passive;
        self.import_max_prefix = opt_i64(proto.import_max_prefix);
        self.export_max_prefix = opt_i64(proto.export_max_prefix);
        self.origin_node_id = opt_string(&proto.origin_node_id);
        if !proto.updated_at.is_empty() {
            self.updated_at = proto.updated_at.clone();
        }
    }
}

fn opt_string(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

fn opt_i64(v: i32) -> Option<i64> {
    if v == 0 {
        None
    } else {
        Some(v as i64)
    }
}

impl From<&Peer> for crate::grpc::generated::Peer {
    fn from(p: &Peer) -> Self {
        Self {
            id: p.id.clone(),
            name: p.name.clone(),
            description: p.description.clone().unwrap_or_default(),
            asn: p.asn,
            local_asn: p.local_asn,
            wg_private_key: p.wg_private_key.clone().unwrap_or_default(),
            wg_public_key: p.wg_public_key.clone().unwrap_or_default(),
            wg_remote_address: p.wg_remote_address.clone(),
            wg_remote_port: p.wg_remote_port as u32,
            wg_listen_port: p.wg_listen_port as u32,
            wg_interface_name: p.wg_interface_name.clone(),
            ipv4_tunnel_local: p.ipv4_tunnel_local.clone().unwrap_or_default(),
            ipv4_tunnel_remote: p.ipv4_tunnel_remote.clone().unwrap_or_default(),
            ipv6_tunnel_local: p.ipv6_tunnel_local.clone().unwrap_or_default(),
            ipv6_tunnel_remote: p.ipv6_tunnel_remote.clone().unwrap_or_default(),
            multiprotocol: p.multiprotocol,
            extended_nexthop: p.extended_nexthop,
            sessions: p.sessions,
            passive: p.passive,
            import_max_prefix: p.import_max_prefix.unwrap_or(0) as i32,
            export_max_prefix: p.export_max_prefix.unwrap_or(0) as i32,
            enabled: p.enabled,
            created_at: p.created_at.clone(),
            updated_at: p.updated_at.clone(),
            origin_node_id: p.origin_node_id.clone().unwrap_or_default(),
        }
    }
}

#[derive(Clone)]
pub struct PeerRepository {
    pool: SqlitePool,
}

const PEER_COLUMNS: &str = "id, name, description, asn, local_asn, \
    wg_private_key, wg_public_key, wg_remote_address, wg_remote_port, wg_listen_port, wg_interface_name, \
    ipv4_tunnel_local, ipv4_tunnel_remote, ipv6_tunnel_local, ipv6_tunnel_remote, \
    multiprotocol, extended_nexthop, sessions, passive, \
    import_max_prefix, export_max_prefix, \
    enabled, created_at, updated_at, origin_node_id";

impl PeerRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_all(&self) -> Result<Vec<Peer>, AppError> {
        let peers =
            sqlx::query_as::<_, Peer>(&format!("SELECT {PEER_COLUMNS} FROM peers ORDER BY name"))
                .fetch_all(&self.pool)
                .await?;

        Ok(peers)
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Peer, AppError> {
        let peer =
            sqlx::query_as::<_, Peer>(&format!("SELECT {PEER_COLUMNS} FROM peers WHERE id = ?"))
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

        let peer = sqlx::query_as::<_, Peer>(&format!(
            "INSERT INTO peers (id, name, asn, local_asn, wg_remote_address, wg_remote_port, wg_listen_port, wg_interface_name, created_at, updated_at)
             VALUES (?, ?, 0, ?, '', 0, ?, '', ?, ?)
             RETURNING {PEER_COLUMNS}"
        ))
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

        let updated = sqlx::query_as::<_, Peer>(&format!(
            "UPDATE peers SET
             name = ?, description = ?, asn = ?, local_asn = ?,
             wg_private_key = ?, wg_public_key = ?, wg_remote_address = ?, wg_remote_port = ?, wg_listen_port = ?, wg_interface_name = ?,
             ipv4_tunnel_local = ?, ipv4_tunnel_remote = ?, ipv6_tunnel_local = ?, ipv6_tunnel_remote = ?,
             multiprotocol = ?, extended_nexthop = ?, sessions = ?, passive = ?,
             import_max_prefix = ?, export_max_prefix = ?, enabled = ?,
             origin_node_id = ?, updated_at = ?
             WHERE id = ?
             RETURNING {PEER_COLUMNS}"
        ))
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
        .bind(peer.enabled)
        .bind(&peer.origin_node_id)
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
        let now = Utc::now().to_rfc3339();
        let updated = sqlx::query_as::<_, Peer>(&format!(
            "UPDATE peers SET enabled = NOT enabled, updated_at = ?1
             WHERE id = ?2
             RETURNING {PEER_COLUMNS}"
        ))
        .bind(&now)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Peer {id} not found")))?;

        Ok(updated)
    }
}
