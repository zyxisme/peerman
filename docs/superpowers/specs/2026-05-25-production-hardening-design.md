# Production Hardening Design

## Scope

Fix 8 issues identified in the production readiness audit (2026-05-25), grouped into three areas: auth, cluster completeness, and code quality.

## Group A: Auth Hardening

### A1 — Add auth to sensitive read endpoints

**Files:** `src/grpc/peer_service.rs`

Add `crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;` as the first line in these 4 handlers:

| Endpoint | Line |
|----------|------|
| `get_wire_guard_config` | 279 |
| `get_bird_config` | 299 |
| `export_all_wire_guard` | 319 |
| `export_all_bird` | 344 |

`PeerServiceImpl` already has `jwt_secret: Arc<String>`, no struct changes needed.

Status endpoints (`GetWireGuardStatus`, `GetBirdStatus`) remain public — they expose only public keys and protocol state, not private keys.

### A5 — Frontend ProtectedRoute gaps

**Files:** `frontend/src/App.tsx`

- Wrap `/export` (line 53) in `<ProtectedRoute>`
- Wrap `/communities` (line 60) in `<ProtectedRoute>`

### A6 — Hard-fail on empty password

**Files:** `src/main.rs` lines 195-197

Change the warning to a hard error:
```rust
if cfg.auth.password.is_empty() {
    anyhow::bail!(
        "auth.password must be set in config. Empty password is not allowed."
    );
}
```

## Group B: Cluster Completeness

### B1 — Cross-node BIRD command execution

**Files:** `src/grpc/bird_service.rs`

Replace the stub at lines 35-41 with actual cross-node gRPC call:
- Look up the target node's `listen_addr` from `node_repo`
- Build a `BirdServiceClient` via `tonic::transport::Endpoint`
- Proxy the `ExecuteCommand` request to the remote node
- Return the remote node's results

`BirdServiceImpl` gains `node_repo: NodeRepository` and `cluster_key: Arc<String>` fields.

Also extend `run_traceroute` to fan-out across all online nodes when no specific target is given, returning per-node results.

### B2 — Wire ClusterAggregator fanout into list handlers

**Files:** `src/grpc/cluster_service.rs`, `src/main.rs`

In `main.rs`: create a `ClusterCache` and `ClusterAggregator` instance, pass to `ClusterServiceImpl`.

In `ClusterServiceImpl`: add `aggregator: ClusterAggregator` field. In `list_probe_results` and `list_community_rules` handlers, call the corresponding `fanout_*` method after querying local data, merging results.

`list_nodes` is skipped — node table is already synced cluster-wide via `ExchangeNodes`.

## Group C: Code Quality

### C4 — Integration tests

**Files:** new files in `tests/`

| File | Coverage |
|------|----------|
| `tests/auth_test.rs` | login/logout/me endpoints, JWT issuance, empty-password rejection |
| `tests/peer_api_test.rs` | Peer CRUD flow, auth rejection on write endpoints, auth rejection on config export |
| `tests/cluster_test.rs` | Node registration, ExchangeNodes, cluster-key auth |

Use `axum::test` + tonic in-memory channel. Test DB uses `sqlx::SqlitePool` with `:memory:`.

### C7 — Configurable system paths

**Files:** `src/config.rs`, `src/main.rs`, `src/grpc/peer_service.rs`, `src/cluster/tunnel.rs`, `src/services/bird.rs`, `src/services/bird_socket.rs`

Add to `StorageConfig`:
```rust
#[serde(default = "default_wg_config_path")]
pub wg_config_path: String,       // "/etc/wireguard/wg0.conf"
#[serde(default = "default_bird_config_path")]
pub bird_config_path: String,     // "/etc/bird/bird.conf"
#[serde(default = "default_bird_socket_path")]
pub bird_socket_path: String,     // "/var/run/bird.ctl"
```

Replace hardcoded paths across the codebase with reads from `APP_CONFIG.get()`.

### C8 — Remove deprecated chrono API

**Files:** `src/models/flap_event.rs` line 139

Replace `date_naive()` + manual time construction with:
```rust
let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
```
This is simpler, avoids the deprecated API, and matches the RFC3339 string format used in the SQL query on line 132.
