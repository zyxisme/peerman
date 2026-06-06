# WG + BIRD Auto-Config & Per-Peer Interface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement per-peer WG interfaces (DN42 standard), settings immediate apply, peer form auto-fill, and config status visibility.

**Architecture:** Per-peer WireGuard interfaces replace the single `wg0.conf` model. `apply_wg_bird` iterates each enabled peer, generating and applying independent conf files. Settings save triggers immediate apply. `ApplyStatus` tracks last apply time/errors for frontend visibility.

**Tech Stack:** Rust (tonic, axum, sqlx), React (TypeScript, Tailwind CSS), protobuf/gRPC-Web

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/app_state.rs` | Add `ApplyStatus` struct (shared apply state) |
| `src/services/wireguard.rs` | Add `interface_exists`, `create_wg_interface`, `remove_wg_interface` |
| `src/grpc/peer_service.rs` | Rewrite `apply_wg_bird` for per-peer interfaces; add interface uniqueness check |
| `src/grpc/settings_service.rs` | Inject PeerState/pool, trigger immediate apply on save |
| `src/grpc/management_service.rs` | Implement `GetApplyStatus`, `ApplyConfigNow` RPCs |
| `src/main.rs` | Wire new deps into SettingsServiceImpl |
| `proto/peerman.proto` | Add ApplyStatus messages + ManagementService RPCs |
| `frontend/src/lib/peerman_pb.ts` | Regenerated TS proto stubs |
| `frontend/src/hooks/useManagement.ts` | Add `useApplyStatus`, `useApplyConfigNow` |
| `frontend/src/components/peers/PeerForm.tsx` | Auto-fill from settings, collapsible advanced section |
| `frontend/src/components/status/StatusPage.tsx` | New Config Status card |

---

### Task 1: ApplyStatus Struct and Proto Additions

**Files:**
- Modify: `src/app_state.rs`
- Modify: `proto/peerman.proto`

- [ ] **Step 1: Add ApplyStatus struct to app_state.rs**

Add after the existing `AppState` struct in `src/app_state.rs`:

```rust
use std::sync::atomic::AtomicBool;
use tokio::sync::Mutex;

/// Shared state tracking the last WG+BIRD config apply result.
#[derive(Clone)]
pub struct ApplyStatus {
    pub last_apply_at: Arc<Mutex<Option<String>>>,
    pub last_apply_error: Arc<Mutex<Option<String>>>,
    pub pending: Arc<AtomicBool>,
    pub managed_interfaces: Arc<Mutex<Vec<String>>>,
}

impl ApplyStatus {
    pub fn new() -> Self {
        Self {
            last_apply_at: Arc::new(Mutex::new(None)),
            last_apply_error: Arc::new(Mutex::new(None)),
            pending: Arc::new(AtomicBool::new(false)),
            managed_interfaces: Arc::new(Mutex::new(Vec::new())),
        }
    }
}
```

Add `apply_status: ApplyStatus` field to `AppState`. Initialize it in `AppState::new()`.

- [ ] **Step 2: Add proto messages and RPCs**

Add to `proto/peerman.proto` before the `ManagementService` definition:

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
```

Add to the `ManagementService` definition:

```protobuf
service ManagementService {
  rpc GetWireGuardStatus(GetWGStatusRequest) returns (WGStatusResponse);
  rpc GetBirdStatus(GetBirdStatusRequest) returns (BirdStatusResponse);
  rpc GetApplyStatus(GetApplyStatusRequest) returns (GetApplyStatusResponse);
  rpc ApplyConfigNow(ApplyConfigNowRequest) returns (ApplyConfigNowResponse);
}
```

- [ ] **Step 3: Run cargo check to verify proto compiles**

Run: `source "$HOME/.cargo/env" && cargo check 2>&1 | tail -20`
Expected: Compilation succeeds (may have warnings about unused fields).

- [ ] **Step 4: Commit**

```bash
git add src/app_state.rs proto/peerman.proto
git commit -m "feat: add ApplyStatus struct and proto definitions for config apply tracking"
```

---

### Task 2: WireGuard Helper Functions

**Files:**
- Modify: `src/services/wireguard.rs`

- [ ] **Step 1: Add tests for new helper functions**

Add at the end of the `tests` module in `src/services/wireguard.rs`:

```rust
#[test]
fn test_interface_exists_returns_false_for_nonexistent() {
    // This test runs without root — just verifies the function doesn't panic
    let exists = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(interface_exists("wg-nonexistent-999"));
    assert!(!exists);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `source "$HOME/.cargo/env" && cargo test --lib services::wireguard::tests::test_interface_exists_returns_false_for_nonexistent 2>&1 | tail -10`
Expected: FAIL — `interface_exists` not found.

- [ ] **Step 3: Implement interface_exists**

Add to `src/services/wireguard.rs` (before `generate_config`):

```rust
/// Check if a network interface exists by looking at /sys/class/net/<name>.
pub async fn interface_exists(name: &str) -> bool {
    std::path::Path::new(&format!("/sys/class/net/{name}")).exists()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `source "$HOME/.cargo/env" && cargo test --lib services::wireguard::tests::test_interface_exists_returns_false_for_nonexistent 2>&1 | tail -10`
Expected: PASS

- [ ] **Step 5: Add test for create_wg_interface**

```rust
#[test]
fn test_create_wg_interface_is_async_and_compiles() {
    // Verify the function signature compiles — actual creation requires root
    fn _assert_sig(name: &str, conf: &str) -> impl std::future::Future<Output = Result<(), crate::error::AppError>> + '_ {
        create_wg_interface(name, conf)
    }
}
```

- [ ] **Step 6: Run test to verify it fails**

Run: `source "$HOME/.cargo/env" && cargo test --lib services::wireguard::tests::test_create_wg_interface_is_async_and_compiles 2>&1 | tail -10`
Expected: FAIL — `create_wg_interface` not found.

- [ ] **Step 7: Implement create_wg_interface**

```rust
/// Create a new WireGuard interface and apply its config.
/// Equivalent to: `ip link add <iface> type wireguard && wg setconf <iface> <conf> && ip link set <iface> up`
pub async fn create_wg_interface(iface: &str, conf_path: &str) -> Result<(), AppError> {
    let output = tokio::process::Command::new("ip")
        .args(["link", "add", iface, "type", "wireguard"])
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("ip link add failed: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Internal(format!(
            "ip link add {iface} failed: {stderr}"
        )));
    }

    let output = tokio::process::Command::new("wg")
        .args(["setconf", iface, conf_path])
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("wg setconf failed: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Internal(format!(
            "wg setconf {iface} failed: {stderr}"
        )));
    }

    let output = tokio::process::Command::new("ip")
        .args(["link", "set", iface, "up"])
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("ip link set up failed: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Internal(format!(
            "ip link set {iface} up failed: {stderr}"
        )));
    }

    Ok(())
}
```

- [ ] **Step 8: Run test to verify it passes**

Run: `source "$HOME/.cargo/env" && cargo test --lib services::wireguard::tests::test_create_wg_interface_is_async_and_compiles 2>&1 | tail -10`
Expected: PASS

- [ ] **Step 9: Add test for remove_wg_interface**

```rust
#[test]
fn test_remove_wg_interface_is_async_and_compiles() {
    fn _assert_sig(name: &str) -> impl std::future::Future<Output = Result<(), crate::error::AppError>> + '_ {
        remove_wg_interface(name)
    }
}
```

- [ ] **Step 10: Implement remove_wg_interface**

```rust
/// Remove a WireGuard interface and its config file.
/// Runs `wg-quick down <iface>` (ignoring errors if already down), then removes the conf file.
pub async fn remove_wg_interface(iface: &str) -> Result<(), AppError> {
    // Try wg-quick down (ignore errors — interface may already be down)
    let _ = tokio::process::Command::new("wg-quick")
        .args(["down", iface])
        .output()
        .await;

    // Remove conf file if it exists
    let conf_path = format!("/etc/wireguard/{iface}.conf");
    if std::path::Path::new(&conf_path).exists() {
        std::fs::remove_file(&conf_path)
            .map_err(|e| AppError::Internal(format!("Cannot remove {conf_path}: {e}")))?;
    }

    Ok(())
}
```

- [ ] **Step 11: Run all wireguard tests**

Run: `source "$HOME/.cargo/env" && cargo test --lib services::wireguard 2>&1 | tail -10`
Expected: All tests PASS.

- [ ] **Step 12: Commit**

```bash
git add src/services/wireguard.rs
git commit -m "feat: add WG interface management helpers (exists, create, remove)"
```

---

### Task 3: Per-Peer WG Apply in apply_wg_bird

**Files:**
- Modify: `src/grpc/peer_service.rs`
- Modify: `src/app_state.rs` (if ApplyStatus needs to be passed)

- [ ] **Step 1: Update apply_wg_bird signature to accept ApplyStatus**

In `src/grpc/peer_service.rs`, change the `apply_wg_bird` signature:

```rust
pub async fn apply_wg_bird(
    state: &PeerState,
    listen_addr: &str,
    pool: &sqlx::SqlitePool,
    apply_status: &crate::app_state::ApplyStatus,
) -> Result<(), Status> {
```

- [ ] **Step 2: Add apply lock to prevent concurrent execution**

At the top of `apply_wg_bird`, add:

```rust
    // Prevent concurrent apply
    static APPLY_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    let _guard = APPLY_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
```

- [ ] **Step 3: Set pending flag and track managed interfaces**

After the lock, before the existing peers/settings fetch:

```rust
    apply_status.pending.store(true, std::sync::atomic::Ordering::Relaxed);
```

After fetching peers, collect interface names:

```rust
    let interfaces: Vec<String> = peers
        .iter()
        .filter(|p| p.enabled)
        .map(|p| {
            if p.wg_interface_name.is_empty() {
                "wg0".to_string()
            } else {
                p.wg_interface_name.clone()
            }
        })
        .collect();
    *apply_status.managed_interfaces.lock().await = interfaces;
```

- [ ] **Step 4: Replace the single wg0.conf logic with per-peer loop**

Replace the existing WireGuard section (lines that build `wg_config` from all peers, write to `wg0.conf`, and call `apply_syncconf("wg0", ...)`) with:

```rust
    // 1. WireGuard: per-peer interface apply
    for peer in peers.iter().filter(|p| p.enabled) {
        let iface = if peer.wg_interface_name.is_empty() {
            "wg0".to_string()
        } else {
            peer.wg_interface_name.clone()
        };
        let conf_path = format!("/etc/wireguard/{iface}.conf");
        let tmp_path = format!("{conf_path}.tmp");
        let config = services::wireguard::generate_config(peer, &settings);

        std::fs::write(&tmp_path, &config)
            .map_err(|e| Status::internal(format!("Cannot write {conf_path}: {e}")))?;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| Status::internal(format!("Cannot set permissions on {conf_path}: {e}")))?;
        std::fs::rename(&tmp_path, &conf_path)
            .map_err(|e| Status::internal(format!("Cannot rename {conf_path}: {e}")))?;

        if !services::wireguard::interface_exists(&iface).await {
            services::wireguard::create_wg_interface(&iface, &conf_path)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
        } else {
            services::wireguard::apply_syncconf(&iface, &conf_path)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
        }
    }
```

- [ ] **Step 5: Update ApplyStatus on success/failure**

At the end of `apply_wg_bird`, before `Ok(())`:

```rust
    apply_status.pending.store(false, std::sync::atomic::Ordering::Relaxed);
    *apply_status.last_apply_at.lock().await = Some(chrono::Utc::now().to_rfc3339());
    *apply_status.last_apply_error.lock().await = None;

    Ok(())
```

For the error path (in the callers), add error tracking. Wrap the existing error return at the BIRD section:

```rust
    if let Err(e) = crate::services::bird::apply_config(&bird_config) {
        apply_status.pending.store(false, std::sync::atomic::Ordering::Relaxed);
        *apply_status.last_apply_error.lock().await = Some(e.to_string());
        return Err(Status::internal(e.to_string()));
    }
```

- [ ] **Step 6: Update all callers of apply_wg_bird**

In `src/tasks/apply.rs`, the `spawn_config_apply` function calls `apply_wg_bird`. Update it to pass `apply_status`:

The `spawn_config_apply` function needs access to `ApplyStatus`. Add it as a parameter:

```rust
pub fn spawn_config_apply(
    config_dirty: Arc<AtomicBool>,
    peer_state: PeerState,
    listen_addr: String,
    pool: SqlitePool,
    shutdown: CancellationToken,
    apply_status: crate::app_state::ApplyStatus,
) {
```

Update the call inside:

```rust
    if config_dirty.swap(false, Ordering::Relaxed) {
        tracing::info!("Config dirty flag set, applying WG+BIRD configs...");
        if let Err(e) =
            crate::grpc::peer_service::apply_wg_bird(&peer_state, &listen_addr, &pool, &apply_status).await
        {
            tracing::warn!("Auto-apply WG+BIRD failed: {e}");
        }
    }
```

- [ ] **Step 7: Update main.rs to pass apply_status to spawn_config_apply**

In `src/main.rs`, find the `spawn_config_apply` call and add `state.apply_status.clone()` as the last argument.

- [ ] **Step 8: Run cargo check**

Run: `source "$HOME/.cargo/env" && cargo check 2>&1 | tail -20`
Expected: Compilation succeeds.

- [ ] **Step 9: Commit**

```bash
git add src/grpc/peer_service.rs src/tasks/apply.rs src/main.rs
git commit -m "feat: per-peer WG interface apply with status tracking"
```

---

### Task 4: Peer Deletion Cleanup

**Files:**
- Modify: `src/grpc/peer_service.rs`

- [ ] **Step 1: Add cleanup in delete_peer**

In the `delete_peer` method of `PeerServiceImpl`, fetch the peer before deleting to get its interface name, then clean up after deletion:

```rust
    async fn delete_peer(
        &self,
        request: Request<DeletePeerRequest>,
    ) -> Result<Response<DeletePeerResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();

        // Fetch peer to get interface name before deletion
        let peer = self
            .state
            .peer_repo
            .find_by_id(&req.id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;

        self.state
            .peer_repo
            .delete(&req.id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;

        // Clean up WG interface
        let iface = if peer.wg_interface_name.is_empty() {
            "wg0".to_string()
        } else {
            peer.wg_interface_name.clone()
        };
        if let Err(e) = services::wireguard::remove_wg_interface(&iface).await {
            tracing::warn!("Failed to remove WG interface {iface}: {e}");
        }

        self.config_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);

        Ok(Response::new(DeletePeerResponse {}))
    }
```

- [ ] **Step 2: Run cargo check**

Run: `source "$HOME/.cargo/env" && cargo check 2>&1 | tail -20`
Expected: Compilation succeeds.

- [ ] **Step 3: Commit**

```bash
git add src/grpc/peer_service.rs
git commit -m "feat: clean up WG interface on peer deletion"
```

---

### Task 5: Interface Name Uniqueness Validation

**Files:**
- Modify: `src/grpc/peer_service.rs`

- [ ] **Step 1: Add uniqueness check in create_peer**

In `PeerServiceImpl::create_peer`, after `validate_peer_fields` and before `create_full`, add:

```rust
        // Validate interface name uniqueness
        if !req.wg_interface_name.is_empty() {
            let existing = self
                .state
                .peer_repo
                .list_all()
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            if existing.iter().any(|p| p.wg_interface_name == req.wg_interface_name) {
                return Err(Status::invalid_argument(format!(
                    "Interface name '{}' is already in use by another peer",
                    req.wg_interface_name
                )));
            }
        }
```

- [ ] **Step 2: Add uniqueness check in update_peer**

In `PeerServiceImpl::update_peer`, after `validate_peer_fields` and before `peer.apply_proto`, add:

```rust
        // Validate interface name uniqueness (exclude current peer)
        if !req.wg_interface_name.is_empty() {
            let existing = self
                .state
                .peer_repo
                .list_all()
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            if existing
                .iter()
                .any(|p| p.wg_interface_name == req.wg_interface_name && p.id != req.id)
            {
                return Err(Status::invalid_argument(format!(
                    "Interface name '{}' is already in use by another peer",
                    req.wg_interface_name
                )));
            }
        }
```

- [ ] **Step 3: Run cargo check**

Run: `source "$HOME/.cargo/env" && cargo check 2>&1 | tail -20`
Expected: Compilation succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/grpc/peer_service.rs
git commit -m "feat: validate WG interface name uniqueness on create/update"
```

---

### Task 6: Settings Immediate Apply

**Files:**
- Modify: `src/grpc/settings_service.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Update SettingsServiceImpl struct**

In `src/grpc/settings_service.rs`, update the struct:

```rust
use crate::app_state::PeerState;

pub struct SettingsServiceImpl {
    pub settings_repo: SettingsRepository,
    pub jwt_secret: std::sync::Arc<String>,
    pub peer_state: PeerState,
    pub listen_addr: String,
    pub pool: sqlx::SqlitePool,
    pub apply_status: crate::app_state::ApplyStatus,
}
```

- [ ] **Step 2: Add apply call after save_settings**

In the `save_settings` method, after `self.settings_repo.save(&settings).await` succeeds and before the `Ok(Response::new(...))` return, add:

```rust
        // Immediately apply WG+BIRD configs
        if let Err(e) = crate::grpc::peer_service::apply_wg_bird(
            &self.peer_state,
            &self.listen_addr,
            &self.pool,
            &self.apply_status,
        )
        .await
        {
            tracing::warn!("Settings saved but WG+BIRD apply failed: {e}");
        }
```

- [ ] **Step 3: Update main.rs construction**

In `src/main.rs`, update the `SettingsServiceImpl` construction:

```rust
    let settings_svc = SettingsServiceImpl {
        settings_repo: state.settings_repo.clone(),
        jwt_secret: jwt_secret.clone(),
        peer_state: state.peer_state(),
        listen_addr: listen_addr.clone(),
        pool: pool.clone(),
        apply_status: state.apply_status.clone(),
    };
```

- [ ] **Step 4: Run cargo check**

Run: `source "$HOME/.cargo/env" && cargo check 2>&1 | tail -20`
Expected: Compilation succeeds.

- [ ] **Step 5: Commit**

```bash
git add src/grpc/settings_service.rs src/main.rs
git commit -m "feat: trigger immediate WG+BIRD apply on settings save"
```

---

### Task 7: Management gRPC — GetApplyStatus and ApplyConfigNow

**Files:**
- Modify: `src/grpc/management_service.rs`

- [ ] **Step 1: Update ManagementServiceImpl struct**

```rust
pub struct ManagementServiceImpl {
    pub jwt_secret: std::sync::Arc<String>,
    pub apply_status: crate::app_state::ApplyStatus,
    pub peer_state: crate::app_state::PeerState,
    pub listen_addr: String,
    pub pool: sqlx::SqlitePool,
}
```

- [ ] **Step 2: Implement GetApplyStatus**

```rust
    async fn get_apply_status(
        &self,
        request: Request<super::generated::GetApplyStatusRequest>,
    ) -> Result<Response<super::generated::GetApplyStatusResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;

        let last_apply_at = self
            .apply_status
            .last_apply_at
            .lock()
            .await
            .clone()
            .unwrap_or_default();
        let last_error = self
            .apply_status
            .last_apply_error
            .lock()
            .await
            .clone()
            .unwrap_or_default();
        let pending = self
            .apply_status
            .pending
            .load(std::sync::atomic::Ordering::Relaxed);
        let interfaces = self
            .apply_status
            .managed_interfaces
            .lock()
            .await
            .clone();

        Ok(Response::new(super::generated::GetApplyStatusResponse {
            status: Some(super::generated::ApplyStatus {
                last_apply_at,
                pending,
                last_error,
                managed_interfaces: interfaces,
            }),
        }))
    }
```

- [ ] **Step 3: Implement ApplyConfigNow**

```rust
    async fn apply_config_now(
        &self,
        request: Request<super::generated::ApplyConfigNowRequest>,
    ) -> Result<Response<super::generated::ApplyConfigNowResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;

        crate::grpc::peer_service::apply_wg_bird(
            &self.peer_state,
            &self.listen_addr,
            &self.pool,
            &self.apply_status,
        )
        .await?;

        Ok(Response::new(super::generated::ApplyConfigNowResponse {}))
    }
```

- [ ] **Step 4: Update main.rs construction**

In `src/main.rs`, update the `ManagementServiceImpl` construction:

```rust
    let mgmt_svc = ManagementServiceImpl {
        jwt_secret: jwt_secret.clone(),
        apply_status: state.apply_status.clone(),
        peer_state: state.peer_state(),
        listen_addr: listen_addr.clone(),
        pool: pool.clone(),
    };
```

- [ ] **Step 5: Run cargo check**

Run: `source "$HOME/.cargo/env" && cargo check 2>&1 | tail -20`
Expected: Compilation succeeds.

- [ ] **Step 6: Run all tests**

Run: `source "$HOME/.cargo/env" && cargo test 2>&1 | tail -20`
Expected: All tests PASS.

- [ ] **Step 7: Run clippy and fmt**

Run: `source "$HOME/.cargo/env" && cargo clippy 2>&1 | tail -10 && cargo fmt`
Expected: No warnings.

- [ ] **Step 8: Commit**

```bash
git add src/grpc/management_service.rs src/main.rs
git commit -m "feat: implement GetApplyStatus and ApplyConfigNow RPCs"
```

---

### Task 8: Regenerate Proto Stubs

**Files:**
- Modify: `frontend/src/lib/peerman_pb.ts`

- [ ] **Step 1: Regenerate TS proto stubs**

Run: `cd /home/cc/peerman && PATH="frontend/node_modules/.bin:$PATH" protoc -I proto --es_out frontend/src/lib --es_opt target=ts proto/peerman.proto`

Expected: `frontend/src/lib/peerman_pb.ts` is regenerated with new `ApplyStatus`, `GetApplyStatusRequest`, `GetApplyStatusResponse`, `ApplyConfigNowRequest`, `ApplyConfigNowResponse` types.

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd /home/cc/peerman/frontend && pnpm exec tsc --noEmit 2>&1 | tail -10`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add frontend/src/lib/peerman_pb.ts
git commit -m "chore: regenerate TS proto stubs with ApplyStatus types"
```

---

### Task 9: Peer Form Auto-Fill

**Files:**
- Modify: `frontend/src/components/peers/PeerForm.tsx`
- Modify: `frontend/src/hooks/useSettings.ts` (if needed for settings access)

- [ ] **Step 1: Add settings hook import and auto-fill logic**

In `PeerForm.tsx`, add import at the top:

```tsx
import { useSettings } from '../../hooks/useSettings';
```

Inside the component, add:

```tsx
const { settings } = useSettings();
```

Add a function to generate interface name from peer name:

```tsx
function sanitizeInterfaceName(name: string): string {
  return 'wg-' + name
    .toLowerCase()
    .replace(/[^a-z0-9]/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
    .substring(0, 12); // 15 - 3 for "wg-" prefix
}
```

Add useEffect for auto-fill on new peer (after the existing useEffect for `existingPeer`):

```tsx
useEffect(() => {
  if (!isEdit && settings) {
    setWgListenPort(String(settings.wgDefaultListenPort));
    setLocalAsn(settings.localAsn.toString());
  }
}, [isEdit, settings]);
```

Add useEffect to auto-generate interface name from peer name (for new peers only):

```tsx
useEffect(() => {
  if (!isEdit && name) {
    setWgInterfaceName(sanitizeInterfaceName(name));
  }
}, [isEdit, name]);
```

- [ ] **Step 2: Collapse advanced fields**

Replace the WireGuard fieldset content with a collapsible section. The Private Key, Public Key, Remote Address, Remote Port stay visible. Listen Port and Interface Name move into a `<details>` block:

```tsx
<fieldset className="card">
  <legend className="text-body-sm-strong text-ink mb-md">WireGuard</legend>
  <div className="flex items-center gap-2 mb-sm">
    <button type="button" onClick={handleGenerate} disabled={genLoading} className="btn-secondary-sm">
      <RefreshCw className={`w-3.5 h-3.5 ${genLoading ? 'animate-spin' : ''}`} />
      Generate Keypair
    </button>
  </div>
  <div className="grid grid-cols-1 md:grid-cols-2 gap-sm">
    <Input label="Private Key" value={wgPrivateKey} onChange={setWgPrivateKey} mono />
    <Input label="Public Key" value={wgPublicKey} onChange={setWgPublicKey} mono />
    <Input label="Remote Address" value={wgRemoteAddress} onChange={setWgRemoteAddress} />
    <Input label="Remote Port" value={wgRemotePort} onChange={setWgRemotePort} type="number" />
  </div>
  <details className="mt-sm">
    <summary className="text-caption text-mute cursor-pointer select-none">Advanced Settings</summary>
    <div className="grid grid-cols-1 md:grid-cols-2 gap-sm mt-sm">
      <Input label="Listen Port" value={wgListenPort} onChange={setWgListenPort} type="number" />
      <Input label="Interface Name" value={wgInterfaceName} onChange={setWgInterfaceName} placeholder="wg-peer-name" />
    </div>
  </details>
</fieldset>
```

Also collapse Local ASN in the Identity section:

```tsx
<details className="mt-sm">
  <summary className="text-caption text-mute cursor-pointer select-none">Advanced Identity</summary>
  <div className="grid grid-cols-1 md:grid-cols-2 gap-sm mt-sm">
    <Input label="Local ASN" value={localAsn} onChange={setLocalAsn} />
  </div>
</details>
```

- [ ] **Step 3: Verify TypeScript compiles**

Run: `cd /home/cc/peerman/frontend && pnpm exec tsc --noEmit 2>&1 | tail -10`
Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/peers/PeerForm.tsx
git commit -m "feat: auto-fill peer form from settings, collapse advanced fields"
```

---

### Task 10: Config Status Card

**Files:**
- Modify: `frontend/src/hooks/useManagement.ts`
- Modify: `frontend/src/components/status/StatusPage.tsx`

- [ ] **Step 1: Add useApplyStatus hook**

In `frontend/src/hooks/useManagement.ts`, add:

```tsx
import {
  GetWGStatusRequestSchema,
  GetBirdStatusRequestSchema,
  GetApplyStatusRequestSchema,
  ApplyConfigNowRequestSchema,
} from '../lib/peerman_pb';
import type { WGInterface, BirdProtocol, ApplyStatus } from '../lib/peerman_pb';
```

Add the hook:

```tsx
export function useApplyStatus() {
  const [status, setStatus] = useState<ApplyStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetch = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await mgmtClient.getApplyStatus(
        create(GetApplyStatusRequestSchema, {})
      );
      setStatus(res.status ?? null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);

  return { status, loading, error, refetch: fetch };
}

export function useApplyConfigNow() {
  const [loading, setLoading] = useState(false);

  const apply = useCallback(async () => {
    setLoading(true);
    try {
      await mgmtClient.applyConfigNow(create(ApplyConfigNowRequestSchema, {}));
    } finally {
      setLoading(false);
    }
  }, []);

  return { apply, loading };
}
```

- [ ] **Step 2: Add Config Status card to StatusPage**

In `frontend/src/components/status/StatusPage.tsx`, add imports:

```tsx
import { useWireGuardStatus, useBirdStatus, useApplyStatus, useApplyConfigNow } from '../../hooks/useManagement';
```

Inside the component:

```tsx
const applyStatus = useApplyStatus();
const applyNow = useApplyConfigNow();
```

Update the Refresh button to also refetch apply status:

```tsx
<button
  onClick={() => { wg.refetch(); bird.refetch(); applyStatus.refetch(); }}
  className="btn-ghost text-xs flex items-center gap-1"
>
```

Add the Config Status card after the BIRD card:

```tsx
{/* Config Status */}
<div className="card">
  <h2 className="text-body-md-strong text-ink mb-md">Config Status</h2>
  {applyStatus.loading && <div className="text-body-sm text-body">Loading...</div>}
  {applyStatus.error && <div className="text-body-sm text-error">{applyStatus.error}</div>}
  {!applyStatus.loading && !applyStatus.error && applyStatus.status && (
    <div className="space-y-sm">
      <div className="grid grid-cols-2 gap-sm text-body-sm">
        <div>
          <span className="text-mute">Status:</span>{' '}
          <span className={`badge ${
            applyStatus.status.pending
              ? 'bg-yellow-500/20 text-yellow-500'
              : applyStatus.status.lastError
                ? 'bg-red-500/20 text-red-500'
                : 'bg-green-500/20 text-green-500'
          }`}>
            {applyStatus.status.pending ? 'pending' : applyStatus.status.lastError ? 'error' : 'synced'}
          </span>
        </div>
        <div>
          <span className="text-mute">Last Apply:</span>{' '}
          {applyStatus.status.lastApplyAt || '—'}
        </div>
      </div>
      {applyStatus.status.lastError && (
        <div className="text-body-sm text-error bg-error-soft px-md py-sm rounded-sm">
          {applyStatus.status.lastError}
        </div>
      )}
      {applyStatus.status.managedInterfaces.length > 0 && (
        <div className="text-caption text-mute">
          Managed interfaces: {applyStatus.status.managedInterfaces.join(', ')}
        </div>
      )}
      <button
        onClick={async () => {
          await applyNow.apply();
          applyStatus.refetch();
        }}
        disabled={applyNow.loading}
        className="btn-primary text-sm mt-sm"
      >
        {applyNow.loading ? 'Applying...' : 'Apply Now'}
      </button>
    </div>
  )}
</div>
```

- [ ] **Step 3: Verify TypeScript compiles**

Run: `cd /home/cc/peerman/frontend && pnpm exec tsc --noEmit 2>&1 | tail -10`
Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add frontend/src/hooks/useManagement.ts frontend/src/components/status/StatusPage.tsx
git commit -m "feat: add Config Status card with apply-now button to Status page"
```

---

### Task 11: Final Integration — Build, Test, Lint

- [ ] **Step 1: Run cargo clippy**

Run: `source "$HOME/.cargo/env" && cargo clippy 2>&1 | tail -20`
Expected: No errors or warnings.

- [ ] **Step 2: Run cargo fmt**

Run: `source "$HOME/.cargo/env" && cargo fmt`
Expected: Code formatted.

- [ ] **Step 3: Run cargo test**

Run: `source "$HOME/.cargo/env" && cargo test 2>&1 | tail -20`
Expected: All tests PASS.

- [ ] **Step 4: Run frontend type check**

Run: `cd /home/cc/peerman/frontend && pnpm exec tsc --noEmit 2>&1 | tail -10`
Expected: No errors.

- [ ] **Step 5: Final commit with all formatting**

```bash
git add -A
git commit -m "chore: final formatting and lint fixes for WG/BIRD auto-config"
```
