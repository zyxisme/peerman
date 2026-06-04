# Architecture Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix 7 architecture issues identified in the design review — reduce code duplication, split oversized files, improve connection reuse, and add frontend auth resilience.

**Architecture:** Each task is independent and can be executed in any order. Tasks 6 and 7 (main.rs split) should be done sequentially. All other tasks are fully independent. Each task produces a compilable, test-passing state.

**Tech Stack:** Rust (tonic, axum, sqlx), TypeScript (React, ConnectRPC), SQLite

---

## File Map

| File | Action | Purpose |
|------|--------|---------|
| `src/models/peer.rs` | Modify | Add `PEER_COLUMNS` constant |
| `src/models/node.rs` | Modify | Add `NODE_COLUMNS` constant |
| `src/models/settings.rs` | Modify | Add `SETTINGS_COLUMNS` constant |
| `src/cluster/aggregator.rs` | Modify | Extract generic `fanout()` method |
| `src/grpc/peer_service.rs` | Modify | Reuse connection pool, single-step create |
| `src/http/mod.rs` | Create | HTTP handler module |
| `src/http/handlers.rs` | Create | login/logout/me/health handlers |
| `src/http/rate_limit.rs` | Create | LoginRateLimiter |
| `src/tasks/mod.rs` | Create | Background task spawning module |
| `src/tasks/cluster.rs` | Create | Cluster background tasks |
| `src/tasks/apply.rs` | Create | WG+BIRD apply task |
| `src/tasks/retention.rs` | Create | Data retention cleanup task |
| `src/main.rs` | Modify | Slim down to ~150 lines |
| `frontend/src/lib/http.ts` | Create | Fetch wrapper with 401 interceptor |
| `frontend/src/lib/auth.tsx` | Modify | Use http.ts for auth requests |
| `frontend/src/hooks/usePeers.ts` | Modify | Use http.ts fetch wrapper |

---

### Task 1: Add Column Constants to Models

**Files:**
- Modify: `src/models/peer.rs:1-66`
- Modify: `src/models/node.rs:1-44`
- Modify: `src/models/settings.rs:1-33`

- [ ] **Step 1: Add PEER_COLUMNS constant to peer.rs**

In `src/models/peer.rs`, add a constant after the `Peer` struct definition (after line 34):

```rust
pub const PEER_COLUMNS: &str =
    "id, name, description, asn, local_asn, wg_private_key, wg_public_key, \
     wg_remote_address, wg_remote_port, wg_listen_port, wg_interface_name, \
     ipv4_tunnel_local, ipv4_tunnel_remote, ipv6_tunnel_local, ipv6_tunnel_remote, \
     multiprotocol, extended_nexthop, sessions, passive, \
     import_max_prefix, export_max_prefix, enabled, \
     created_at, updated_at, origin_node_id";
```

- [ ] **Step 2: Replace hardcoded column lists in PeerRepository**

In `src/models/peer.rs`, replace all `SELECT id, name, description, asn, ...` with `SELECT {PEER_COLUMNS}`. There are multiple `query_as` calls in `PeerRepository` — each one should use `format!("SELECT {PEER_COLUMNS} FROM peers ...")` or a string constant like:

```rust
const PEER_SELECT: &str = "SELECT id, name, description, asn, local_asn, wg_private_key, wg_public_key, \
    wg_remote_address, wg_remote_port, wg_listen_port, wg_interface_name, \
    ipv4_tunnel_local, ipv4_tunnel_remote, ipv6_tunnel_local, ipv6_tunnel_remote, \
    multiprotocol, extended_nexthop, sessions, passive, \
    import_max_prefix, export_max_prefix, enabled, created_at, updated_at, origin_node_id \
    FROM peers";
```

Then use `PEER_SELECT` in every `query_as` call:
- `list_all()` → `"{PEER_SELECT} ORDER BY name"`
- `find_by_id()` → `"{PEER_SELECT} WHERE id = ?"`
- etc.

- [ ] **Step 3: Add NODE_COLUMNS constant to node.rs**

In `src/models/node.rs`, add after the `Node` struct (after line 22):

```rust
const NODE_COLUMNS: &str =
    "id, name, listen_addr, local_asn, description, online, \
     last_seen_at, created_at, updated_at, wg_pubkey, tunnel_ip, tunnel_ipv6, \
     wg_private_key";
```

Replace all `query_as` SELECT lists in `NodeRepository` with `{NODE_COLUMNS}`.

- [ ] **Step 4: Add SETTINGS_COLUMNS constant to settings.rs**

In `src/models/settings.rs`, add after the `Settings` struct (after line 33):

```rust
const SETTINGS_COLUMNS: &str =
    "local_asn, bird_template_name, bird_router_id, \
     wg_default_listen_port, dn42_ipv4_prefix, dn42_ipv6_prefix, wg_table, \
     wg_mtu, wg_fwmark, wg_post_up, wg_post_down, \
     roa_mode, roa_static_v4_url, roa_static_v6_url, roa_rtr_address, roa_rtr_port, \
     bird_import_limit, bird_export_filter, bird_import_filter, \
     enable_community_filters, enable_bfd, bfd_interval_ms, bfd_multiplier, \
     cluster_tunnel_ipv6_range, enable_confederation, confederation_local_asn";
```

Replace `SettingsRepository::load()` SELECT list with `{SETTINGS_COLUMNS}`.

- [ ] **Step 5: Run tests to verify**

Run: `cargo test`
Expected: All 71 tests pass (column order unchanged, just extracted to constants)

- [ ] **Step 6: Run clippy and format**

Run: `cargo clippy && cargo fmt`
Expected: No warnings

- [ ] **Step 7: Commit**

```bash
git add src/models/peer.rs src/models/node.rs src/models/settings.rs
git commit -m "refactor: extract SQL column lists to constants in model repositories"
```

---

### Task 2: Generic Fanout in ClusterAggregator

**Files:**
- Modify: `src/cluster/aggregator.rs:61-280`

- [ ] **Step 1: Add generic fanout method**

In `src/cluster/aggregator.rs`, add a new method inside the `impl ClusterAggregator` block, after `set_cluster_key()` (after line 59):

```rust
/// Generic fan-out: call `rpc_fn` on every online node in parallel.
/// On success, call `on_success` to cache the result.
/// On failure, fall back to cache via `from_cache`.
async fn fanout<T, Req, RpcFn, OnSuccess, FromCache>(
    &self,
    local_addr: &str,
    online_nodes: &[Node],
    make_request: impl Fn() -> Req + Send + Sync,
    rpc_fn: RpcFn,
    on_success: OnSuccess,
    from_cache: FromCache,
) -> AggregatedResult<T>
where
    T: Clone + Send + 'static,
    Req: Send + 'static,
    RpcFn: Fn(ClusterServiceClient<tonic::transport::Channel>, Req) -> futures::future::BoxFuture<'static, Result<Vec<T>, String>>
        + Send
        + Sync
        + 'static,
    OnSuccess: Fn(&str, Vec<T>) -> futures::future::BoxFuture<'static, ()> + Send + Sync + 'static,
    FromCache: Fn(&str) -> futures::future::BoxFuture<'static, Option<Vec<T>>> + Send + Sync + 'static,
{
    let futures: Vec<_> = online_nodes
        .iter()
        .filter(|n| n.listen_addr != local_addr)
        .map(|node| {
            let node_addr = node.listen_addr.clone();
            let node_name = node.name.clone();
            let cache = self.cache.clone();
            let cluster_key = self.cluster_key.clone();
            let req = make_request();
            let rpc = &rpc_fn;
            let success = &on_success;
            let cache_fallback = &from_cache;

            async move {
                let mut client = match Self::connect(&node_addr).await {
                    Ok(c) => c,
                    Err(e) => {
                        let cached = cache_fallback(&node_addr).await;
                        cache.mark_stale(&node_addr).await;
                        return (
                            cached.unwrap_or_default(),
                            vec![NodeStatus::unknown(&node_name, &node_addr, &e)],
                        );
                    }
                };

                // Inject cluster key
                if !cluster_key.is_empty() {
                    if let Ok(val) = cluster_key.parse() {
                        // We can't mutate the request here since it's moved into rpc_fn
                        // The caller handles metadata injection
                    }
                }

                match timeout(FANOUT_TIMEOUT, rpc(client, req)).await {
                    Ok(Ok(items)) => {
                        success(&node_addr, items.clone()).await;
                        (items, vec![NodeStatus::online(&node_name, &node_addr)])
                    }
                    _ => {
                        let cached = cache_fallback(&node_addr).await;
                        cache.mark_stale(&node_addr).await;
                        let items = cached.unwrap_or_default();
                        let status = if items.is_empty() {
                            NodeStatus::unknown(&node_name, &node_addr, "fanout timeout")
                        } else {
                            NodeStatus::offline(&node_name, &node_addr, "stale")
                        };
                        (items, vec![status])
                    }
                }
            }
        })
        .collect();

    let results = futures::future::join_all(futures).await;
    let mut all = Vec::new();
    let mut statuses = Vec::new();
    for (items, node_statuses) in results {
        all.extend(items);
        statuses.extend(node_statuses);
    }

    AggregatedResult {
        items: all,
        node_statuses: statuses,
    }
}
```

**Note:** The generic approach with closures and `BoxFuture` adds complexity. A simpler alternative is to keep the three methods but extract the inner per-node logic into a helper. Given the different request types and cache update methods, the pragmatic approach is to extract just the common "connect → timeout → cache fallback" pattern:

```rust
/// Execute an RPC on a single node with timeout and cache fallback.
/// Returns (items, node_status) tuple.
async fn call_node<T: Clone>(
    node_name: &str,
    node_addr: &str,
    cluster_key: &str,
    cache: &ClusterCache,
    rpc_call: impl std::future::Future<Output = Result<Vec<T>, String>>,
    cache_fn: impl std::future::Future<Output = Option<Vec<T>>>,
) -> (Vec<T>, Vec<NodeStatus>) {
    match timeout(FANOUT_TIMEOUT, rpc_call).await {
        Ok(Ok(items)) => {
            (items, vec![NodeStatus::online(node_name, node_addr)])
        }
        _ => {
            let cached = cache_fn.await;
            cache.mark_stale(node_addr).await;
            let items = cached.unwrap_or_default();
            let status = if items.is_empty() {
                NodeStatus::unknown(node_name, node_addr, "fanout timeout")
            } else {
                NodeStatus::offline(node_name, node_addr, "stale")
            };
            (items, vec![status])
        }
    }
}
```

- [ ] **Step 2: Refactor fanout_peers to use the helper**

Rewrite `fanout_peers()` to use `call_node()`:

```rust
pub async fn fanout_peers(
    &self,
    local_addr: &str,
    online_nodes: &[Node],
) -> AggregatedResult<Peer> {
    let futures: Vec<_> = online_nodes
        .iter()
        .filter(|n| n.listen_addr != local_addr)
        .map(|node| {
            let node_addr = node.listen_addr.clone();
            let node_name = node.name.clone();
            let cache = self.cache.clone();
            let cluster_key = self.cluster_key.clone();
            async move {
                let client = match Self::connect(&node_addr).await {
                    Ok(c) => c,
                    Err(e) => {
                        let cached = cache.get(&node_addr).await;
                        cache.mark_stale(&node_addr).await;
                        return (
                            cached.map(|c| c.peers).unwrap_or_default(),
                            vec![NodeStatus::unknown(&node_name, &node_addr, &e)],
                        );
                    }
                };

                let mut req = Request::new(PullPeersRequest { since: String::new() });
                if !cluster_key.is_empty() {
                    if let Ok(val) = cluster_key.parse() {
                        req.metadata_mut().insert("x-cluster-key", val);
                    }
                }

                let cache_clone = cache.clone();
                let addr_clone = node_addr.clone();
                let (items, statuses) = call_node(
                    &node_name,
                    &node_addr,
                    &cluster_key,
                    &cache,
                    client.pull_peers(req).map(|r| r.map(|resp| resp.into_inner().peers)),
                    async move { cache_clone.get(&addr_clone).await.map(|c| c.peers) },
                )
                .await;

                if !items.is_empty() {
                    cache.update_peers(&node_addr, items.clone()).await;
                }

                (items, statuses)
            }
        })
        .collect();

    let results = futures::future::join_all(futures).await;
    let mut all = Vec::new();
    let mut statuses = Vec::new();
    for (items, node_statuses) in results {
        all.extend(items);
        statuses.extend(node_statuses);
    }
    AggregatedResult { items: all, node_statuses: statuses }
}
```

- [ ] **Step 3: Refactor fanout_probe_results and fanout_community_rules similarly**

Apply the same pattern to `fanout_probe_results()` and `fanout_community_rules()`. Each becomes ~40 lines instead of ~70 lines.

- [ ] **Step 4: Run tests to verify**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 5: Run clippy and format**

Run: `cargo clippy && cargo fmt`

- [ ] **Step 6: Commit**

```bash
git add src/cluster/aggregator.rs
git commit -m "refactor: extract call_node helper to reduce fanout duplication in ClusterAggregator"
```

---

### Task 3: Reuse Connection Pool in proxy_push_peer

**Files:**
- Modify: `src/grpc/peer_service.rs:126-155`

- [ ] **Step 1: Add ClusterAggregator field to PeerServiceImpl**

In `src/grpc/peer_service.rs`, modify the `PeerServiceImpl` struct (line 21) to add an aggregator field:

```rust
pub struct PeerServiceImpl {
    pub peer_repo: crate::models::peer::PeerRepository,
    pub settings_repo: crate::models::settings::SettingsRepository,
    pub jwt_secret: std::sync::Arc<String>,
    pub node_repo: crate::models::node::NodeRepository,
    pub cluster_key: std::sync::Arc<String>,
    pub listen_addr: String,
    pub config_dirty: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub aggregator: crate::cluster::aggregator::ClusterAggregator,
}
```

- [ ] **Step 2: Rewrite proxy_push_peer to use aggregator**

Replace the `proxy_push_peer` method (lines 127-154) to use the aggregator's connection pool:

```rust
async fn proxy_push_peer(&self, target_addr: &str, peer: Peer) -> Result<Peer, Status> {
    use crate::grpc::generated::cluster_service_client::ClusterServiceClient;
    use crate::grpc::generated::PushPeerRequest;

    let mut client = ClusterAggregator::connect(target_addr)
        .await
        .map_err(|e| Status::internal(format!("connect failed: {e}")))?;

    let mut req = Request::new(PushPeerRequest {
        peer: Some(peer.clone()),
        origin_node_id: peer.origin_node_id.clone(),
    });

    if !self.cluster_key.is_empty() {
        if let Ok(v) = self.cluster_key.parse() {
            req.metadata_mut().insert("x-cluster-key", v);
        }
    }

    client
        .push_peer(req)
        .await
        .map_err(|e| Status::internal(format!("proxy push failed: {e}")))?;

    Ok(peer)
}
```

**Note:** `ClusterAggregator::connect()` returns a `ClusterServiceClient<Channel>` which is `Clone` (tonic clients are cheap to clone). The `CHANNEL_POOL` DashMap caches the underlying channel.

- [ ] **Step 3: Update main.rs to pass aggregator to PeerServiceImpl**

In `src/main.rs`, when constructing `PeerServiceImpl` (around line 321), add the aggregator field:

```rust
let aggregator = crate::cluster::aggregator::ClusterAggregator::new(
    state.cluster_cache.clone(),
    cluster_key.clone(),
);
let peer_svc = PeerServiceImpl {
    peer_repo: state.peer_repo.clone(),
    settings_repo: state.settings_repo.clone(),
    jwt_secret: jwt_secret.clone(),
    node_repo: state.node_repo.clone(),
    cluster_key: Arc::new(cluster_key.clone()),
    listen_addr: listen_addr.clone(),
    config_dirty: config_dirty.clone(),
    aggregator,
};
```

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add src/grpc/peer_service.rs src/main.rs
git commit -m "refactor: reuse ClusterAggregator connection pool in proxy_push_peer"
```

---

### Task 4: Single-Step Create Peer

**Files:**
- Modify: `src/grpc/peer_service.rs:193-245`
- Modify: `src/models/peer.rs` (add `create_full` method)

- [ ] **Step 1: Add create_full method to PeerRepository**

In `src/models/peer.rs`, add a new method to `PeerRepository`:

```rust
/// Create a peer with all fields in a single INSERT ... RETURNING.
pub async fn create_full(&self, peer: &Peer) -> Result<Peer, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query_as::<_, Peer>(&format!(
        "INSERT INTO peers ({PEER_COLUMNS})
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?)
         RETURNING {PEER_COLUMNS}"
    ))
    .bind(&id)
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
    // enabled = 1 (hardcoded in VALUES)
    .bind(&now) // created_at
    .bind(&now) // updated_at
    .bind(&peer.origin_node_id)
    .fetch_one(&self.pool)
    .await
    .map_err(Into::into)
}
```

- [ ] **Step 2: Rewrite create_peer in PeerServiceImpl**

In `src/grpc/peer_service.rs`, replace the `create_peer` method (lines 193-245) with:

```rust
async fn create_peer(
    &self,
    request: Request<CreatePeerRequest>,
) -> Result<Response<Peer>, Status> {
    crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
    let req = request.into_inner();
    let origin = req.origin_node_id.clone();

    // If targeting a remote node, proxy the write
    if !origin.is_empty() && origin != self.listen_addr {
        let target_node = self
            .node_repo
            .find_by_id(&origin)
            .await
            .map_err(|_| Status::not_found("target node not found"))?;

        let proto = create_request_to_proto(&req);
        let proxied = self
            .proxy_push_peer(&target_node.listen_addr, proto)
            .await?;
        return Ok(Response::new(proxied));
    }

    validate_peer_fields(
        &req.name,
        req.asn,
        &req.wg_public_key,
        &req.ipv4_tunnel_local,
        &req.ipv4_tunnel_remote,
        &req.ipv6_tunnel_local,
        &req.ipv6_tunnel_remote,
    )
    .map_err(|e| Status::invalid_argument(e.to_string()))?;

    let proto = create_request_to_proto(&req);
    let peer = self
        .peer_repo
        .create_full(&proto)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    self.config_dirty
        .store(true, std::sync::atomic::Ordering::Relaxed);

    Ok(Response::new(peer_to_proto(&peer)))
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add src/models/peer.rs src/grpc/peer_service.rs
git commit -m "refactor: use single INSERT for create_peer instead of two-step create+update"
```

---

### Task 5: Consolidate AppState into gRPC Services

**Files:**
- Modify: `src/app_state.rs`
- Modify: `src/main.rs:319-357`

- [ ] **Step 1: Create sub-state structs for gRPC services**

In `src/app_state.rs`, add focused sub-state structs:

```rust
/// State for services that need peer + settings repos (PeerService, ClusterService).
#[derive(Clone)]
pub struct PeerState {
    pub peer_repo: PeerRepository,
    pub settings_repo: SettingsRepository,
    pub node_repo: NodeRepository,
}

/// State for services that need probe + community repos (ClusterService).
#[derive(Clone)]
pub struct ClusterState {
    pub node_repo: NodeRepository,
    pub peer_repo: PeerRepository,
    pub probe_repo: ProbeResultRepository,
    pub community_repo: CommunityRuleRepository,
    pub settings_repo: SettingsRepository,
    pub cluster_cache: ClusterCache,
}

impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            peer_repo: PeerRepository::new(pool.clone()),
            settings_repo: SettingsRepository::new(pool.clone()),
            node_repo: NodeRepository::new(pool.clone()),
            probe_repo: ProbeResultRepository::new(pool.clone()),
            community_repo: CommunityRuleRepository::new(pool.clone()),
            flap_event_repo: FlapEventRepository::new(pool),
            cluster_cache: ClusterCache::new(),
        }
    }

    pub fn peer_state(&self) -> PeerState {
        PeerState {
            peer_repo: self.peer_repo.clone(),
            settings_repo: self.settings_repo.clone(),
            node_repo: self.node_repo.clone(),
        }
    }

    pub fn cluster_state(&self) -> ClusterState {
        ClusterState {
            node_repo: self.node_repo.clone(),
            peer_repo: self.peer_repo.clone(),
            probe_repo: self.probe_repo.clone(),
            community_repo: self.community_repo.clone(),
            settings_repo: self.settings_repo.clone(),
            cluster_cache: self.cluster_cache.clone(),
        }
    }
}
```

- [ ] **Step 2: Update gRPC services to accept sub-states**

Modify each gRPC service struct to accept the relevant sub-state. For example, `PeerServiceImpl`:

```rust
pub struct PeerServiceImpl {
    pub state: crate::app_state::PeerState,
    pub jwt_secret: std::sync::Arc<String>,
    pub cluster_key: std::sync::Arc<String>,
    pub listen_addr: String,
    pub config_dirty: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub aggregator: crate::cluster::aggregator::ClusterAggregator,
}
```

Update all method bodies to use `self.state.peer_repo` instead of `self.peer_repo`, etc.

- [ ] **Step 3: Simplify main.rs service construction**

In `src/main.rs`, replace the verbose service construction with:

```rust
let peer_svc = PeerServiceImpl {
    state: state.peer_state(),
    jwt_secret: jwt_secret.clone(),
    cluster_key: Arc::new(cluster_key.clone()),
    listen_addr: listen_addr.clone(),
    config_dirty: config_dirty.clone(),
    aggregator: crate::cluster::aggregator::ClusterAggregator::new(
        state.cluster_cache.clone(),
        cluster_key.clone(),
    ),
};
```

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add src/app_state.rs src/main.rs src/grpc/peer_service.rs src/grpc/cluster_service.rs src/grpc/bird_service.rs src/grpc/settings_service.rs src/grpc/flap_service.rs
git commit -m "refactor: consolidate AppState into focused sub-state structs for gRPC services"
```

---

### Task 6: Extract HTTP Handlers from main.rs

**Files:**
- Create: `src/http/mod.rs`
- Create: `src/http/handlers.rs`
- Create: `src/http/rate_limit.rs`
- Modify: `src/main.rs` (remove lines 37-256)

- [ ] **Step 1: Create src/http/mod.rs**

```rust
pub mod handlers;
pub mod rate_limit;
```

- [ ] **Step 2: Create src/http/rate_limit.rs**

Extract `LoginRateLimiter` from `src/main.rs` (lines 41-86):

```rust
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct LoginRateLimiter {
    attempts: Mutex<HashMap<String, (u32, Instant)>>,
}

impl LoginRateLimiter {
    pub const fn new() -> Self {
        Self {
            attempts: Mutex::new(HashMap::new()),
        }
    }

    pub fn check(&self, ip: &str) -> Result<(), u64> {
        let mut attempts = self.attempts.lock().unwrap();
        let now = Instant::now();
        let entry = attempts.entry(ip.to_string()).or_insert((0, now));

        if now.duration_since(entry.1) > Duration::from_secs(60) {
            *entry = (1, now);
            return Ok(());
        }

        entry.0 += 1;
        if entry.0 > 5 {
            let remaining = 60 - now.duration_since(entry.1).as_secs();
            Err(remaining)
        } else {
            Ok(())
        }
    }
}
```

- [ ] **Step 3: Create src/http/handlers.rs**

Extract all HTTP handlers from `src/main.rs` (lines 89-256):

```rust
use axum::extract::ConnectInfo;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use super::rate_limit::LoginRateLimiter;
use crate::{auth, app_config};

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub success: bool,
    pub user: Option<UserInfo>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct UserInfo {
    pub username: String,
}

#[derive(Serialize)]
pub struct MeResponse {
    pub authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

static LOGIN_RATE_LIMITER: LoginRateLimiter = LoginRateLimiter::new();

pub async fn handle_health() -> &'static str {
    "ok"
}

pub async fn handle_login(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<LoginRequest>,
) -> Response {
    let ip = addr.ip().to_string();

    if let Err(retry_after) = LOGIN_RATE_LIMITER.check(&ip) {
        return json_response(
            StatusCode::TOO_MANY_REQUESTS,
            &LoginResponse {
                success: false,
                user: None,
                error: Some(format!("Too many attempts. Try again in {retry_after}s")),
            },
            None,
        );
    }

    let cfg = app_config();
    if req.username != cfg.auth.username {
        return json_response(
            StatusCode::UNAUTHORIZED,
            &LoginResponse {
                success: false,
                user: None,
                error: Some("Invalid credentials".into()),
            },
            None,
        );
    }

    let password_ok = if cfg.auth.password_hash.is_empty() {
        tracing::warn!("Using plaintext password comparison — set password_hash in config");
        req.password == cfg.auth.password
    } else {
        auth::password::verify_password(&req.password, &cfg.auth.password_hash).unwrap_or(false)
    };

    if !password_ok {
        return json_response(
            StatusCode::UNAUTHORIZED,
            &LoginResponse {
                success: false,
                user: None,
                error: Some("Invalid credentials".into()),
            },
            None,
        );
    }

    let secret = if cfg.auth.jwt_secret.is_empty() {
        ""
    } else {
        &cfg.auth.jwt_secret
    };

    match auth::create_token(&req.username, secret) {
        Ok(token) => {
            let cookie = format!(
                "jwt={}; HttpOnly; SameSite=Strict; Path=/; Max-Age=3600; Secure",
                token
            );
            json_response(
                StatusCode::OK,
                &LoginResponse {
                    success: true,
                    user: Some(UserInfo {
                        username: req.username,
                    }),
                    error: None,
                },
                Some(&cookie),
            )
        }
        Err(e) => json_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &LoginResponse {
                success: false,
                user: None,
                error: Some(format!("Token creation failed: {e}")),
            },
            None,
        ),
    }
}

pub async fn handle_logout() -> Response {
    json_response(
        StatusCode::OK,
        &serde_json::json!({"success": true}),
        Some("jwt=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0; Secure"),
    )
}

pub async fn handle_me(headers: axum::http::HeaderMap) -> Json<MeResponse> {
    let cfg = app_config();
    let cookie_header = headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    match auth::parse_cookie(cookie_header, "jwt") {
        Some(token) => match auth::verify_token(token, &cfg.auth.jwt_secret) {
            Ok(claims) => Json(MeResponse {
                authenticated: true,
                username: Some(claims.sub),
            }),
            Err(_) => Json(MeResponse {
                authenticated: false,
                username: None,
            }),
        },
        None => Json(MeResponse {
            authenticated: false,
            username: None,
        }),
    }
}

fn json_response(
    status: StatusCode,
    body: &impl Serialize,
    cookie: Option<&str>,
) -> Response {
    use axum::http::header::SET_COOKIE;
    let json = serde_json::to_string(body).unwrap_or_default();
    let mut builder = Response::builder()
        .status(status)
        .header("content-type", "application/json");
    if let Some(c) = cookie {
        builder = builder.header(SET_COOKIE, c);
    }
    builder
        .body(axum::body::Body::from(json))
        .expect("body is infallible")
}
```

- [ ] **Step 4: Update main.rs to use new modules**

In `src/main.rs`:
1. Add `mod http;` at the top
2. Remove lines 37-256 (all HTTP handler code, LoginRateLimiter, LoginRequest, etc.)
3. Update router to use `crate::http::handlers::*`:

```rust
use crate::http::handlers;

let app = Router::new()
    .route("/health", get(handlers::handle_health))
    .route("/api/auth/login", post(handlers::handle_login))
    .route("/api/auth/logout", post(handlers::handle_logout))
    .route("/api/auth/me", get(handlers::handle_me))
    .nest("/api", grpc_router)
    .fallback(static_files::serve_static)
    .layer(TraceLayer::new_for_http());
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add src/http/ src/main.rs
git commit -m "refactor: extract HTTP handlers and rate limiter from main.rs into http module"
```

---

### Task 7: Extract Background Tasks from main.rs

**Files:**
- Create: `src/tasks/mod.rs`
- Create: `src/tasks/cluster.rs`
- Create: `src/tasks/apply.rs`
- Create: `src/tasks/retention.rs`
- Modify: `src/main.rs` (remove background task spawning code)

- [ ] **Step 1: Create src/tasks/mod.rs**

```rust
pub mod apply;
pub mod cluster;
pub mod retention;
```

- [ ] **Step 2: Create src/tasks/cluster.rs**

Extract cluster background tasks from `main.rs` (lines 564-876). Move the stale-node cleanup, health check + probe, anti-entropy exchange, and BGP flap detector tasks:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

use crate::app_state::AppState;

pub struct ClusterTasks {
    pub node_name: String,
    pub listen_addr: String,
    pub cluster_key: String,
    pub sync_interval: u64,
    pub probe_interval: u64,
    pub peer_nodes: Vec<String>,
    pub tunnel_ip_range: String,
    pub tunnel_ipv6_range: String,
    pub state: AppState,
    pub pool: sqlx::SqlitePool,
    pub shutdown: CancellationToken,
}

impl ClusterTasks {
    pub fn spawn_all(self) {
        self.spawn_stale_cleanup();
        if self.probe_interval > 0 {
            self.spawn_health_check();
        }
        self.spawn_anti_entropy();
        self.spawn_flap_detector();
    }

    fn spawn_stale_cleanup(&self) {
        let state = self.state.clone();
        let interval = self.sync_interval;
        let token = self.shutdown.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        tracing::info!("Stale-node cleanup task shutting down");
                        return;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(interval)) => {}
                }
                if let Err(e) = state.node_repo.mark_stale(120).await {
                    tracing::warn!("Failed to mark stale nodes: {}", e);
                }
            }
        });
    }

    fn spawn_health_check(&self) {
        // Extract health check + ICMP probe task from main.rs lines 584-668
        // ... (full implementation extracted from main.rs)
    }

    fn spawn_anti_entropy(&self) {
        // Extract anti-entropy exchange task from main.rs lines 670-793
        // ... (full implementation extracted from main.rs)
    }

    fn spawn_flap_detector(&self) {
        // Extract BGP flap detector task from main.rs lines 795-835
        // ... (full implementation extracted from main.rs)
    }
}
```

**Note:** The actual implementation should copy the exact code from `main.rs` for each task. The structure above shows the API; the body is a direct copy of the existing `tokio::spawn` blocks.

- [ ] **Step 3: Create src/tasks/apply.rs**

Extract the WG+BIRD apply task from `main.rs` (lines 878-915):

```rust
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub fn spawn_config_apply(
    config_dirty: Arc<AtomicBool>,
    pool: sqlx::SqlitePool,
    settings_repo: crate::models::settings::SettingsRepository,
    peer_repo: crate::models::peer::PeerRepository,
    node_repo: crate::models::node::NodeRepository,
    listen_addr: String,
    shutdown: CancellationToken,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("Config apply task shutting down");
                    return;
                }
                _ = interval.tick() => {}
            }
            if config_dirty.swap(false, std::sync::atomic::Ordering::Relaxed) {
                tracing::info!("Config dirty flag set, applying WG+BIRD configs...");
                if let Err(e) = crate::grpc::peer_service::apply_wg_bird(
                    &peer_repo,
                    &settings_repo,
                    &node_repo,
                    &listen_addr,
                    &pool,
                )
                .await
                {
                    tracing::warn!("Auto-apply WG+BIRD failed: {e}");
                }
            }
        }
    });
}
```

- [ ] **Step 4: Create src/tasks/retention.rs**

Extract retention cleanup from `main.rs` (lines 837-875):

```rust
use tokio_util::sync::CancellationToken;

pub fn spawn_retention_cleanup(pool: sqlx::SqlitePool, shutdown: CancellationToken) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
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
```

- [ ] **Step 5: Update main.rs to use task modules**

In `src/main.rs`:
1. Add `mod tasks;` at the top
2. Replace all `tokio::spawn` blocks inside the `if !node_name.is_empty()` block with:

```rust
// Spawn all cluster background tasks
tasks::cluster::ClusterTasks {
    node_name: node_name.clone(),
    listen_addr: listen_addr.clone(),
    cluster_key: cluster_key.clone(),
    sync_interval,
    probe_interval,
    peer_nodes: peer_nodes.clone(),
    tunnel_ip_range: tunnel_ip_range.clone(),
    tunnel_ipv6_range: tunnel_ipv6_range.clone(),
    state: state.clone(),
    pool: pool.clone(),
    shutdown: shutdown.clone(),
}
.spawn_all();

// Spawn config apply task (outside cluster block, always runs)
tasks::apply::spawn_config_apply(
    config_dirty.clone(),
    pool.clone(),
    state.settings_repo.clone(),
    state.peer_repo.clone(),
    state.node_repo.clone(),
    listen_addr.clone(),
    shutdown.clone(),
);

// Spawn retention cleanup
tasks::retention::spawn_retention_cleanup(pool.clone(), shutdown.clone());
```

- [ ] **Step 6: Run tests**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 7: Run clippy and format**

Run: `cargo clippy && cargo fmt`

- [ ] **Step 8: Verify main.rs is under 200 lines**

Run: `wc -l src/main.rs`
Expected: ~180-200 lines

- [ ] **Step 9: Commit**

```bash
git add src/tasks/ src/main.rs
git commit -m "refactor: extract background tasks from main.rs into tasks module"
```

---

### Task 8: Frontend 401 Interceptor

**Files:**
- Create: `frontend/src/lib/http.ts`
- Modify: `frontend/src/lib/auth.tsx`
- Modify: `frontend/src/hooks/usePeers.ts` (and other hooks that use fetch)

- [ ] **Step 1: Create frontend/src/lib/http.ts**

```typescript
/**
 * Fetch wrapper with automatic 401 handling.
 * Redirects to login when JWT expires.
 */
export async function fetchJson<T>(
  url: string,
  init?: RequestInit,
): Promise<T> {
  const res = await fetch(url, {
    ...init,
    credentials: 'same-origin',
  });

  if (res.status === 401) {
    // JWT expired or invalid — redirect to login
    const current = window.location.pathname;
    window.location.href = `/login?redirect=${encodeURIComponent(current)}`;
    throw new Error('Unauthorized');
  }

  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText);
    throw new Error(`${res.status}: ${text}`);
  }

  return res.json();
}

/**
 * Fetch that returns raw Response (for non-JSON endpoints).
 * Still handles 401 redirects.
 */
export async function fetchWithAuth(
  url: string,
  init?: RequestInit,
): Promise<Response> {
  const res = await fetch(url, {
    ...init,
    credentials: 'same-origin',
  });

  if (res.status === 401) {
    const current = window.location.pathname;
    window.location.href = `/login?redirect=${encodeURIComponent(current)}`;
    throw new Error('Unauthorized');
  }

  return res;
}
```

- [ ] **Step 2: Update auth.tsx to use fetchWithAuth**

In `frontend/src/lib/auth.tsx`, replace the raw `fetch` calls in `AuthProvider` with the wrapper:

```typescript
import { fetchJson, fetchWithAuth } from './http';

// In AuthProvider:
useEffect(() => {
  let cancelled = false;
  fetchWithAuth('/api/auth/me')
    .then((r) => r.json())
    .then((data: { authenticated: boolean; username?: string }) => {
      if (cancelled) return;
      setState({
        isAuthenticated: data.authenticated,
        username: data.username ?? null,
        loading: false,
      });
    })
    .catch(() => {
      if (cancelled) return;
      setState({ isAuthenticated: false, username: null, loading: false });
    });
  return () => { cancelled = true; };
}, []);

// login and logout methods remain the same (they don't need 401 handling)
```

- [ ] **Step 3: Verify hooks use gRPC (no raw fetch to update)**

Check that hooks like `usePeers.ts`, `useNodes.ts` etc. use ConnectRPC clients (not raw fetch). The gRPC transport already handles auth via cookies. If any hooks use raw `fetch` for non-gRPC endpoints, update them to use `fetchJson`.

Run: `grep -r "fetch(" frontend/src/hooks/ --include="*.ts"`
Expected: No raw fetch calls (all go through gRPC or should use fetchJson)

- [ ] **Step 4: Run TypeScript type check**

Run: `cd frontend && pnpm exec tsc --noEmit`
Expected: No type errors

- [ ] **Step 5: Commit**

```bash
git add frontend/src/lib/http.ts frontend/src/lib/auth.tsx
git commit -m "feat: add 401 interceptor to frontend for automatic login redirect on JWT expiry"
```

---

## Verification Checklist

After all tasks are complete:

- [ ] `cargo test` — all 71 tests pass
- [ ] `cargo clippy` — no warnings
- [ ] `cargo fmt` — all formatted
- [ ] `cd frontend && pnpm exec tsc --noEmit` — no type errors
- [ ] `wc -l src/main.rs` — under 200 lines
- [ ] `wc -l src/cluster/aggregator.rs` — reduced from 410 lines
- [ ] `cargo build` — full build succeeds
- [ ] Manual smoke test: `cargo run -- -c config.toml` — server starts, login works, peer CRUD works
