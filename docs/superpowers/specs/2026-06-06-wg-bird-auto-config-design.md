# WG + BIRD Auto-Config & Per-Peer Interface Design

**Date:** 2026-06-06
**Status:** Approved
**Scope:** Backend (Rust) + Frontend (React/TypeScript)

## Problem Statement

The current WG+BIRD configuration system has several UX and correctness issues:

1. **Settings changes don't trigger re-apply.** `save_settings` persists to DB but never sets `config_dirty`, so changes to BFD, community filters, import limits, etc. require a subsequent peer change to take effect.

2. **All peers share a single WG interface.** `apply_wg_bird` writes every enabled peer into `/etc/wireguard/wg0.conf`, ignoring each peer's `wg_interface_name` field. This contradicts the DN42 standard ("one interface per peering") and prevents per-peer listen port, MTU, and keypair isolation.

3. **Peer form requires manual entry for boilerplate fields.** Listen Port, Interface Name, and Local ASN should be auto-populated from global Settings.

4. **No apply status visibility.** Users cannot see when configs were last applied, whether changes are pending, or manually trigger an apply.

## Design

### 1. Settings Change Immediate Apply

**File:** `src/grpc/settings_service.rs`

Inject `PeerState`, `listen_addr`, and `SqlitePool` into `SettingsServiceImpl`. After successful persist, call `apply_wg_bird` immediately.

```rust
pub struct SettingsServiceImpl {
    pub settings_repo: SettingsRepository,
    pub jwt_secret: Arc<String>,
    // New fields for immediate apply
    pub peer_state: PeerState,
    pub listen_addr: String,
    pub pool: SqlitePool,
}
```

**save_settings flow:**
```
validate settings → persist to DB → apply_wg_bird → return settings
```

If `apply_wg_bird` fails, settings are already saved. Return the settings with a warning log; do not roll back. The config state is correct even if the apply execution failed.

**`apply_wg_bird` signature change:** Add `apply_status: &ApplyStatus` parameter so the function can update `last_apply_at`, `last_apply_error`, `managed_interfaces` on success/failure. The `pending` flag is set to `true` before calling and cleared to `false` after.

**Registration change in `main.rs`:** Pass `peer_state`, `listen_addr`, and `pool` when constructing `SettingsServiceImpl`.

### 2. Per-Peer WG Interface (DN42 Standard)

**File:** `src/grpc/peer_service.rs` — `apply_wg_bird`

Rewrite the WireGuard section of `apply_wg_bird` to iterate over each enabled peer individually instead of generating a single combined config.

```rust
for peer in peers.iter().filter(|p| p.enabled) {
    let iface = if peer.wg_interface_name.is_empty() {
        "wg0".to_string()
    } else {
        peer.wg_interface_name.clone()
    };
    let conf_path = format!("/etc/wireguard/{iface}.conf");
    let config = generate_config(peer, &settings);

    // Atomic write: tmp → chmod 0600 → rename
    atomic_write(&conf_path, &config)?;

    // Apply: create interface if new, syncconf if existing
    if !interface_exists(&iface).await {
        create_wg_interface(&iface, &conf_path).await?;
    } else {
        apply_syncconf(&iface, &conf_path).await?;
    }
}
```

**New helper functions in `src/services/wireguard.rs`:**

- `interface_exists(name: &str) -> bool` — checks `/sys/class/net/<name>` existence
- `create_wg_interface(iface: &str, conf_path: &str)` — `ip link add <iface> type wireguard` + `wg setconf <iface> <conf_path>` + `ip link set <iface> up`
- `remove_wg_interface(iface: &str)` — `wg-quick down <iface>` + remove conf file

**Peer deletion cleanup** (`src/grpc/peer_service.rs` — `delete_peer`):
After deleting the peer from DB, call `remove_wg_interface` to clean up the WG interface and conf file.

**BIRD config unchanged.** `generate_full_config` already uses `peer.wg_interface_name` in the `%` syntax for BIRD neighbor lines. No changes needed.

### 3. Peer Form Auto-Fill

**File:** `frontend/src/components/peers/PeerForm.tsx`

Auto-populate three fields from Settings when creating a new peer:

| Field | Source | Default |
|-------|--------|---------|
| `wg_listen_port` | `settings.wgDefaultListenPort` | 42420 |
| `wg_interface_name` | Auto-generated from peer name | `wg-<sanitized-name>` |
| `local_asn` | `settings.localAsn` | 4242420000 |

**Interface name generation rule:**
```
peer name → lowercase → replace non-alphanumeric with '-' → prepend "wg-" → truncate to 15 chars
Examples:
  "My DN42 Peer" → "wg-my-dn42-peer"
  "kioubit" → "wg-kioubit"
```

**Auto-fill timing:**
- New peer: useEffect loads defaults from settings on mount
- Edit peer: keep existing values (do not overwrite user's previous choices)

**UI change:** Wrap Listen Port, Interface Name, and Local ASN in a collapsible `<details>` element labeled "Advanced Settings". These fields are hidden by default.

### 4. Config Status Card

**File:** `frontend/src/components/status/StatusPage.tsx`

Add a "Config Status" card alongside existing WG Status and BIRD Status cards.

**Card displays:**
| Field | Description |
|-------|-------------|
| Last Apply | Timestamp of last successful apply (RFC3339) |
| Status | `synced` / `pending` / `error` |
| Error | Last error message (only shown when status is `error`) |
| Interfaces | List of managed WG interfaces and their state |

**"Apply Now" button** triggers `ApplyConfigNow` RPC and refreshes status.

**Backend — `src/app_state.rs`:**
```rust
pub struct ApplyStatus {
    pub last_apply_at: Arc<Mutex<Option<String>>>,
    pub last_apply_error: Arc<Mutex<Option<String>>>,
    pub pending: Arc<AtomicBool>,
    pub managed_interfaces: Arc<Mutex<Vec<String>>>,
}
```

Updated by `apply_wg_bird` on success/failure. Stored in `AppState`.

**Proto additions (`proto/peerman.proto`):**
```protobuf
message ApplyStatus {
  string last_apply_at = 1;
  bool pending = 2;
  string last_error = 3;
  repeated string managed_interfaces = 4;
}

message GetApplyStatusRequest {}
message GetApplyStatusResponse {
  ApplyStatus status = 1;
}

message ApplyConfigNowRequest {}
message ApplyConfigNowResponse {}

service ManagementService {
  // ...existing...
  rpc GetApplyStatus(GetApplyStatusRequest) returns (GetApplyStatusResponse);
  rpc ApplyConfigNow(ApplyConfigNowRequest) returns (ApplyConfigNowResponse);
}
```

### 5. Edge Cases & Validation

**Interface name uniqueness:** Multiple peers with the same `wg_interface_name` causes conf file conflicts. Add a `validate_peer_interface_unique` check in `PeerServiceImpl::create_peer` and `update_peer` (before the DB write), querying `peer_repo` for existing peers with the same `wg_interface_name` and excluding the current peer's ID. This is a service-layer check, not in `validation.rs`, because it requires DB access.

**Interface name length:** Linux `IFNAMSIZ` = 15 characters. `validate_wg_interface_name` already exists; ensure auto-generated names are truncated to 15 chars.

**Concurrent apply:** Settings save triggers immediate apply while the debounce task may also fire. Use an `AtomicBool` apply lock in `apply_wg_bird` to prevent concurrent execution.

**Empty enabled peers:** When all peers are disabled, do not delete existing conf files or interfaces. Only process enabled peers.

**Existing interfaces not managed by peerman:** Do not touch interfaces that were not created by peerman. Only operate on interfaces corresponding to enabled peers.

## Files Changed

| Module | File | Change |
|--------|------|--------|
| Backend - Settings apply | `src/grpc/settings_service.rs` | Inject PeerState/pool, immediate apply after save |
| Backend - Settings registration | `src/main.rs` | Pass new deps to SettingsServiceImpl |
| Backend - Multi-interface WG | `src/grpc/peer_service.rs` | Rewrite WG section of `apply_wg_bird`; delete cleanup |
| Backend - WireGuard helpers | `src/services/wireguard.rs` | New: `interface_exists`, `create_wg_interface`, `remove_wg_interface` |
| Backend - Apply status | `src/app_state.rs` | New: `ApplyStatus` struct |
| Backend - Proto | `proto/peerman.proto` | New: `ApplyStatus`, `GetApplyStatus`, `ApplyConfigNow` messages + RPCs |
| Backend - Management | `src/grpc/management_service.rs` | Implement `GetApplyStatus`, `ApplyConfigNow` |
| Backend - Validation | `src/grpc/peer_service.rs` | Interface name uniqueness check (service-layer, needs DB) |
| Frontend - Auto-fill | `frontend/src/components/peers/PeerForm.tsx` | Auto-populate from Settings, collapsible Advanced |
| Frontend - Status | `frontend/src/components/status/StatusPage.tsx` | New Config Status card |
| Frontend - Hooks | `frontend/src/hooks/` | New: `useApplyStatus`, `useApplyConfigNow` |
| Frontend - Proto stubs | `frontend/src/lib/peerman_pb.ts` | Regenerate |
