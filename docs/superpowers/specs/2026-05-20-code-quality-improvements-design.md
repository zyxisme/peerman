# Peerman Code Quality Improvements — Design

## Scope

13 improvements across 3 phases, touching all layers of the Rust backend and some frontend.

## Phase 1: Safety & Robustness

### 1.1 Eliminate `.unwrap()` calls (8 locations)

- `probe.rs`: Two `Regex::new(...).unwrap()` → `lazy_static!` or `OnceLock<Regex>`
- `bgp_listener.rs`: `build_bgp_msg(4, &[]).unwrap()` — infallible case, document with `.expect("keepalive never fails")`
- `bird.rs`: 4 `.unwrap()` on `Option` → `ok_or_else(|| AppError::...)` with descriptive error
- `flap_event.rs`: 2 `.unwrap()` on datetime → `.unwrap_or_default()` or error propagation

### 1.2 Async ping

Replace `std::process::Command` in `probe.rs:ping()` with `tokio::process::Command`.
Remove the stale comment about "timeout is handled by the caller" since the caller doesn't timeout.

### 1.3 Graceful shutdown

Add `tokio-util` dependency for `CancellationToken`.
Propagate token to: stale-node cleanup task, probe task, flap detector task.
Main function awaits SIGINT via `tokio::signal::ctrl_c()`, cancels token, waits for tasks.

### 1.4 FlapDetector memory bound

Add TTL-based eviction to PrefixTracker HashMap. Trackers with no changes for `resolve_after_secs * 2` are removed on each tick.

## Phase 2: Code Quality & Performance

### 2.1 Proto conversion traits

Add `From<&PeerModel> for ProtoPeer` impl in `models/peer.rs`.
Remove `apply_create_fields()` and `apply_proto_to_model()`.
Keep `peer_to_proto()` as a thin forwarding delegate.

### 2.2 Atomic upsert for community rules

Replace check-then-insert-or-update in `CommunityRuleRepository::save()` with:
```sql
INSERT INTO community_rules (...) VALUES (...)
ON CONFLICT(id) DO UPDATE SET ...
```

### 2.3 Deduplicate community_mapper probe queries

Query `latest_between` once, destructure both `avg_latency_ms` and `packet_loss_pct` from the same row.

### 2.4 Missing database indexes

Migration `004_indexes.sql`:
- `CREATE INDEX IF NOT EXISTS idx_nodes_listen_addr ON nodes(listen_addr)`
- `CREATE INDEX IF NOT EXISTS idx_probe_results_from_to_probed ON probe_results(from_node_id, to_node_id, probed_at DESC)`
- `CREATE INDEX IF NOT EXISTS idx_flap_events_prefix_node_active ON flap_events(prefix, node_id, active)`

### 2.5 Dynamic SQL for list_by_filters

Replace 4-branch `if/else` in `ProbeResultRepository::list_by_filters()` with dynamically-built WHERE clauses using `sqlx::QueryBuilder`.

## Phase 3: Tests & Operations

### 3.1 Unit tests

Add `#[cfg(test)] mod tests` to:
- `validation.rs` — test all 6 validation functions
- `wireguard.rs` — test `generate_keypair` output format
- `bird.rs` — test `sanitize_name`, `generate_peer_block` with various peer configs
- `probe.rs` — test `resolve_target_ip`, ping output parsing

### 3.2 Frontend error boundary

Add `ErrorBoundary.tsx` component wrapping `<Routes>` in `App.tsx`.

### 3.3 .gitignore

Add: `data/`, `*.db`, `.idea/`, `*.swp`, `.DS_Store`

### 3.4 README sync

Update gRPC API section to match current `peerman.proto` definitions.

## What we do NOT do

- No authentication/authorization (out of scope for now)
- No CORS production config (depends on deployment topology)
- No CI/CD (no repo platform info)
- No BirdSocket reconnect (requires broader redesign)
- No WireGuard key encryption at rest (requires KMS/infrastructure)
