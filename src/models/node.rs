use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub listen_addr: String,
    pub local_asn: i64,
    pub description: Option<String>,
    pub online: bool,
    pub last_seen_at: String,
    pub created_at: String,
    pub updated_at: String,
    pub wg_pubkey: String,
    pub tunnel_ip: String,
    pub tunnel_ipv6: String,
}

#[derive(Clone)]
pub struct NodeRepository {
    pool: SqlitePool,
}

impl NodeRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_all(&self) -> Result<Vec<Node>, AppError> {
        sqlx::query_as::<_, Node>(
            "SELECT id, name, listen_addr, local_asn, description, online,
             last_seen_at, created_at, updated_at, wg_pubkey, tunnel_ip, tunnel_ipv6
             FROM nodes ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn find_by_id(&self, id: &str) -> Result<Node, AppError> {
        sqlx::query_as::<_, Node>(
            "SELECT id, name, listen_addr, local_asn, description, online,
             last_seen_at, created_at, updated_at, wg_pubkey, tunnel_ip, tunnel_ipv6
             FROM nodes WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Node {id} not found")))
    }

    pub async fn find_by_listen_addr(&self, addr: &str) -> Result<Option<Node>, AppError> {
        sqlx::query_as::<_, Node>(
            "SELECT id, name, listen_addr, local_asn, description, online,
             last_seen_at, created_at, updated_at, wg_pubkey, tunnel_ip, tunnel_ipv6
             FROM nodes WHERE listen_addr = ?",
        )
        .bind(addr)
        .fetch_optional(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn create(
        &self,
        name: &str,
        listen_addr: &str,
        local_asn: i64,
        description: &str,
    ) -> Result<Node, AppError> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query_as::<_, Node>(
            "INSERT INTO nodes (id, name, listen_addr, local_asn, description, wg_pubkey, tunnel_ip, tunnel_ipv6, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             RETURNING id, name, listen_addr, local_asn, description, online,
             last_seen_at, created_at, updated_at, wg_pubkey, tunnel_ip, tunnel_ipv6",
        )
        .bind(&id)
        .bind(name)
        .bind(listen_addr)
        .bind(local_asn)
        .bind(description)
        .bind("")
        .bind("")
        .bind("")
        .bind(&now)
        .bind(&now)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn update(&self, node: &Node) -> Result<Node, AppError> {
        let now = Utc::now().to_rfc3339();

        sqlx::query_as::<_, Node>(
            "UPDATE nodes SET name = ?, listen_addr = ?, local_asn = ?, description = ?,
             wg_pubkey = ?, tunnel_ip = ?, tunnel_ipv6 = ?, updated_at = ?
             WHERE id = ?
             RETURNING id, name, listen_addr, local_asn, description, online,
             last_seen_at, created_at, updated_at, wg_pubkey, tunnel_ip, tunnel_ipv6",
        )
        .bind(&node.name)
        .bind(&node.listen_addr)
        .bind(node.local_asn)
        .bind(&node.description)
        .bind(&node.wg_pubkey)
        .bind(&node.tunnel_ip)
        .bind(&node.tunnel_ipv6)
        .bind(&now)
        .bind(&node.id)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn delete(&self, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM nodes WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Node {id} not found")));
        }
        Ok(())
    }

    pub async fn mark_online(&self, id: &str) -> Result<(), AppError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE nodes SET online = 1, last_seen_at = ?1, updated_at = ?1 WHERE id = ?2",
        )
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_stale_node(&self, id: &str) -> Result<(), AppError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE nodes SET online = 0, updated_at = ?1 WHERE id = ?2")
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_cluster_fields(
        &self,
        id: &str,
        wg_pubkey: &str,
        tunnel_ip: &str,
    ) -> Result<(), AppError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE nodes SET wg_pubkey = ?, tunnel_ip = ?, updated_at = ? WHERE id = ?")
            .bind(wg_pubkey)
            .bind(tunnel_ip)
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_tunnel_ipv6(&self, id: &str, tunnel_ipv6: &str) -> Result<(), AppError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE nodes SET tunnel_ipv6 = ?, updated_at = ? WHERE id = ?")
            .bind(tunnel_ipv6)
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn mark_stale(&self, threshold_secs: i64) -> Result<(), AppError> {
        let threshold = Utc::now() - chrono::Duration::seconds(threshold_secs);
        sqlx::query("UPDATE nodes SET online = 0 WHERE online = 1 AND last_seen_at < ?")
            .bind(threshold.to_rfc3339())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn upsert_self(
        &self,
        name: &str,
        listen_addr: &str,
        local_asn: i64,
    ) -> Result<Node, AppError> {
        let existing = self.find_by_listen_addr(listen_addr).await?;
        match existing {
            Some(node) => {
                let mut updated = node;
                updated.name = name.to_string();
                updated.listen_addr = listen_addr.to_string();
                updated.local_asn = local_asn;
                self.update(&updated).await
            }
            None => self.create(name, listen_addr, local_asn, "").await,
        }
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Option<Node>, AppError> {
        sqlx::query_as::<_, Node>(
            "SELECT id, name, listen_addr, local_asn, description, online,
             last_seen_at, created_at, updated_at, wg_pubkey, tunnel_ip, tunnel_ipv6
             FROM nodes WHERE name = ?",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn upsert_by_name(
        &self,
        name: &str,
        listen_addr: &str,
        local_asn: i64,
        description: &str,
    ) -> Result<Node, AppError> {
        match self.find_by_name(name).await? {
            Some(node) => {
                let mut updated = node;
                updated.listen_addr = listen_addr.to_string();
                updated.local_asn = local_asn;
                updated.description = if description.is_empty() {
                    None
                } else {
                    Some(description.to_string())
                };
                updated.last_seen_at = Utc::now().to_rfc3339();
                self.update(&updated).await
            }
            None => self.create(name, listen_addr, local_asn, description).await,
        }
    }
}
