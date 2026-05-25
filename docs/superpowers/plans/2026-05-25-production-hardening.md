# Production Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 8 production readiness issues across auth, cluster completeness, and code quality identified in the 2026-05-25 audit.

**Architecture:** Three independent groups — Group A (auth hardening) touches `peer_service.rs`, `main.rs`, and `App.tsx`; Group B (cluster completeness) touches `bird_service.rs`, `cluster_service.rs`, and `main.rs`; Group C (code quality) touches config, system paths, flap_event model, and adds integration tests. Groups can be implemented in parallel.

**Tech Stack:** Rust (tonic, axum, sqlx), TypeScript (React, react-router-dom), SQLite

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `src/grpc/peer_service.rs` | Modify | Add `check_auth` to 4 sensitive config-export endpoints; use configurable WG path |
| `src/main.rs` | Modify | Hard-fail on empty password; create ClusterAggregator; pass to services; use configurable paths |
| `frontend/src/App.tsx` | Modify | Wrap `/export` and `/communities` in `ProtectedRoute` |
| `src/grpc/bird_service.rs` | Modify | Replace cross-node stub with real gRPC proxy; add cross-node traceroute fan-out |
| `src/grpc/cluster_service.rs` | Modify | Wire `ClusterAggregator` fanout into `list_probe_results` and `list_community_rules` |
| `src/config.rs` | Modify | Add `wg_config_path`, `bird_config_path`, `bird_socket_path` to `StorageConfig` |
| `src/services/bird.rs` | Modify | Accept bird config path from config instead of hardcoded |
| `src/services/bird_socket.rs` | Modify | Accept socket path as parameter instead of const |
| `src/cluster/tunnel.rs` | Modify | Accept WG cluster config path from config |
| `src/models/flap_event.rs` | Modify | Replace deprecated `date_naive()` with `format()` |
| `src/auth.rs` | Modify | Append `#[cfg(test)]` module with JWT unit tests |
| `src/grpc/peer_service.rs` | Modify | Append `#[cfg(test)]` module with auth rejection tests |
| `src/cluster/auth.rs` | Modify | Append `#[cfg(test)]` module with cluster key auth tests |

---

### Task 1: Add auth to 4 sensitive config-export gRPC endpoints

**Files:**
- Modify: `src/grpc/peer_service.rs:279-358`

- [ ] **Step 1: Add check_auth to get_wire_guard_config (line 279)**

Add as the first line in the handler body, after `let req = request.into_inner();`:

```rust
async fn get_wire_guard_config(
    &self,
    request: Request<GetConfigRequest>,
) -> Result<Response<ConfigResponse>, Status> {
    crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
    let req = request.into_inner();
    // ... rest unchanged
```

- [ ] **Step 2: Add check_auth to get_bird_config (line 299)**

```rust
async fn get_bird_config(
    &self,
    request: Request<GetConfigRequest>,
) -> Result<Response<ConfigResponse>, Status> {
    crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
    let req = request.into_inner();
    // ... rest unchanged
```

- [ ] **Step 3: Add check_auth to export_all_wire_guard (line 319)**

```rust
async fn export_all_wire_guard(
    &self,
    request: Request<ExportAllRequest>,
) -> Result<Response<ConfigResponse>, Status> {
    crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
    // ... rest unchanged
```

- [ ] **Step 4: Add check_auth to export_all_bird (line 344)**

```rust
async fn export_all_bird(
    &self,
    request: Request<ExportAllRequest>,
) -> Result<Response<ConfigResponse>, Status> {
    crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
    // ... rest unchanged
```

- [ ] **Step 5: Build and verify**

Run: `cargo build`
Expected: compiles cleanly (no new imports needed — `check_auth` already imported in this file via `use crate::auth`)

- [ ] **Step 6: Commit**

```bash
git add src/grpc/peer_service.rs
git commit -m "fix: add auth checks to sensitive config-export gRPC endpoints"
```

---

### Task 2: Hard-fail on empty password at startup

**Files:**
- Modify: `src/main.rs:195-197`

- [ ] **Step 1: Replace warning with hard error**

Old:
```rust
if cfg.auth.password.is_empty() {
    tracing::warn!("No auth password configured — login will always fail");
}
```

New:
```rust
if cfg.auth.password.is_empty() {
    anyhow::bail!(
        "auth.password must be set in config.toml. Empty password is not allowed for security reasons."
    );
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo build`
Expected: compiles

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "fix: hard-fail on empty auth password instead of warning"
```

---

### Task 3: Wrap frontend /export and /communities routes in ProtectedRoute

**Files:**
- Modify: `frontend/src/App.tsx:53,60`

- [ ] **Step 1: Wrap /export route (line 53)**

Old:
```tsx
<Route path="/export" element={<ExportPage />} />
```

New:
```tsx
<Route path="/export" element={<ProtectedRoute><ExportPage /></ProtectedRoute>} />
```

- [ ] **Step 2: Wrap /communities route (line 60)**

Old:
```tsx
<Route path="/communities" element={<CommunityRules />} />
```

New:
```tsx
<Route path="/communities" element={<ProtectedRoute><CommunityRules /></ProtectedRoute>} />
```

- [ ] **Step 3: Type-check**

Run: `cd frontend && pnpm exec tsc --noEmit`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add frontend/src/App.tsx
git commit -m "fix: add ProtectedRoute wrapper to /export and /communities pages"
```

---

### Task 4: Implement cross-node BIRD command execution

**Files:**
- Modify: `src/grpc/bird_service.rs:1-98`
- Modify: `src/main.rs:258-261`

- [ ] **Step 1: Add new fields and imports to BirdServiceImpl**

Replace the struct and imports at the top of the file:

Old:
```rust
use tonic::{Request, Response, Status};

use super::generated::{
    bird_service_server::BirdService, ExecuteCommandRequest, ExecuteCommandResponse,
    NodeBirdResult, NodeTracerouteResult, RunTracerouteRequest, RunTracerouteResponse,
};

use crate::services::bird_socket::BirdSocket;

pub struct BirdServiceImpl {
    pub node_name: String,
    pub jwt_secret: std::sync::Arc<String>,
}
```

New:
```rust
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tonic::transport::Endpoint;

use super::generated::{
    bird_service_client::BirdServiceClient,
    bird_service_server::BirdService, ExecuteCommandRequest, ExecuteCommandResponse,
    NodeBirdResult, NodeTracerouteResult, RunTracerouteRequest, RunTracerouteResponse,
};

use crate::models::node::NodeRepository;
use crate::services::bird_socket::BirdSocket;

pub struct BirdServiceImpl {
    pub node_name: String,
    pub jwt_secret: Arc<String>,
    pub node_repo: NodeRepository,
    pub cluster_key: Arc<String>,
}
```

- [ ] **Step 2: Replace the cross-node stub in execute_command (lines 34-42)**

Old:
```rust
        } else {
            // Remote node — not yet wired, return placeholder
            vec![NodeBirdResult {
                node_id: req.target_node_id.clone(),
                node_name: req.target_node_id.clone(),
                output: String::new(),
                status_code: 1,
                error: "Cross-node BIRD execution not yet implemented".to_string(),
            }]
        };
```

New:
```rust
        } else {
            // Proxy to remote node via gRPC
            let target_addr = self
                .node_repo
                .find_by_id(&req.target_node_id)
                .await
                .map_err(|e| Status::not_found(format!("target node not found: {e}")))?
                .listen_addr;
            let uri = format!("http://{target_addr}");
            let channel = Endpoint::from_shared(uri)
                .map_err(|e| Status::internal(format!("invalid uri: {e}")))?
                .connect()
                .await
                .map_err(|e| Status::unavailable(format!("connect to {target_addr}: {e}")))?;
            let mut client = BirdServiceClient::new(channel);
            let mut remote_req = Request::new(ExecuteCommandRequest {
                target_node_id: req.target_node_id.clone(),
                command: req.command.clone(),
            });
            if !self.cluster_key.is_empty() {
                if let Ok(val) = self.cluster_key.as_str().parse() {
                    remote_req.metadata_mut().insert("x-cluster-key", val);
                }
            }
            client
                .execute_command(remote_req)
                .await
                .map(|r| r.into_inner().results)
                .unwrap_or_else(|e| vec![NodeBirdResult {
                    node_id: req.target_node_id.clone(),
                    node_name: req.target_node_id.clone(),
                    output: String::new(),
                    status_code: 1,
                    error: format!("remote BIRD call failed: {e}"),
                }])
        };
```

- [ ] **Step 3: Update main.rs BirdServiceImpl construction (line 258-261)**

Old:
```rust
    let bird_svc = BirdServiceImpl {
        node_name: node_name.clone(),
        jwt_secret: jwt_secret.clone(),
    };
```

New:
```rust
    let bird_svc = BirdServiceImpl {
        node_name: node_name.clone(),
        jwt_secret: jwt_secret.clone(),
        node_repo: state.node_repo.clone(),
        cluster_key: Arc::new(cluster_key.clone()),
    };
```

- [ ] **Step 4: Build and verify**

Run: `cargo build`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add src/grpc/bird_service.rs src/main.rs
git commit -m "feat: implement cross-node BIRD command execution via gRPC proxy"
```

---

### Task 5: Wire ClusterAggregator fanout into cluster list handlers

**Files:**
- Modify: `src/grpc/cluster_service.rs:1-31,247-303`
- Modify: `src/main.rs:248-257`

- [ ] **Step 1: Add aggregator field and import to ClusterServiceImpl**

Add to imports at the top of the file (after line 15):
```rust
use crate::cluster::aggregator::{AggregatedResult, ClusterAggregator};
use crate::cluster::cache::ClusterCache;
```

Add field to struct (line 22-31):
```rust
pub struct ClusterServiceImpl {
    pub node_repo: NodeRepository,
    pub peer_repo: PeerRepository,
    pub probe_repo: ProbeResultRepository,
    pub community_repo: CommunityRuleRepository,
    pub settings_repo: SettingsRepository,
    pub jwt_secret: std::sync::Arc<String>,
    pub cluster_key: std::sync::Arc<String>,
    pub listen_addr: String,
    pub aggregator: std::sync::Arc<ClusterAggregator>,
}
```

- [ ] **Step 2: Wire fanout into list_probe_results (lines 247-261)**

Old:
```rust
    async fn list_probe_results(
        &self,
        request: Request<ListProbeResultsRequest>,
    ) -> Result<Response<ListProbeResultsResponse>, Status> {
        let req = request.into_inner();
        let results = self
            .probe_repo
            .list_by_filters(&req.from_node_id, &req.to_node_id, req.limit)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ListProbeResultsResponse {
            results: results.iter().map(probe_result_to_proto).collect(),
        }))
    }
```

New:
```rust
    async fn list_probe_results(
        &self,
        request: Request<ListProbeResultsRequest>,
    ) -> Result<Response<ListProbeResultsResponse>, Status> {
        let req = request.into_inner();
        let mut results: Vec<ProbeResult> = self
            .probe_repo
            .list_by_filters(&req.from_node_id, &req.to_node_id, req.limit)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .iter()
            .map(probe_result_to_proto)
            .collect();

        // Fan-out to other online nodes in cluster mode
        if let Ok(nodes) = self.node_repo.list_all().await {
            let online: Vec<_> = nodes.into_iter().filter(|n| n.online).collect();
            if online.len() > 1 {
                let aggregated = self.aggregator
                    .fanout_probe_results(&self.listen_addr, &online)
                    .await;
                results.extend(aggregated.items);
            }
        }

        Ok(Response::new(ListProbeResultsResponse { results }))
    }
```

- [ ] **Step 3: Wire fanout into list_community_rules (lines 290-303)**

Old:
```rust
    async fn list_community_rules(
        &self,
        _request: Request<ListCommunityRulesRequest>,
    ) -> Result<Response<ListCommunityRulesResponse>, Status> {
        let rules = self
            .community_repo
            .list_all()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ListCommunityRulesResponse {
            rules: rules.iter().map(community_rule_to_proto).collect(),
        }))
    }
```

New:
```rust
    async fn list_community_rules(
        &self,
        _request: Request<ListCommunityRulesRequest>,
    ) -> Result<Response<ListCommunityRulesResponse>, Status> {
        let mut rules: Vec<CommunityRule> = self
            .community_repo
            .list_all()
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .iter()
            .map(community_rule_to_proto)
            .collect();

        // Fan-out to other online nodes in cluster mode
        if let Ok(nodes) = self.node_repo.list_all().await {
            let online: Vec<_> = nodes.into_iter().filter(|n| n.online).collect();
            if online.len() > 1 {
                let aggregated = self.aggregator
                    .fanout_community_rules(&self.listen_addr, &online)
                    .await;
                rules.extend(aggregated.items);
            }
        }

        Ok(Response::new(ListCommunityRulesResponse { rules }))
    }
```

- [ ] **Step 4: Create ClusterCache and ClusterAggregator in main.rs (before line 248)**

Insert before the `ClusterServiceImpl` construction:
```rust
    let cluster_cache = cluster::cache::ClusterCache::new();
    let cluster_aggregator = Arc::new(cluster::aggregator::ClusterAggregator::new(
        cluster_cache,
        cluster_key.clone(),
    ));
```

- [ ] **Step 5: Pass aggregator to ClusterServiceImpl in main.rs (lines 248-257)**

Add the field to the construction:
```rust
    let cluster_svc = ClusterServiceImpl {
        node_repo: state.node_repo.clone(),
        peer_repo: state.peer_repo.clone(),
        probe_repo: state.probe_repo.clone(),
        community_repo: state.community_repo.clone(),
        settings_repo: state.settings_repo.clone(),
        jwt_secret: jwt_secret.clone(),
        cluster_key: Arc::new(cluster_key.clone()),
        listen_addr: listen_addr.clone(),
        aggregator: cluster_aggregator,
    };
```

- [ ] **Step 6: Build and verify**

Run: `cargo build`
Expected: compiles

- [ ] **Step 7: Commit**

```bash
git add src/grpc/cluster_service.rs src/main.rs
git commit -m "feat: wire ClusterAggregator fanout into list_probe_results and list_community_rules"
```

---

### Task 6: Add unit tests for auth JWT functions

**Files:**
- Modify: `src/auth.rs` (append test module)

- [ ] **Step 1: Add #[cfg(test)] module to src/auth.rs**

Append at the end of `src/auth.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tonic::Request;

    #[test]
    fn test_generate_jwt_secret_is_not_empty() {
        let secret = generate_jwt_secret();
        assert!(!secret.is_empty());
        assert!(secret.len() >= 32);
    }

    #[test]
    fn test_create_and_verify_token() {
        let secret = "test-secret-key-32-bytes-min";
        let token = create_token("admin", secret, 30).expect("should create token");
        let claims = verify_token(&token, secret).expect("should verify token");
        assert_eq!(claims.sub, "admin");
    }

    #[test]
    fn test_verify_token_wrong_secret() {
        let token = create_token("admin", "correct-secret-key-32", 30).unwrap();
        let result = verify_token(&token, "wrong-secret-key-32-xxx");
        assert!(result.is_err());
    }

    #[test]
    fn test_check_auth_no_authorization_header() {
        let req: Request<()> = Request::new(());
        let result = check_auth(&req, "secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_check_auth_valid_token() {
        let secret = "test-secret-key-32-bytes-min";
        let token = create_token("admin", secret, 300).unwrap();
        let mut req: Request<()> = Request::new(());
        req.metadata_mut().insert(
            "authorization",
            format!("Bearer {}", token).parse().unwrap(),
        );
        let result = check_auth(&req, secret);
        assert!(result.is_ok());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `source "$HOME/.cargo/env" && cargo test auth::tests`
Expected: 5 tests pass

- [ ] **Step 3: Commit**

```bash
git add src/auth.rs
git commit -m "test: add auth JWT unit tests"
```

---

### Task 7: Add unit tests for peer service auth rejection

**Files:**
- Modify: `src/grpc/peer_service.rs` (append test module)

- [ ] **Step 1: Add #[cfg(test)] module to src/grpc/peer_service.rs**

Append at the end of `src/grpc/peer_service.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    async fn setup_svc() -> PeerServiceImpl {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE peers (id TEXT PRIMARY KEY, name TEXT NOT NULL, asn INTEGER NOT NULL,
             wg_public_key TEXT NOT NULL DEFAULT '', wg_private_key TEXT NOT NULL DEFAULT '',
             wg_endpoint TEXT NOT NULL DEFAULT '', wg_persistent_keepalive INTEGER NOT NULL DEFAULT 0,
             wg_mtu INTEGER NOT NULL DEFAULT 0, wg_fwmark INTEGER NOT NULL DEFAULT 0,
             wg_post_up TEXT NOT NULL DEFAULT '', wg_post_down TEXT NOT NULL DEFAULT '',
             tunnel_ipv4 TEXT NOT NULL DEFAULT '', tunnel_ipv6 TEXT NOT NULL DEFAULT '',
             enabled INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL DEFAULT '',
             updated_at TEXT NOT NULL DEFAULT '', origin_node_id TEXT NOT NULL DEFAULT '')"
        ).execute(&pool).await.unwrap();
        sqlx::query(
            "CREATE TABLE settings (id INTEGER PRIMARY KEY CHECK (id = 1),
             local_asn INTEGER NOT NULL DEFAULT 0, wg_listen_port INTEGER NOT NULL DEFAULT 0,
             wg_private_key TEXT NOT NULL DEFAULT '', wg_public_key TEXT NOT NULL DEFAULT '',
             tunnel_ipv4_subnet TEXT NOT NULL DEFAULT '', tunnel_ipv6_subnet TEXT NOT NULL DEFAULT '',
             endpoint TEXT NOT NULL DEFAULT '', roa_mode TEXT NOT NULL DEFAULT 'none',
             roa_v4_url TEXT NOT NULL DEFAULT '', roa_v6_url TEXT NOT NULL DEFAULT '',
             bird_local_asn INTEGER NOT NULL DEFAULT 0, bird_router_id TEXT NOT NULL DEFAULT '',
             community_ipv4 TEXT NOT NULL DEFAULT '', community_ipv6 TEXT NOT NULL DEFAULT '',
             large_community TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL DEFAULT '',
             updated_at TEXT NOT NULL DEFAULT '')"
        ).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO settings (id) VALUES (1)").execute(&pool).await.unwrap();

        PeerServiceImpl {
            peer_repo: crate::models::peer::PeerRepository::new(pool.clone()),
            settings_repo: crate::models::settings::SettingsRepository::new(pool.clone()),
            jwt_secret: Arc::new("test-secret-key-32-bytes-min".into()),
            node_repo: crate::models::node::NodeRepository::new(pool.clone()),
            cluster_key: Arc::new(String::new()),
            listen_addr: "127.0.0.1:3000".into(),
        }
    }

    #[tokio::test]
    async fn test_get_wireguard_config_rejects_unauthenticated() {
        let svc = setup_svc().await;
        let req = Request::new(GetConfigRequest { id: "any".into() });
        let result = svc.get_wire_guard_config(req).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[tokio::test]
    async fn test_get_bird_config_rejects_unauthenticated() {
        let svc = setup_svc().await;
        let req = Request::new(GetConfigRequest { id: "any".into() });
        let result = svc.get_bird_config(req).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[tokio::test]
    async fn test_export_all_wireguard_rejects_unauthenticated() {
        let svc = setup_svc().await;
        let req = Request::new(ExportAllRequest {});
        let result = svc.export_all_wire_guard(req).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[tokio::test]
    async fn test_export_all_bird_rejects_unauthenticated() {
        let svc = setup_svc().await;
        let req = Request::new(ExportAllRequest {});
        let result = svc.export_all_bird(req).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `source "$HOME/.cargo/env" && cargo test grpc::peer_service::tests`
Expected: 4 tests pass

- [ ] **Step 3: Commit**

```bash
git add src/grpc/peer_service.rs
git commit -m "test: add peer service auth rejection tests"
```

---

### Task 8: Add unit tests for cluster key auth

**Files:**
- Modify: `src/cluster/auth.rs` (append test module)

- [ ] **Step 1: Add #[cfg(test)] module to src/cluster/auth.rs**

Append at the end of `src/cluster/auth.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tonic::Request;

    #[test]
    fn test_check_cluster_key_valid() {
        let mut req: Request<()> = Request::new(());
        req.metadata_mut()
            .insert("x-cluster-key", "my-shared-secret".parse().unwrap());
        assert!(check_cluster_key(&req, "my-shared-secret").is_ok());
    }

    #[test]
    fn test_check_cluster_key_wrong() {
        let mut req: Request<()> = Request::new(());
        req.metadata_mut()
            .insert("x-cluster-key", "wrong-key".parse().unwrap());
        assert!(check_cluster_key(&req, "my-shared-secret").is_err());
    }

    #[test]
    fn test_check_cluster_key_missing() {
        let req: Request<()> = Request::new(());
        assert!(check_cluster_key(&req, "my-shared-secret").is_err());
    }

    #[test]
    fn test_check_cluster_key_empty_config_allows_all() {
        let mut req: Request<()> = Request::new(());
        req.metadata_mut()
            .insert("x-cluster-key", "anything".parse().unwrap());
        assert!(check_cluster_key(&req, "").is_ok());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `source "$HOME/.cargo/env" && cargo test cluster::auth::tests`
Expected: 4 tests pass

- [ ] **Step 3: Commit**

```bash
git add src/cluster/auth.rs
git commit -m "test: add cluster key auth unit tests"
```

---

### Task 9: Make system paths configurable

**Files:**
- Modify: `src/config.rs:39-42` (StorageConfig)
- Modify: `src/services/bird_socket.rs:7` (SOCKET_PATH const)
- Modify: `src/main.rs:397-407` (path usage)
- Modify: `src/grpc/peer_service.rs:77-78` (hardcoded WG paths)
- Modify: `src/services/bird.rs:274-275` (hardcoded bird paths)
- Modify: `src/cluster/tunnel.rs:84` (hardcoded cluster WG path)

- [ ] **Step 1: Add fields to StorageConfig in config.rs**

Add to `StorageConfig` struct:
```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct StorageConfig {
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_wg_config_path")]
    pub wg_config_path: String,
    #[serde(default = "default_bird_config_path")]
    pub bird_config_path: String,
    #[serde(default = "default_bird_socket_path")]
    pub bird_socket_path: String,
}
```

Add default functions:
```rust
fn default_wg_config_path() -> String {
    "/etc/wireguard/wg0.conf".into()
}
fn default_bird_config_path() -> String {
    "/etc/bird/bird.conf".into()
}
fn default_bird_socket_path() -> String {
    "/var/run/bird.ctl".into()
}
```

Update `Default` impl for `StorageConfig`:
```rust
impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            db_path: default_db_path(),
            wg_config_path: default_wg_config_path(),
            bird_config_path: default_bird_config_path(),
            bird_socket_path: default_bird_socket_path(),
        }
    }
}
```

- [ ] **Step 2: Update BirdSocket to accept path as parameter (src/services/bird_socket.rs)**

Old:
```rust
const SOCKET_PATH: &str = "/var/run/bird.ctl";

pub struct BirdSocket {
    reader: BufReader<tokio::io::ReadHalf<UnixStream>>,
    writer: tokio::io::WriteHalf<UnixStream>,
}
```

New — add a `connect_with_path` method alongside existing `connect`:
```rust
const DEFAULT_SOCKET_PATH: &str = "/var/run/bird.ctl";

pub struct BirdSocket {
    reader: BufReader<tokio::io::ReadHalf<UnixStream>>,
    writer: tokio::io::WriteHalf<UnixStream>,
}

impl BirdSocket {
    /// Connect using the default socket path (backward compat)
    pub async fn connect() -> Result<Self, AppError> {
        Self::connect_at(DEFAULT_SOCKET_PATH).await
    }

    /// Connect using a custom socket path
    pub async fn connect_at(path: &str) -> Result<Self, AppError> {
        let stream = UnixStream::connect(Path::new(path)).await?;
        let (reader, mut writer) = tokio::io::split(stream);
        // ... rest of existing connect logic
    }
}
```

- [ ] **Step 3: Update peer_service.rs to use configurable WG path**

In `auto_apply_wg_bird` (lines 77-78), replace:
```rust
let conf_path = "/etc/wireguard/wg0.conf";
let tmp_path = "/etc/wireguard/wg0.conf.tmp";
```
With:
```rust
let config = crate::APP_CONFIG.get().expect("APP_CONFIG not initialized");
let conf_path = &config.storage.wg_config_path;
let tmp_path = format!("{conf_path}.tmp");
let tmp_path = tmp_path.as_str();
```

- [ ] **Step 4: Update services/bird.rs to use configurable path**

In `apply_config` (lines 274-275), replace:
```rust
let config_path = "/etc/bird/bird.conf";
let tmp_path = "/etc/bird/bird.conf.tmp";
```
With:
```rust
let config = crate::APP_CONFIG.get().expect("APP_CONFIG not initialized");
let config_path = config.storage.bird_config_path.as_str();
let tmp_path = format!("{config_path}.tmp");
```

- [ ] **Step 5: Update cluster/tunnel.rs to use configurable path**

Line 84:
```rust
let config_path = format!("/etc/wireguard/{CLUSTER_WG_INTERFACE}.conf");
```
Replace with:
```rust
let config = crate::APP_CONFIG.get().expect("APP_CONFIG not initialized");
let base = std::path::Path::new(&config.storage.wg_config_path);
let dir = base.parent().unwrap_or(std::path::Path::new("/etc/wireguard"));
let config_path = format!("{}/{}.conf", dir.display(), CLUSTER_WG_INTERFACE);
```

- [ ] **Step 6: Build and verify**

Run: `cargo build`
Expected: compiles

- [ ] **Step 7: Run existing unit tests**

Run: `source "$HOME/.cargo/env" && cargo test`
Expected: all 34 existing tests pass

- [ ] **Step 8: Commit**

```bash
git add src/config.rs src/services/bird_socket.rs src/services/bird.rs src/grpc/peer_service.rs src/cluster/tunnel.rs
git commit -m "feat: make WG/bird config and socket paths configurable via StorageConfig"
```

---

### Task 10: Fix deprecated chrono date_naive() API

**Files:**
- Modify: `src/models/flap_event.rs:138-142`

- [ ] **Step 1: Replace deprecated date_naive() usage**

Old:
```rust
        let midnight = chrono::Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .and_then(|t| t.and_local_timezone(chrono::Utc).latest())
            .unwrap_or_else(chrono::Utc::now);
        let hours_since_midnight = (chrono::Utc::now() - midnight).num_minutes() as f64 / 60.0;
```

New:
```rust
        let now = chrono::Utc::now();
        let today_str = now.format("%Y-%m-%d").to_string();
        let midnight_str = format!("{today_str}T00:00:00+00:00");
        let midnight = chrono::DateTime::parse_from_rfc3339(&midnight_str)
            .map(|t| t.with_timezone(&chrono::Utc))
            .unwrap_or(now);
        let hours_since_midnight = (now - midnight).num_minutes() as f64 / 60.0;
```

- [ ] **Step 2: Build and verify**

Run: `cargo build`
Expected: compiles without deprecated warnings for this file

- [ ] **Step 3: Run existing tests**

Run: `source "$HOME/.cargo/env" && cargo test`
Expected: all 34 existing tests pass

- [ ] **Step 4: Commit**

```bash
git add src/models/flap_event.rs
git commit -m "fix: replace deprecated chrono date_naive() with RFC3339 parsing"
```

---

## Execution Order

Groups A, B, C are independent. Within each group, tasks are sequential.

```
Group A: Task 1 → Task 2 → Task 3
Group B: Task 4 → Task 5
Group C: Task 6 → Task 7 → Task 8 → Task 9 → Task 10
```

All three groups can run in parallel.
