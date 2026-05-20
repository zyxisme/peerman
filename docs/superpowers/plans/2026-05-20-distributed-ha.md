# Distributed Flat HA Cluster — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Distributed flat-hierarchy HA management plane — any node serves the full panel, nodes discover each other via gossip, fan-out reads with local cache, write proxying.

**Architecture:** Two-path network (public IP for cluster gRPC, WG mesh for DN42 BGP). Anti-entropy node discovery via `ExchangeNodes`. API aggregation in each node's backend: fan-out concurrent gRPC to all online nodes on read, proxy to target node on write. In-memory `ClusterCache` serves stale data for offline nodes. `x-cluster-key` metadata authenticates inter-node calls.

**Tech Stack:** Rust (tonic 0.12 + axum 0.7 + sqlx), TypeScript (React 18 + Vite + Tailwind CSS), gRPC-Web via @connectrpc/connect v2.

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `src/config.rs` | Modify | `cluster_key`, `peer_nodes` fields |
| `proto/peerman.proto` | Modify | `ExchangeNodes`, `HealthCheck` RPCs + messages |
| `src/grpc/cluster_service.rs` | Modify | Implement new RPCs, cluster_key auth on inter-node calls, aggregator field |
| `src/cluster/auth.rs` | **Create** | `check_cluster_key()` helper |
| `src/cluster/cache.rs` | **Create** | `ClusterCache` |
| `src/cluster/aggregator.rs` | **Create** | `ClusterAggregator` — fan-out + proxy |
| `src/cluster/mod.rs` | **Create** | Module declarations |
| `src/app_state.rs` | Modify | Add `ClusterCache` field |
| `src/main.rs` | Modify | Bootstrap with ExchangeNodes, enhanced health, wire cache |
| `src/services/probe.rs` | Modify | Flap suppression helper |
| `src/models/node.rs` | Modify | `mark_online` / probe streak tracking |
| `config.toml.example` | Modify | Add new cluster fields |
| `frontend/src/layout/NavBar.tsx` | Modify | Cluster health indicator dot |
| `frontend/src/peers/PeerTable.tsx` | Modify | Origin node column, offline row styling |
| `frontend/src/peers/PeerForm.tsx` | Modify | Target node dropdown |
| `frontend/src/hooks/useNodes.ts` | Modify | Expose node status map |

---

### Task 1: Config — add cluster_key and peer_nodes

**Files:**
- Modify: `src/config.rs:52-62`
- Modify: `config.toml.example:27-31`

- [ ] **Step 1: Replace ClusterConfig struct and Default impl**

In `src/config.rs`, replace lines 52-62 (the struct):

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClusterConfig {
    #[serde(default)]
    pub node_name: String,
    #[serde(default)]
    pub cluster_key: String,
    #[serde(default)]
    pub peer_nodes: Vec<String>,
    #[serde(default = "default_probe_interval")]
    pub probe_interval_secs: u64,
    #[serde(default = "default_sync_interval")]
    pub sync_interval_secs: u64,
}
```

Replace lines 112-117 (Default impl for ClusterConfig):

```rust
impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            node_name: String::new(),
            cluster_key: String::new(),
            peer_nodes: Vec::new(),
            probe_interval_secs: 60,
            sync_interval_secs: 30,
        }
    }
}
```

- [ ] **Step 2: Update config.toml.example cluster section**

Replace the `[cluster]` section:

```toml
[cluster]
# node_name = ""            # empty = standalone mode
# cluster_key = ""          # shared secret for inter-node gRPC
# peer_nodes = ["1.2.3.4:3000", "5.6.7.8:3000"]  # initial bootstrap peers (public IPs)
# probe_interval_secs = 15  # health check interval (default 60)
# sync_interval_secs = 30   # node list exchange interval (default 30)
```

- [ ] **Step 3: Build**

```bash
source "$HOME/.cargo/env" && cargo build 2>&1 | tail -10
```

Expected: `Finished` with no errors.

- [ ] **Step 4: Commit**

```bash
git add src/config.rs config.toml.example
git commit -m "feat: add cluster_key and peer_nodes to ClusterConfig"
```

---

### Task 2: Proto — add ExchangeNodes and HealthCheck RPCs

**Files:**
- Modify: `proto/peerman.proto:193-207`

- [ ] **Step 1: Add message types after GetPeerCommunitiesResponse**

Find the last message before `BirdService` (around line 260). After `GetPeerCommunitiesResponse`, insert:

```protobuf
message NodeInfo {
  string name = 1;
  string listen_addr = 2;
  int64 local_asn = 3;
  string description = 4;
  string last_seen_at = 5;
}

message ExchangeNodesRequest {
  repeated NodeInfo nodes = 1;
}

message ExchangeNodesResponse {
  repeated NodeInfo nodes = 1;
}

message HealthCheckRequest {}

message HealthCheckResponse {
  bool ok = 1;
}

message ListNodesRequest {}
```

- [ ] **Step 2: Add RPCs to ClusterService**

Find `service ClusterService` (line ~193). After the last existing RPC (`GetPeerCommunities` around line 207), add:

```protobuf
  rpc ExchangeNodes(ExchangeNodesRequest) returns (ExchangeNodesResponse);
  rpc HealthCheck(HealthCheckRequest) returns (HealthCheckResponse);
```

- [ ] **Step 3: Build to regenerate stubs**

```bash
source "$HOME/.cargo/env" && cargo build 2>&1 | tail -20
```

Expected: compiles. New types `ExchangeNodesRequest`, `ExchangeNodesResponse`, `HealthCheckRequest`, `HealthCheckResponse` available in generated code. Two new methods on `ClusterService` trait.

- [ ] **Step 4: Commit**

```bash
git add proto/peerman.proto
git commit -m "feat: add ExchangeNodes and HealthCheck RPCs to ClusterService"
```

---

### Task 3: Cluster auth module

**Files:**
- Create: `src/cluster/mod.rs`
- Create: `src/cluster/auth.rs`
- Modify: `src/main.rs` (add `mod cluster;`)

- [ ] **Step 1: Create `src/cluster/mod.rs`**

```rust
pub mod auth;
pub mod cache;
pub mod aggregator;
```

- [ ] **Step 2: Create `src/cluster/auth.rs`**

```rust
use tonic::{Status, Request};

/// Validate x-cluster-key metadata against the shared secret.
/// Returns Ok if valid, Err(PermissionDenied) if missing or mismatched.
/// If cluster_key is empty on this node (not configured), allows all.
pub fn check_cluster_key<T>(req: &Request<T>, secret: &str) -> Result<(), Status> {
    if secret.is_empty() {
        return Ok(());
    }
    let key = req
        .metadata()
        .get("x-cluster-key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| Status::permission_denied("missing x-cluster-key"))?;
    if key != secret {
        return Err(Status::permission_denied("cluster key mismatch"));
    }
    Ok(())
}
```

- [ ] **Step 3: Add `mod cluster;` to `src/main.rs`**

After `mod auth;` (around line 16):

```rust
mod cluster;
```

- [ ] **Step 4: Build**

```bash
source "$HOME/.cargo/env" && cargo build 2>&1 | tail -10
```

Expected: `Finished` (cache.rs and aggregator.rs not created yet, but mod.rs references them — need to create them or use `#[path]` later; for now build will fail on missing modules)

**Fix**: Create placeholder files:

`src/cluster/cache.rs`:
```rust
// placeholder
```

`src/cluster/aggregator.rs`:
```rust
// placeholder
```

Then build.

- [ ] **Step 5: Commit**

```bash
git add src/cluster/ src/main.rs
git commit -m "feat: add cluster auth helper check_cluster_key"
```

---

### Task 4: In-Memory ClusterCache

**Files:**
- Modify: `src/cluster/cache.rs` (replace placeholder)
- Modify: `src/app_state.rs`

- [ ] **Step 1: Write `src/cluster/cache.rs`**

```rust
use std::collections::HashMap;
use std::time::Instant;

use crate::models::community::CommunityRule;
use crate::models::peer::Peer;
use crate::models::probe::ProbeResult;

#[derive(Clone, Debug)]
pub struct NodeCacheEntry {
    pub peers: Vec<Peer>,
    pub probe_results: Vec<ProbeResult>,
    pub community_rules: Vec<CommunityRule>,
    pub fetched_at: Instant,
    pub stale: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ClusterCache {
    by_node: std::sync::Arc<tokio::sync::RwLock<HashMap<String, NodeCacheEntry>>>,
}

impl ClusterCache {
    pub fn new() -> Self {
        Self {
            by_node: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    pub async fn update(
        &self,
        node_addr: &str,
        peers: Vec<Peer>,
        probe_results: Vec<ProbeResult>,
        community_rules: Vec<CommunityRule>,
    ) {
        let mut map = self.by_node.write().await;
        map.insert(
            node_addr.to_string(),
            NodeCacheEntry {
                peers,
                probe_results,
                community_rules,
                fetched_at: Instant::now(),
                stale: false,
            },
        );
    }

    pub async fn get(&self, node_addr: &str) -> Option<NodeCacheEntry> {
        let map = self.by_node.read().await;
        map.get(node_addr).cloned()
    }

    pub async fn mark_stale(&self, node_addr: &str) {
        let mut map = self.by_node.write().await;
        if let Some(entry) = map.get_mut(node_addr) {
            entry.stale = true;
        }
    }

    pub async fn invalidate(&self, node_addr: &str) {
        let mut map = self.by_node.write().await;
        map.remove(node_addr);
    }
}
```

- [ ] **Step 2: Add cache to AppState in `src/app_state.rs`**

Add import:

```rust
use crate::cluster::cache::ClusterCache;
```

Add field:

```rust
pub struct AppState {
    pub peer_repo: PeerRepository,
    pub settings_repo: SettingsRepository,
    pub node_repo: NodeRepository,
    pub probe_repo: ProbeResultRepository,
    pub community_repo: CommunityRuleRepository,
    pub flap_event_repo: FlapEventRepository,
    pub cluster_cache: ClusterCache,
}
```

Update `new()`:

```rust
impl AppState {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            peer_repo: PeerRepository::new(pool.clone()),
            settings_repo: SettingsRepository::new(pool.clone()),
            node_repo: NodeRepository::new(pool.clone()),
            probe_repo: ProbeResultRepository::new(pool.clone()),
            community_repo: CommunityRuleRepository::new(pool.clone()),
            flap_event_repo: FlapEventRepository::new(pool.clone()),
            cluster_cache: ClusterCache::new(),
        }
    }
}
```

- [ ] **Step 3: Build**

```bash
source "$HOME/.cargo/env" && cargo build 2>&1 | tail -10
```

Expected: `Finished`.

- [ ] **Step 4: Commit**

```bash
git add src/cluster/cache.rs src/app_state.rs
git commit -m "feat: add in-memory ClusterCache for node data snapshots"
```

---

### Task 5: ClusterAggregator — fan-out reads

**Files:**
- Modify: `src/cluster/aggregator.rs` (replace placeholder)

- [ ] **Step 1: Write `src/cluster/aggregator.rs`**

```rust
use std::sync::Arc;
use std::time::Duration;

use tokio::time::timeout;
use tonic::transport::Channel;
use tonic::Request;

use crate::cluster::cache::ClusterCache;
use crate::models::community::CommunityRule;
use crate::models::node::Node;
use crate::models::peer::Peer;
use crate::models::probe::ProbeResult;

use super::super::grpc::generated::cluster_service_client::ClusterServiceClient;
use super::super::grpc::generated::{
    ExchangeNodesRequest, ExchangeNodesResponse, HealthCheckRequest, HealthCheckResponse,
    ListCommunityRulesRequest, ListProbeResultsRequest, NodeInfo, PullPeersRequest,
};

const FANOUT_TIMEOUT: Duration = Duration::from_secs(2);

pub struct ClusterAggregator {
    pub cache: ClusterCache,
    pub cluster_key: String,
}

impl ClusterAggregator {
    pub fn new(cache: ClusterCache, cluster_key: String) -> Self {
        Self { cache, cluster_key }
    }

    async fn connect(addr: &str) -> Result<ClusterServiceClient<Channel>, String> {
        let uri = format!("http://{}", addr);
        let channel = Channel::from_shared(uri)
            .map_err(|e| format!("invalid uri: {e}"))?
            .connect()
            .await
            .map_err(|e| format!("connect failed: {e}"))?;
        Ok(ClusterServiceClient::new(channel))
    }

    fn key_metadata<T>(&self) -> tonic::metadata::MetadataValue<tonic::metadata::Ascii> {
        tonic::metadata::Ascii::from_str(&self.cluster_key)
            .unwrap_or_else(|_| tonic::metadata::Ascii::from_str("").unwrap())
    }

    /// Fan-out PullPeers to all online nodes, return merged peers + per-node status.
    /// Updates cache for successful responses; uses cache for failed/timeout nodes.
    pub async fn fanout_peers(
        &self,
        local_addr: &str,
        online_nodes: &[Node],
    ) -> AggregatedResult<Peer> {
        let mut all: Vec<Peer> = Vec::new();
        let mut statuses: Vec<NodeStatus> = Vec::new();

        for node in online_nodes {
            if node.listen_addr == local_addr {
                continue; // skip self
            }
            let node_addr = node.listen_addr.clone();
            let node_name = node.name.clone();

            let client = match Self::connect(&node_addr).await {
                Ok(c) => c,
                Err(e) => {
                    // node unreachable — use cache
                    if let Some(entry) = self.cache.get(&node_addr).await {
                        for mut p in entry.peers {
                            p.origin_node_id = Some(node_name.clone());
                            all.push(p);
                        }
                        statuses.push(NodeStatus {
                            node_name: node_name.clone(),
                            node_addr: node_addr.clone(),
                            online: false,
                            staleness: "stale".into(),
                            error: Some(e),
                        });
                    } else {
                        statuses.push(NodeStatus {
                            node_name: node_name.clone(),
                            node_addr: node_addr.clone(),
                            online: false,
                            staleness: "unknown".into(),
                            error: Some(e),
                        });
                    }
                    self.cache.mark_stale(&node_addr).await;
                    continue;
                }
            };

            let mut req = Request::new(PullPeersRequest {});
            if !self.cluster_key.is_empty() {
                if let Ok(v) = tonic::metadata::Ascii::from_str(&self.cluster_key) {
                    req.metadata_mut().insert("x-cluster-key", v);
                }
            }

            match timeout(FANOUT_TIMEOUT, client.pull_peers(req)).await {
                Ok(Ok(response)) => {
                    let peers: Vec<Peer> = response
                        .into_inner()
                        .peers
                        .into_iter()
                        .filter_map(|p| Peer::try_from_proto(&p))
                        .collect();
                    // Update cache
                    self.cache
                        .update(&node_addr, peers.clone(), vec![], vec![])
                        .await;
                    for mut p in peers {
                        p.origin_node_id = Some(node_name.clone());
                        all.push(p);
                    }
                    statuses.push(NodeStatus {
                        node_name: node_name.clone(),
                        node_addr: node_addr.clone(),
                        online: true,
                        staleness: "fresh".into(),
                        error: None,
                    });
                }
                _ => {
                    // timeout or error — use cache
                    if let Some(entry) = self.cache.get(&node_addr).await {
                        for mut p in entry.peers {
                            p.origin_node_id = Some(node_name.clone());
                            all.push(p);
                        }
                        statuses.push(NodeStatus {
                            node_name: node_name.clone(),
                            node_addr: node_addr.clone(),
                            online: false,
                            staleness: "stale".into(),
                            error: Some("fanout failed".into()),
                        });
                    } else {
                        statuses.push(NodeStatus {
                            node_name: node_name.clone(),
                            node_addr: node_addr.clone(),
                            online: false,
                            staleness: "unknown".into(),
                            error: Some("fanout failed".into()),
                        });
                    }
                    self.cache.mark_stale(&node_addr).await;
                }
            }
        }

        AggregatedResult {
            items: all,
            node_statuses: statuses,
        }
    }

    /// Fan-out ListProbeResults
    pub async fn fanout_probe_results(
        &self,
        local_addr: &str,
        online_nodes: &[Node],
    ) -> AggregatedResult<ProbeResult> {
        let mut all: Vec<ProbeResult> = Vec::new();
        let mut statuses: Vec<NodeStatus> = Vec::new();

        for node in online_nodes {
            if node.listen_addr == local_addr {
                continue;
            }
            let node_addr = node.listen_addr.clone();
            let node_name = node.name.clone();

            let client = match Self::connect(&node_addr).await {
                Ok(c) => c,
                Err(_) => {
                    if let Some(entry) = self.cache.get(&node_addr).await {
                        all.extend(entry.probe_results);
                        statuses.push(NodeStatus {
                            node_name: node_name.clone(),
                            node_addr: node_addr.clone(),
                            online: false,
                            staleness: "stale".into(),
                            error: Some("unreachable".into()),
                        });
                    }
                    self.cache.mark_stale(&node_addr).await;
                    continue;
                }
            };

            let mut req = Request::new(ListProbeResultsRequest {
                from_node_id: String::new(),
                to_node_id: String::new(),
                limit: 0,
            });
            if !self.cluster_key.is_empty() {
                if let Ok(v) = tonic::metadata::Ascii::from_str(&self.cluster_key) {
                    req.metadata_mut().insert("x-cluster-key", v);
                }
            }

            match timeout(FANOUT_TIMEOUT, client.list_probe_results(req)).await {
                Ok(Ok(response)) => {
                    let results: Vec<ProbeResult> = response
                        .into_inner()
                        .results
                        .into_iter()
                        .map(|r| ProbeResult::from_proto(&r))
                        .collect();
                    all.extend(results);
                    statuses.push(NodeStatus {
                        node_name: node_name.clone(),
                        node_addr: node_addr.clone(),
                        online: true,
                        staleness: "fresh".into(),
                        error: None,
                    });
                }
                _ => {
                    if let Some(entry) = self.cache.get(&node_addr).await {
                        all.extend(entry.probe_results);
                    }
                    statuses.push(NodeStatus {
                        node_name: node_name.clone(),
                        node_addr: node_addr.clone(),
                        online: false,
                        staleness: "stale".into(),
                        error: Some("fanout failed".into()),
                    });
                    self.cache.mark_stale(&node_addr).await;
                }
            }
        }

        AggregatedResult {
            items: all,
            node_statuses: statuses,
        }
    }

    /// Fan-out ListCommunityRules
    pub async fn fanout_community_rules(
        &self,
        local_addr: &str,
        online_nodes: &[Node],
    ) -> AggregatedResult<CommunityRule> {
        let mut all: Vec<CommunityRule> = Vec::new();
        let mut statuses: Vec<NodeStatus> = Vec::new();

        for node in online_nodes {
            if node.listen_addr == local_addr {
                continue;
            }
            let node_addr = node.listen_addr.clone();
            let node_name = node.name.clone();

            let client = match Self::connect(&node_addr).await {
                Ok(c) => c,
                Err(_) => {
                    if let Some(entry) = self.cache.get(&node_addr).await {
                        all.extend(entry.community_rules);
                        statuses.push(NodeStatus {
                            node_name: node_name.clone(),
                            node_addr: node_addr.clone(),
                            online: false,
                            staleness: "stale".into(),
                            error: Some("unreachable".into()),
                        });
                    }
                    self.cache.mark_stale(&node_addr).await;
                    continue;
                }
            };

            let mut req = Request::new(ListCommunityRulesRequest {});
            if !self.cluster_key.is_empty() {
                if let Ok(v) = tonic::metadata::Ascii::from_str(&self.cluster_key) {
                    req.metadata_mut().insert("x-cluster-key", v);
                }
            }

            match timeout(FANOUT_TIMEOUT, client.list_community_rules(req)).await {
                Ok(Ok(response)) => {
                    let rules: Vec<CommunityRule> = response
                        .into_inner()
                        .rules
                        .into_iter()
                        .map(|r| CommunityRule::from_proto(&r))
                        .collect();
                    all.extend(rules);
                    statuses.push(NodeStatus {
                        node_name: node_name.clone(),
                        node_addr: node_addr.clone(),
                        online: true,
                        staleness: "fresh".into(),
                        error: None,
                    });
                }
                _ => {
                    if let Some(entry) = self.cache.get(&node_addr).await {
                        all.extend(entry.community_rules);
                    }
                    statuses.push(NodeStatus {
                        node_name: node_name.clone(),
                        node_addr: node_addr.clone(),
                        online: false,
                        staleness: "stale".into(),
                        error: Some("fanout failed".into()),
                    });
                    self.cache.mark_stale(&node_addr).await;
                }
            }
        }

        AggregatedResult {
            items: all,
            node_statuses: statuses,
        }
    }

    /// Call HealthCheck on a single node. Returns true if healthy.
    pub async fn health_check(node_addr: &str, cluster_key: &str) -> bool {
        let client = match Self::connect(node_addr).await {
            Ok(c) => c,
            Err(_) => return false,
        };
        let mut req = Request::new(HealthCheckRequest {});
        if !cluster_key.is_empty() {
            if let Ok(v) = tonic::metadata::Ascii::from_str(cluster_key) {
                req.metadata_mut().insert("x-cluster-key", v);
            }
        }
        timeout(FANOUT_TIMEOUT, client.health_check(req))
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false)
    }

    /// Exchange node list with a peer. Returns the peer's node list.
    pub async fn exchange_with(
        node_addr: &str,
        cluster_key: &str,
        my_nodes: Vec<NodeInfo>,
    ) -> Result<Vec<NodeInfo>, String> {
        let client = Self::connect(node_addr)
            .await
            .map_err(|e| format!("connect: {e}"))?;
        let mut req = Request::new(ExchangeNodesRequest { nodes: my_nodes });
        if !cluster_key.is_empty() {
            if let Ok(v) = tonic::metadata::Ascii::from_str(cluster_key) {
                req.metadata_mut().insert("x-cluster-key", v);
            }
        }
        let response = timeout(FANOUT_TIMEOUT, client.exchange_nodes(req))
            .await
            .map_err(|_| "timeout".to_string())?
            .map_err(|e| format!("rpc: {e}"))?;
        Ok(response.into_inner().nodes)
    }
}

#[derive(Clone, Debug)]
pub struct NodeStatus {
    pub node_name: String,
    pub node_addr: String,
    pub online: bool,
    pub staleness: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AggregatedResult<T> {
    pub items: Vec<T>,
    pub node_statuses: Vec<NodeStatus>,
}
```

The aggregator stores **generated proto types** directly (not model types). The `ClusterCache` declared in Task 4 is updated in Task 17 to use proto types. For this task, use these proto types in the aggregator:

- `generated::Peer` — from `PullPeersResponse.peers`
- `generated::ProbeResult` — from `ListProbeResultsResponse.results`
- `generated::CommunityRule` — from `ListCommunityRulesResponse.rules`

No model conversion needed. Clone proto objects directly into cache entries.

- [ ] **Step 2: Build to check compilation**

```bash
source "$HOME/.cargo/env" && cargo build 2>&1 | tail -30
```

Expected: may have compilation errors due to type paths. Fix by importing from `crate::grpc::generated::*` or the full module path `super::super::grpc::generated::cluster_service_client::ClusterServiceClient`. Adjust import paths until clean.

- [ ] **Step 3: Commit**

```bash
git add src/cluster/aggregator.rs
git commit -m "feat: add ClusterAggregator for fan-out reads and node exchange"
```

---

### Task 6: Implement ExchangeNodes and HealthCheck in ClusterServiceImpl

**Files:**
- Modify: `src/grpc/cluster_service.rs`

- [ ] **Step 1: Add cluster_key to ClusterServiceImpl struct**

At lines 19-25, add the field:

```rust
pub struct ClusterServiceImpl {
    pub node_repo: NodeRepository,
    pub peer_repo: PeerRepository,
    pub probe_repo: ProbeResultRepository,
    pub community_repo: CommunityRuleRepository,
    pub jwt_secret: std::sync::Arc<String>,
    pub cluster_key: std::sync::Arc<String>,
    pub listen_addr: String,
}
```

Update imports for new types — add near other use lines:

```rust
use crate::cluster::auth::check_cluster_key;
```

- [ ] **Step 2: Implement exchange_nodes**

Add at the end of the `impl ClusterService for ClusterServiceImpl` block (before the closing `}`):

```rust
async fn exchange_nodes(
    &self,
    request: Request<ExchangeNodesRequest>,
) -> Result<Response<ExchangeNodesResponse>, Status> {
    check_cluster_key(&request, &self.cluster_key)?;

    let incoming = request.into_inner().nodes;
    for info in &incoming {
        if let Ok(Some(_)) = self.node_repo.find_by_listen_addr(&info.listen_addr).await {
            continue; // already known
        }
        let _ = self
            .node_repo
            .create(
                &info.name,
                &info.listen_addr,
                info.local_asn,
                Some(&info.description.clone().unwrap_or_default()),
            )
            .await;
    }

    // Return our known nodes
    let local_nodes = self.node_repo.list_all().await.map_err(|e| {
        Status::internal(format!("failed to list nodes: {e}"))
    })?;
    let response_nodes: Vec<NodeInfo> = local_nodes
        .iter()
        .map(|n| NodeInfo {
            name: n.name.clone(),
            listen_addr: n.listen_addr.clone(),
            local_asn: n.local_asn,
            description: n.description.clone().unwrap_or_default(),
            last_seen_at: n.last_seen_at.clone(),
        })
        .collect();

    Ok(Response::new(ExchangeNodesResponse {
        nodes: response_nodes,
    }))
}
```

- [ ] **Step 3: Implement health_check**

```rust
async fn health_check(
    &self,
    _request: Request<HealthCheckRequest>,
) -> Result<Response<HealthCheckResponse>, Status> {
    Ok(Response::new(HealthCheckResponse { ok: true }))
}
```

- [ ] **Step 4: Apply cluster_key auth to existing inter-node RPCs**

For `push_peer`: add at the top of the method body, immediately after the function signature:

```rust
async fn push_peer(
    &self,
    request: Request<PushPeerRequest>,
) -> Result<Response<PushPeerResponse>, Status> {
    check_cluster_key(&request, &self.cluster_key)?;
    // ... existing body unchanged
}
```

For `push_probe_result`:

```rust
check_cluster_key(&request, &self.cluster_key)?;
```

- [ ] **Step 5: Apply cluster_key auth to write RPCs called inter-node**

For `save_community_rule` — this can be called both by user (JWT) and by another node (cluster key). Accept either:

```rust
async fn save_community_rule(
    &self,
    request: Request<SaveCommunityRuleRequest>,
) -> Result<Response<SaveCommunityRuleResponse>, Status> {
    // Accept either JWT user auth OR cluster key
    let jwt_ok = crate::auth::check_auth(&request, &self.jwt_secret).is_ok();
    let cluster_ok = check_cluster_key(&request, &self.cluster_key).is_ok();
    if !jwt_ok && !cluster_ok {
        return Err(Status::unauthenticated("auth required"));
    }
    // ... existing body unchanged
}
```

Same dual-auth pattern for `delete_community_rule`.

For `register_node`, `update_node`, `delete_node` — these are user-only (keep JWT only). Nodes join via `ExchangeNodes` + `peer_nodes` bootstrap, not manual `register_node` calls from other nodes.

- [ ] **Step 6: Build**

```bash
source "$HOME/.cargo/env" && cargo build 2>&1 | tail -20
```

Expected: compile errors for type mismatches. Fix:
- `ExchangeNodesRequest`/`ExchangeNodesResponse` generated type names (may have module prefix)
- `NodeInfo` field names (match proto exactly)
- Any missing imports

Fix until clean compile.

- [ ] **Step 7: Commit**

```bash
git add src/grpc/cluster_service.rs
git commit -m "feat: implement ExchangeNodes, HealthCheck, cluster key auth on inter-node RPCs"
```

---

### Task 7: NodeRepository — online/offline tracking

**Files:**
- Modify: `src/models/node.rs`

- [ ] **Step 1: Add `mark_online` method**

After `mark_stale` (line 135), add:

```rust
pub async fn mark_online(&self, id: &str) -> Result<(), AppError> {
    sqlx::query("UPDATE nodes SET online = 1, last_seen_at = ?1, updated_at = ?1 WHERE id = ?2")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id)
        .execute(&self.pool)
        .await?;
    Ok(())
}
```

- [ ] **Step 2: Build**

```bash
source "$HOME/.cargo/env" && cargo build 2>&1 | tail -10
```

Expected: `Finished`.

- [ ] **Step 3: Commit**

```bash
git add src/models/node.rs
git commit -m "feat: add mark_online to NodeRepository"
```

---

### Task 8: Health checking with flap suppression

**Files:**
- Modify: `src/services/probe.rs` (add flap suppression logic, but keep it in the caller in main.rs)
- Modify: `src/main.rs` (rewrite health check task)

- [ ] **Step 1: Use a simple approach in main.rs — replace the probe-all loop**

The current probe task (lines 329-367) pings all nodes every `probe_interval_secs`. Replace the inner loop with flap-suppressed health tracking.

In `src/main.rs`, find the probe task (around line 329). Replace the entire `tokio::spawn` block with:

```rust
let probe_ct = cancel.clone();
let probe_interval = probe_interval;
let node_repo_probe = state.node_repo.clone();
let probe_repo_probe = state.probe_repo.clone();
let node_name_probe = node_name.clone();
let listen_addr_probe = listen_addr.clone();
let aggregator = ClusterAggregator::new(
    state.cluster_cache.clone(),
    cluster_key.clone(),
);

tokio::spawn(async move {
    // Track consecutive failures per node for flap suppression
    let mut fail_streaks: std::collections::HashMap<String, u32> = std::collections::HashMap::new();

    let mut interval = tokio::time::interval(Duration::from_secs(probe_interval));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = probe_ct.cancelled() => break,
            _ = interval.tick() => {}
        }

        let nodes = match node_repo_probe.list_all().await {
            Ok(n) => n,
            Err(e) => {
                tracing::warn!("Failed to list nodes for health check: {e}");
                continue;
            }
        };

        let local = nodes.iter().find(|n| n.name == node_name_probe);

        for node in &nodes {
            if node.name == node_name_probe {
                continue;
            }

            // Try gRPC HealthCheck first, fall back to ICMP ping
            let healthy = ClusterAggregator::health_check(
                &node.listen_addr,
                &cluster_key,
            ).await;

            let prev_fails = fail_streaks.get(&node.listen_addr).copied().unwrap_or(0);

            if healthy {
                if prev_fails >= 2 {
                    // Was offline, now back — mark online
                    let _ = node_repo_probe.mark_online(&node.id).await;
                    state.cluster_cache.invalidate(&node.listen_addr).await;
                    tracing::info!("Node {} ({}) is back online", node.name, node.listen_addr);
                }
                fail_streaks.insert(node.listen_addr.clone(), 0);

                // Also run ICMP probe for latency data
                if let Some(ref local_node) = local {
                    let _ = probe::probe_between(
                        local_node,
                        node,
                        &probe_repo_probe,
                    ).await;
                }
            } else {
                let new_fails = prev_fails + 1;
                fail_streaks.insert(node.listen_addr.clone(), new_fails);

                if new_fails >= 2 && prev_fails < 2 {
                    // Just crossed threshold — mark offline
                    let _ = node_repo_probe.mark_stale_node(&node.id).await;
                    state.cluster_cache.mark_stale(&node.listen_addr).await;
                    tracing::warn!("Node {} ({}) went offline after {} consecutive failures",
                        node.name, node.listen_addr, new_fails);
                }
            }
        }
    }
});
```

Need a `mark_stale_node` helper:

In `src/models/node.rs`, after `mark_online`:

```rust
pub async fn mark_stale_node(&self, id: &str) -> Result<(), AppError> {
    sqlx::query("UPDATE nodes SET online = 0, updated_at = ?1 WHERE id = ?2")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(id)
        .execute(&self.pool)
        .await?;
    Ok(())
}
```

- [ ] **Step 2: Wire cluster_key into main.rs**

In `main.rs`, after reading config fields (around line 210), add:

```rust
let cluster_key = cfg.cluster.cluster_key.clone();
```

Pass `cluster_key` to task spawns and to `ClusterServiceImpl` constructor.

- [ ] **Step 3: Build**

```bash
source "$HOME/.cargo/env" && cargo build 2>&1 | tail -20
```

Expected: compile errors, fix imports and type issues.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs src/models/node.rs
git commit -m "feat: health check with flap suppression and gRPC HealthCheck"
```

---

### Task 9: Node discovery — bootstrap with ExchangeNodes

**Files:**
- Modify: `src/main.rs` (replace bootstrap section)

- [ ] **Step 1: Replace bootstrap_nodes logic with ExchangeNodes**

Find the bootstrap section in `main.rs` (around lines 290-306). Replace:

```rust
// Original: inserts placeholder nodes for each bootstrap address
let mut bootstrap_peer_addrs = Vec::new();
for addr in &bootstrap_nodes { ... }
```

With:

```rust
let mut bootstrap_peer_addrs = peer_nodes.clone();

// Seed bootstrap peers into local nodes table
for addr in &peer_nodes {
    if let Ok(Some(_)) = state.node_repo.find_by_listen_addr(addr).await {
        continue;
    }
    let name = format!("node-{}", addr.replace(['.', ':'], "-"));
    let _ = state
        .node_repo
        .create(&name, addr, 0, Some("bootstrap"))
        .await;
}

// Exchange node lists with all bootstrap peers to discover the full cluster
if !peer_nodes.is_empty() {
    let local_nodes = state.node_repo.list_all().await.unwrap_or_default();
    let my_info: Vec<NodeInfo> = local_nodes
        .iter()
        .map(|n| NodeInfo {
            name: n.name.clone(),
            listen_addr: n.listen_addr.clone(),
            local_asn: n.local_asn,
            description: n.description.clone().unwrap_or_default(),
            last_seen_at: n.last_seen_at.clone(),
        })
        .collect();

    for addr in &peer_nodes {
        match ClusterAggregator::exchange_with(addr, &cluster_key, my_info.clone()).await {
            Ok(remote_nodes) => {
                for info in &remote_nodes {
                    if info.listen_addr == listen_addr {
                        continue; // skip self
                    }
                    if let Ok(Some(_)) = state.node_repo.find_by_listen_addr(&info.listen_addr).await {
                        continue;
                    }
                    let _ = state
                        .node_repo
                        .create(
                            &info.name,
                            &info.listen_addr,
                            info.local_asn,
                            Some(&info.description),
                        )
                        .await;
                }
                tracing::info!("Discovered {} nodes from bootstrap peer {}", remote_nodes.len(), addr);
            }
            Err(e) => {
                tracing::warn!("Failed to exchange nodes with bootstrap peer {}: {}", addr, e);
            }
        }
    }
}
```

Need to add `use crate::cluster::aggregator::ClusterAggregator;` at top of main.rs.

Also add `use super::super::grpc::generated::NodeInfo;` or import from generated.

- [ ] **Step 2: Build**

```bash
source "$HOME/.cargo/env" && cargo build 2>&1 | tail -20
```

Expected: fix import issues.

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: bootstrap node discovery via ExchangeNodes gossip"
```

---

### Task 10: Update ClusterServiceImpl constructor in main.rs

**Files:**
- Modify: `src/main.rs` (around lines 248-253)

- [ ] **Step 1: Pass new fields to ClusterServiceImpl**

Find the ClusterServiceImpl construction. Replace with:

```rust
let cluster_svc = ClusterServiceImpl {
    node_repo: state.node_repo.clone(),
    peer_repo: state.peer_repo.clone(),
    probe_repo: state.probe_repo.clone(),
    community_repo: state.community_repo.clone(),
    jwt_secret: jwt_secret.clone(),
    cluster_key: Arc::new(cluster_key.clone()),
    listen_addr: listen_addr.clone(),
};
```

- [ ] **Step 2: Build and fix any remaining issues**

```bash
source "$HOME/.cargo/env" && cargo build 2>&1 | tail -20
```

- [ ] **Step 3: Run unit tests**

```bash
source "$HOME/.cargo/env" && cargo test 2>&1 | tail -20
```

Expected: all 23 tests pass.

- [ ] **Step 4: Run clippy**

```bash
source "$HOME/.cargo/env" && cargo clippy 2>&1 | tail -20
```

Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire cluster_key and listen_addr into ClusterServiceImpl"
```

---

### Task 11: Frontend regenerate TypeScript proto stubs

**Files:**
- Regenerate: `frontend/src/lib/peerman_pb.ts`

- [ ] **Step 1: Regenerate proto stubs**

```bash
cd frontend && PATH="node_modules/.bin:$PATH" protoc -I ../proto --es_out src/lib --es_opt target=ts ../proto/peerman.proto
```

- [ ] **Step 2: TypeScript type-check**

```bash
cd frontend && pnpm exec tsc --noEmit 2>&1 | tail -20
```

Expected: type errors may appear since generated types changed but hooks haven't been updated yet. That's OK — we'll fix in subsequent tasks. Just verify the proto regeneration succeeded (new types in `peerman_pb.ts`).

- [ ] **Step 3: Commit**

```bash
git add frontend/src/lib/peerman_pb.ts
git commit -m "chore: regenerate TS proto stubs with ExchangeNodes and HealthCheck"
```

---

### Task 12: Frontend — NavBar cluster health indicator

**Files:**
- Modify: `frontend/src/layout/NavBar.tsx`
- Modify: `frontend/src/hooks/useNodes.ts`

- [ ] **Step 1: Add useClusterHealth hook in `useNodes.ts`**

At the end of `frontend/src/hooks/useNodes.ts`, add:

```typescript
export function useClusterHealth(): 'all-online' | 'partial' | 'isolated' {
  const { nodes } = useNodes();
  if (nodes.length <= 1) return 'isolated';
  const onlineCount = nodes.filter((n) => n.online).length;
  if (onlineCount === nodes.length) return 'all-online';
  if (onlineCount <= 1) return 'isolated';
  return 'partial';
}
```

- [ ] **Step 2: Add indicator dot to NavBar**

In `frontend/src/layout/NavBar.tsx`, add import:

```typescript
import { useClusterHealth } from '../hooks/useNodes';
```

Inside the component, before the return:

```typescript
const health = useClusterHealth();
const dotColor =
  health === 'all-online' ? 'bg-green-500' :
  health === 'partial' ? 'bg-yellow-500' :
  'bg-red-500';
```

In the nav JSX, add a dot next to the "Peerman" brand or settings area:

```tsx
<span className={`inline-block w-2 h-2 rounded-full ${dotColor}`}
      title={
        health === 'all-online' ? 'All nodes online' :
        health === 'partial' ? 'Some nodes offline' :
        'Only local node online'
      }
/>
```

Place it after the brand link or near the auth section.

- [ ] **Step 3: TypeScript type-check**

```bash
cd frontend && pnpm exec tsc --noEmit 2>&1 | tail -10
```

Expected: clean (or fix any type errors).

- [ ] **Step 4: Commit**

```bash
git add frontend/src/layout/NavBar.tsx frontend/src/hooks/useNodes.ts
git commit -m "feat: add cluster health indicator dot in NavBar"
```

---

### Task 13: Frontend — origin node column in PeerTable

**Files:**
- Modify: `frontend/src/peers/PeerTable.tsx`

- [ ] **Step 1: Add node column to peer table**

In `PeerTable.tsx`, find the table header (around line 40-70). Add a new `<th>` for "节点":

```tsx
<th className="data-table__th">节点</th>
```

In the table body, add a new `<td>` per row. The `peer` proto has `originNodeId`. Show the node name if available, or "本地" if empty:

```tsx
<td className="data-table__td">
  <span className={!peer.originNodeId ? 'badge' : ''}>
    {peer.originNodeId
      ? peer.originNodeId
      : '本地'}
  </span>
</td>
```

If you want to show the actual node name rather than ID, need to look up from nodes list:

```typescript
const { nodes } = useNodes();
const nodeName = (id: string) => nodes.find(n => n.id === id)?.name || id;
```

Then: `{peer.originNodeId ? nodeName(peer.originNodeId) : '本地'}`

- [ ] **Step 2: Add offline peer row styling**

When a node is offline, its peers are served from cache. Add a visual distinction:

```tsx
const offlineNodes = new Set(
  nodes.filter(n => !n.online).map(n => n.id)
);
const isStale = peer.originNodeId && offlineNodes.has(peer.originNodeId);
```

Apply grayed styling:

```tsx
<tr className={isStale ? 'opacity-50' : ''} title={isStale ? '节点离线，数据来自缓存' : undefined}>
```

- [ ] **Step 3: TypeScript type-check**

```bash
cd frontend && pnpm exec tsc --noEmit 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add frontend/src/peers/PeerTable.tsx
git commit -m "feat: add origin node column and offline styling to PeerTable"
```

---

### Task 14: Frontend — target node selector in PeerForm

**Files:**
- Modify: `frontend/src/peers/PeerForm.tsx`

- [ ] **Step 1: Add node dropdown to create form**

In `PeerForm.tsx`, import `useNodes`:

```typescript
import { useNodes } from '../hooks/useNodes';
```

Inside the component:

```typescript
const { nodes } = useNodes();
const onlineNodes = nodes.filter(n => n.online);
```

In the Identity section of the form (near the top fields), add a node selector:

```tsx
<div className="form-field">
  <label className="form-label">目标节点</label>
  <select
    className="form-input"
    value={targetNodeId}
    onChange={(e) => setTargetNodeId(e.target.value)}
  >
    {onlineNodes.map((n) => (
      <option key={n.id} value={n.id}>
        {n.name} ({n.listenAddr})
      </option>
    ))}
  </select>
</div>
```

Add state:

```typescript
const [targetNodeId, setTargetNodeId] = useState<string>('');
```

Initialize with the node that the user is currently connected to (local node). Can detect from the node list: the one matching the current backend connection.

When submitting, include `target_node_id` in the gRPC request. The backend gateway proxies the write to the target node.

Note: the current `CreatePeerRequest` doesn't have a `target_node_id` field. The gateway determines the target from the `origin_node_id` on the Peer itself. Make sure `origin_node_id` is set to the selected node's ID when creating.

- [ ] **Step 2: TypeScript type-check**

```bash
cd frontend && pnpm exec tsc --noEmit 2>&1 | tail -10
```

- [ ] **Step 3: Commit**

```bash
git add frontend/src/peers/PeerForm.tsx
git commit -m "feat: add target node selector to PeerForm"
```

---

### Task 15: Periodic anti-entropy node exchange

**Files:**
- Modify: `src/main.rs` (add background task)

- [ ] **Step 1: Add periodic ExchangeNodes background task**

After the health check task (Task 8) in `main.rs`, add a new `tokio::spawn`:

```rust
let sync_ct = cancel.clone();
let sync_interval = sync_interval;
let node_repo_sync = state.node_repo.clone();
let cluster_key_sync = cluster_key.clone();
let listen_addr_sync = listen_addr.clone();

tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(sync_interval));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = sync_ct.cancelled() => break,
            _ = interval.tick() => {}
        }

        let nodes = match node_repo_sync.list_all().await {
            Ok(n) => n,
            Err(_) => continue,
        };

        // Pick a random online peer (excluding self)
        let online_peers: Vec<_> = nodes
            .iter()
            .filter(|n| n.online && n.listen_addr != listen_addr_sync)
            .collect();

        if online_peers.is_empty() {
            continue;
        }

        let peer = online_peers[fastrand::usize(..online_peers.len())];

        let my_info: Vec<NodeInfo> = nodes
            .iter()
            .map(|n| NodeInfo {
                name: n.name.clone(),
                listen_addr: n.listen_addr.clone(),
                local_asn: n.local_asn,
                description: n.description.clone().unwrap_or_default(),
                last_seen_at: n.last_seen_at.clone(),
            })
            .collect();

        match ClusterAggregator::exchange_with(
            &peer.listen_addr,
            &cluster_key_sync,
            my_info,
        )
        .await
        {
            Ok(remote_nodes) => {
                for info in &remote_nodes {
                    if info.listen_addr == listen_addr_sync {
                        continue;
                    }
                    if let Ok(Some(_)) = node_repo_sync
                        .find_by_listen_addr(&info.listen_addr)
                        .await
                    {
                        continue;
                    }
                    let _ = node_repo_sync
                        .create(
                            &info.name,
                            &info.listen_addr,
                            info.local_asn,
                            Some(&info.description),
                        )
                        .await;
                }
            }
            Err(e) => {
                tracing::debug!("Periodic ExchangeNodes with {} failed: {}", peer.listen_addr, e);
            }
        }
    }
});
```

Note: Uses `fastrand` for random peer selection. Add `fastrand = "2"` to `Cargo.toml` dependencies, or use a simpler approach: just pick the first online peer (which is fine for small clusters). If avoiding new deps, use:

```rust
use std::time::{SystemTime, UNIX_EPOCH};
let idx = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .subsec_nanos() as usize % online_peers.len();
let peer = online_peers[idx];
```

- [ ] **Step 2: Build**

```bash
source "$HOME/.cargo/env" && cargo build 2>&1 | tail -10
```

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: add periodic anti-entropy node exchange task"
```

---

### Task 16: Write proxy — forward CreatePeer/UpdatePeer to target node

**Files:**
- Modify: `src/grpc/peer_service.rs` (add proxy logic)

- [ ] **Step 1: Add proxy support to PeerServiceImpl**

In `src/grpc/peer_service.rs`, add fields to the struct:

```rust
pub struct PeerServiceImpl {
    pub peer_repo: PeerRepository,
    pub node_repo: NodeRepository,
    pub cluster_key: std::sync::Arc<String>,
    pub listen_addr: String,
}
```

Import the generated client and cluster auth:

```rust
use crate::cluster::auth::check_cluster_key;
use crate::grpc::generated::cluster_service_client::ClusterServiceClient;
use crate::grpc::generated::PushPeerRequest;
```

- [ ] **Step 2: Add proxy helper method**

```rust
impl PeerServiceImpl {
    async fn proxy_push_peer(
        &self,
        target_addr: &str,
        peer: generated::Peer,
    ) -> Result<generated::Peer, tonic::Status> {
        let uri = format!("http://{}", target_addr);
        let channel = tonic::transport::Channel::from_shared(uri)
            .map_err(|e| tonic::Status::internal(format!("invalid uri: {e}")))?
            .connect()
            .await
            .map_err(|e| tonic::Status::internal(format!("connect failed: {e}")))?;
        let mut client = ClusterServiceClient::new(channel);

        let mut req = tonic::Request::new(PushPeerRequest {
            peer: Some(peer.clone()),
            origin_node_id: peer.origin_node_id.clone(),
        });

        if !self.cluster_key.is_empty() {
            if let Ok(v) = tonic::metadata::Ascii::from_str(&self.cluster_key) {
                req.metadata_mut().insert("x-cluster-key", v);
            }
        }

        let resp = client
            .push_peer(req)
            .await
            .map_err(|e| tonic::Status::internal(format!("proxy push failed: {e}")))?;

        Ok(resp.into_inner().peer.unwrap_or(peer))
    }
}
```

- [ ] **Step 3: Modify create_peer to proxy when target is remote**

In the `create_peer` method, after validation but before local DB insert, check if origin_node_id points to a remote node:

```rust
async fn create_peer(
    &self,
    request: tonic::Request<CreatePeerRequest>,
) -> Result<tonic::Response<CreatePeerResponse>, tonic::Status> {
    let proto = request.into_inner().peer.unwrap_or_default();
    let origin = proto.origin_node_id.clone();

    // If targeting a remote node, proxy
    if !origin.is_empty() && origin != self.listen_addr {
        // Find target node's listen_addr
        let target = self
            .node_repo
            .find_by_listen_addr(&origin)
            .await
            .map_err(|_| tonic::Status::not_found("target node not found"))?
            .ok_or_else(|| tonic::Status::not_found("target node not found"))?;

        let proxied = self.proxy_push_peer(&target.listen_addr, proto).await?;
        return Ok(tonic::Response::new(CreatePeerResponse {
            peer: Some(proxied),
        }));
    }

    // Local create (existing logic follows)
    // ... rest of existing create_peer body
}
```

Same pattern for `update_peer`: if origin_node_id is remote, proxy instead of local update.

- [ ] **Step 4: Update PeerServiceImpl constructor in main.rs**

Find PeerServiceImpl construction (around line 237):

```rust
let peer_svc = PeerServiceImpl {
    peer_repo: state.peer_repo.clone(),
    node_repo: state.node_repo.clone(),
    cluster_key: Arc::new(cluster_key.clone()),
    listen_addr: listen_addr.clone(),
};
```

- [ ] **Step 5: Build**

```bash
source "$HOME/.cargo/env" && cargo build 2>&1 | tail -20
```

Expected: compile errors for existing callers that construct PeerServiceImpl without the new fields. Fix all construction sites.

- [ ] **Step 6: Commit**

```bash
git add src/grpc/peer_service.rs src/main.rs
git commit -m "feat: add write proxy for remote peer create/update"
```

---

### Task 17: Proto-to-model conversion helpers in aggregator

**Files:**
- Modify: `src/cluster/aggregator.rs` (fix conversion calls to use real codebase patterns)

- [ ] **Step 1: Fix conversion calls in aggregator**

The current aggregator uses `Peer::try_from_proto(&p)`, `ProbeResult::from_proto(&r)`, `CommunityRule::from_proto(&r)` which don't exist. Replace with real patterns.

For `fanout_peers`, use `apply_proto` on a default Peer:

```rust
let peers: Vec<Peer> = response
    .into_inner()
    .peers
    .into_iter()
    .filter_map(|p| {
        let mut peer = Peer::default(); // need Default impl or create empty
        peer.apply_proto(&p);
        Some(peer)
    })
    .collect();
```

But `Peer` probably doesn't implement `Default`. Better approach: add a `Peer::from_proto(p: &generated::Peer) -> Self` constructor. Add to `src/models/peer.rs`:

```rust
impl Peer {
    pub fn from_proto(proto: &generated::Peer) -> Self {
        let mut peer = Self {
            id: String::new(),
            name: String::new(),
            asn: 0,
            // ... all fields with defaults or empty
        };
        peer.apply_proto(proto);
        peer
    }
}
```

Actually, this is tedious with 33 fields. A simpler approach: just read `pull_peers` response as raw proto and pass through, or add `#[derive(Default)]` to Peer and then use `apply_proto`.

Check if `Peer` derives `Default`. If not, add `Default` to the derive list. If field types don't support Default (e.g., some String fields), provide manual defaults.

Simplest fix: in the aggregator, just pass the proto Peer objects to the response directly without converting to the model layer. The aggregator can work with proto types:

```rust
pub struct NodeCacheEntry {
    pub peers: Vec<generated::Peer>,          // proto, not model
    pub probe_results: Vec<generated::ProbeResult>,
    pub community_rules: Vec<generated::CommunityRule>,
    pub fetched_at: Instant,
    pub stale: bool,
}
```

This avoids needing model-level conversions entirely — the cache stores proto objects, and the fan-out returns merged proto lists. This is simpler and avoids the conversion problem.

Revisit: change `ClusterCache` and `AggregatedResult` to use generated proto types instead of model types. This means the cache module needs access to `crate::grpc::generated::*`.

Update `src/cluster/cache.rs`:

```rust
use crate::grpc::generated::{Peer, ProbeResult, CommunityRule};
```

Update `src/cluster/aggregator.rs` accordingly.

This is the cleanest approach — no model conversions needed in the aggregator.

- [ ] **Step 2: Build**

```bash
source "$HOME/.cargo/env" && cargo build 2>&1 | tail -20
```

- [ ] **Step 3: Commit**

```bash
git add src/cluster/cache.rs src/cluster/aggregator.rs
git commit -m "fix: use generated proto types in cache and aggregator to avoid conversion friction"
```

---

### Task 18: Integration — full build and test

**Files:**
- All modified files

- [ ] **Step 1: Full cargo build**

```bash
source "$HOME/.cargo/env" && cargo build 2>&1 | tail -10
```

Expected: `Finished`.

- [ ] **Step 2: Run all unit tests**

```bash
source "$HOME/.cargo/env" && cargo test 2>&1
```

Expected: all tests pass.

- [ ] **Step 3: Run clippy**

```bash
source "$HOME/.cargo/env" && cargo clippy 2>&1 | tail -20
```

Expected: no warnings.

- [ ] **Step 4: Frontend type-check**

```bash
cd frontend && pnpm exec tsc --noEmit 2>&1
```

Expected: no errors.

- [ ] **Step 5: Frontend build**

```bash
cd frontend && pnpm run build 2>&1 | tail -10
```

Expected: builds successfully.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: final integration fixes for distributed HA cluster"
```
