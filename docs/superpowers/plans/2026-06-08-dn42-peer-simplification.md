# DN42 Peer Simplification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align peer creation with DN42 conventions — one node keypair, auto-derived listen port/link-local, simplified 5-field form.

**Architecture:** Add node-level WG keypair to Settings (auto-generated on first load). Add DN42 convention helper functions for ASN-derived values. Auto-fill peer fields in create/update. Simplify PeerForm to 5 required fields with collapsible advanced options.

**Tech Stack:** Rust (tonic, sqlx, x25519-dalek), TypeScript (React, ConnectRPC), SQLite, protobuf

**Spec:** `docs/superpowers/specs/2026-06-08-dn42-peer-simplification-design.md`

---

### Task 1: Proto + Migration

**Files:**
- Modify: `proto/peerman.proto:71-105` (Settings message)
- Create: `migrations/013_node_wg_keypair.sql`

- [ ] **Step 1: Add fields to proto Settings message**

In `proto/peerman.proto`, add two fields after `confederation_local_asn = 26`:

```protobuf
  // Node WG keypair
  string node_wg_private_key = 27;
  string node_wg_public_key = 28;
```

- [ ] **Step 2: Create migration SQL**

Create `migrations/013_node_wg_keypair.sql`:

```sql
ALTER TABLE settings ADD COLUMN node_wg_private_key TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN node_wg_public_key TEXT NOT NULL DEFAULT '';
```

- [ ] **Step 3: Regenerate frontend proto stubs**

```bash
cd /home/cc/peerman
PATH="frontend/node_modules/.bin:$PATH" protoc -I proto --es_out frontend/src/lib --es_opt target=ts proto/peerman.proto
```

Expected: `frontend/src/lib/peerman_pb.ts` updated with `nodeWgPrivateKey` and `nodeWgPublicKey` fields on Settings type.

- [ ] **Step 4: Commit**

```bash
git add proto/peerman.proto migrations/013_node_wg_keypair.sql frontend/src/lib/peerman_pb.ts
git commit -m "feat: add node WG keypair fields to proto and migration"
```

---

### Task 2: Settings Model + Repository

**Files:**
- Modify: `src/models/settings.rs:6-46` (struct + SQL constants)
- Modify: `src/models/settings.rs:53-61` (load method)

- [ ] **Step 1: Add fields to Settings struct**

In `src/models/settings.rs`, add after `confederation_local_asn: i64` (line 32):

```rust
    pub node_wg_private_key: String,
    pub node_wg_public_key: String,
```

- [ ] **Step 2: Update SETTINGS_COLUMNS**

In `src/models/settings.rs`, update the `SETTINGS_COLUMNS` constant to append:

```rust
const SETTINGS_COLUMNS: &str = "local_asn, bird_template_name, bird_router_id, \
    wg_default_listen_port, dn42_ipv4_prefix, dn42_ipv6_prefix, wg_table, \
    wg_mtu, wg_fwmark, wg_post_up, wg_post_down, \
    roa_mode, roa_static_v4_url, roa_static_v6_url, roa_rtr_address, roa_rtr_port, \
    bird_import_limit, bird_export_filter, bird_import_filter, \
    enable_community_filters, enable_bfd, bfd_interval_ms, bfd_multiplier, \
    cluster_tunnel_ipv6_range, enable_confederation, confederation_local_asn, \
    node_wg_private_key, node_wg_public_key";
```

- [ ] **Step 3: Update save() SQL**

In `src/models/settings.rs`, update the `save()` method to include the new columns. Add to the UPDATE SET clause:

```sql
             enable_confederation = ?, confederation_local_asn = ?,
             node_wg_private_key = ?, node_wg_public_key = ?
             WHERE id = 1
```

And add the binds after `confederation_local_asn`:

```rust
        .bind(&settings.node_wg_private_key)
        .bind(&settings.node_wg_public_key)
```

- [ ] **Step 4: Add auto-generate logic to load()**

Replace the `load()` method in `src/models/settings.rs`:

```rust
    pub async fn load(&self) -> Result<Settings, AppError> {
        let mut row = sqlx::query_as::<_, Settings>(&format!(
            "SELECT {SETTINGS_COLUMNS} FROM settings WHERE id = 1"
        ))
        .fetch_one(&self.pool)
        .await?;

        // Auto-generate node WG keypair on first load
        if row.node_wg_private_key.is_empty() {
            let (private_key, public_key) = crate::services::wireguard::generate_keypair();
            sqlx::query(
                "UPDATE settings SET node_wg_private_key = ?, node_wg_public_key = ? WHERE id = 1",
            )
            .bind(&private_key)
            .bind(&public_key)
            .execute(&self.pool)
            .await?;
            row.node_wg_private_key = private_key;
            row.node_wg_public_key = public_key;
        }

        Ok(row)
    }
```

- [ ] **Step 5: Update all test_settings() helpers**

Search for all `fn test_settings()` in the codebase. Each one needs the two new fields added. There are at least 2 locations: `src/services/wireguard.rs:335` and `src/services/bird.rs`.

Add to each `Settings { ... }` constructor:

```rust
            node_wg_private_key: String::new(),
            node_wg_public_key: String::new(),
```

- [ ] **Step 6: Verify compilation**

```bash
source "$HOME/.cargo/env" && cargo check 2>&1 | tail -20
```

Expected: Compiles without errors.

- [ ] **Step 7: Commit**

```bash
git add src/models/settings.rs src/services/wireguard.rs src/services/bird.rs
git commit -m "feat: add node WG keypair to Settings model with auto-generation"
```

---

### Task 3: Settings Service Proto Conversion

**Files:**
- Modify: `src/grpc/settings_service.rs:18-47` (settings_to_proto)
- Modify: `src/grpc/settings_service.rs:49-123` (apply_settings)

- [ ] **Step 1: Update settings_to_proto**

In `src/grpc/settings_service.rs`, add to `settings_to_proto()` after `confederation_local_asn`:

```rust
        node_wg_private_key: s.node_wg_private_key.clone(),
        node_wg_public_key: s.node_wg_public_key.clone(),
```

- [ ] **Step 2: Update apply_settings**

In `src/grpc/settings_service.rs`, add to `apply_settings()` after the `confederation_local_asn` block. The private key should NOT be user-editable (auto-generated only), but the public key is read-only display. Skip both in apply_settings to prevent user overwrites:

```rust
    // node_wg_private_key and node_wg_public_key are auto-generated, not user-editable
```

(No code change needed — just don't add apply logic for these fields.)

- [ ] **Step 3: Verify compilation**

```bash
source "$HOME/.cargo/env" && cargo check 2>&1 | tail -20
```

Expected: Compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add src/grpc/settings_service.rs
git commit -m "feat: expose node WG keypair in settings proto conversion"
```

---

### Task 4: DN42 Convention Helpers

**Files:**
- Create: `src/services/dn42.rs`
- Modify: `src/services/mod.rs` (add `pub mod dn42;`)

- [ ] **Step 1: Create dn42.rs with helper functions**

Create `src/services/dn42.rs`:

```rust
/// DN42 convention helpers for ASN-derived values.

/// Listen port: `2` + last 4 digits of ASN.
/// Example: ASN 4242420365 → port 20365
pub fn listen_port_from_asn(asn: i64) -> u32 {
    (20000 + (asn % 10000)) as u32
}

/// Link-local IPv6: `fe80::` + last 4 digits of ASN (strip leading zeros).
/// Example: ASN 4242420365 → "fe80::365"
/// Example: ASN 4242421000 → "fe80::1000"
pub fn link_local_from_asn(asn: i64) -> String {
    let last4 = (asn % 10000) as u64;
    format!("fe80::{last4:x}")
}

/// Sanitize peer name into WG interface name.
/// Example: "Aleksana" → "wg-aleksana"
pub fn sanitize_interface_name(name: &str) -> String {
    let sanitized: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    // Collapse consecutive dashes
    let mut result = String::new();
    let mut prev_dash = false;
    for c in sanitized.chars() {
        if c == '-' {
            if !prev_dash {
                result.push(c);
            }
            prev_dash = true;
        } else {
            result.push(c);
            prev_dash = false;
        }
    }
    // Trim leading/trailing dashes
    let trimmed = result.trim_matches('-');
    // Prepend "wg-" and truncate
    let full = format!("wg-{trimmed}");
    if full.len() > 15 {
        full[..15].to_string()
    } else {
        full
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_listen_port_from_asn() {
        assert_eq!(listen_port_from_asn(4242420365), 20365);
        assert_eq!(listen_port_from_asn(4242420000), 20000);
        assert_eq!(listen_port_from_asn(4242429999), 29999);
    }

    #[test]
    fn test_link_local_from_asn() {
        assert_eq!(link_local_from_asn(4242420365), "fe80::365");
        assert_eq!(link_local_from_asn(4242421000), "fe80::1000");
        assert_eq!(link_local_from_asn(4242420001), "fe80::1");
        assert_eq!(link_local_from_asn(4242420000), "fe80::0");
    }

    #[test]
    fn test_sanitize_interface_name() {
        assert_eq!(sanitize_interface_name("Aleksana"), "wg-aleksana");
        assert_eq!(sanitize_interface_name("my peer!"), "wg-my-peer");
        assert_eq!(sanitize_interface_name("a--b"), "wg-a-b");
        assert_eq!(sanitize_interface_name("-dash-"), "wg-dash");
    }
}
```

- [ ] **Step 2: Register module**

In `src/services/mod.rs`, add:

```rust
pub mod dn42;
```

- [ ] **Step 3: Run tests**

```bash
source "$HOME/.cargo/env" && cargo test dn42 2>&1 | tail -20
```

Expected: All 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/services/dn42.rs src/services/mod.rs
git commit -m "feat: add DN42 convention helper functions"
```

---

### Task 5: WireGuard AllowedIPs Update

**Files:**
- Modify: `src/services/wireguard.rs:318-323` (AllowedIPs in generate_config)

- [ ] **Step 1: Add fe80::/10 to AllowedIPs**

In `src/services/wireguard.rs`, find the AllowedIPs line in `generate_config()` (around line 318-323). Replace:

```rust
    let allowed_ips = format!(
        "{}, {}, fe80::/10",
        settings.dn42_ipv4_prefix, settings.dn42_ipv6_prefix
    );
```

- [ ] **Step 2: Update existing test**

In `src/services/wireguard.rs`, find `test_generate_config_contains_sections` and verify it checks for `fe80::/10` in the output. If it asserts on AllowedIPs content, update the expected string.

- [ ] **Step 3: Run tests**

```bash
source "$HOME/.cargo/env" && cargo test wireguard 2>&1 | tail -20
```

Expected: All wireguard tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/services/wireguard.rs
git commit -m "feat: add fe80::/10 to WG AllowedIPs for DN42 compliance"
```

---

### Task 6: Peer Service Auto-Fill

**Files:**
- Modify: `src/grpc/peer_service.rs:233-305` (create_peer)
- Modify: `src/grpc/peer_service.rs:307-390` (update_peer)

- [ ] **Step 1: Add auto-fill helper function**

In `src/grpc/peer_service.rs`, add a new function before `create_request_to_proto`:

```rust
/// Apply DN42 defaults to a peer proto for empty/zero fields.
fn apply_dn42_defaults(peer: &mut Peer, settings: &crate::models::settings::Settings) {
    use crate::services::dn42;

    // Private key: use node keypair if not set
    if peer.wg_private_key.is_empty() {
        peer.wg_private_key = settings.node_wg_private_key.clone();
    }

    // Listen port: derive from local ASN if not set
    if peer.wg_listen_port == 0 {
        peer.wg_listen_port = dn42::listen_port_from_asn(settings.local_asn);
    }

    // Interface name: generate from name if not set
    if peer.wg_interface_name.is_empty() {
        peer.wg_interface_name = dn42::sanitize_interface_name(&peer.name);
    }

    // Local ASN: use settings default if not set
    if peer.local_asn == 0 {
        peer.local_asn = settings.local_asn;
    }

    // IPv6 link-local: derive from ASN if not set
    if peer.ipv6_tunnel_local.is_empty() && peer.local_asn > 0 {
        peer.ipv6_tunnel_local = dn42::link_local_from_asn(peer.local_asn);
    }
    if peer.ipv6_tunnel_remote.is_empty() && peer.asn > 0 {
        peer.ipv6_tunnel_remote = dn42::link_local_from_asn(peer.asn);
    }

    // Remote port: derive from peer ASN if not set
    if peer.wg_remote_port == 0 && peer.asn > 0 {
        peer.wg_remote_port = dn42::listen_port_from_asn(peer.asn);
    }
}
```

- [ ] **Step 2: Wire auto-fill into create_peer**

In `create_peer()`, after `let proto = create_request_to_proto(&req);` (line 293) and before `let peer = self.state.peer_repo.create_full(...)`, add:

```rust
        let mut proto = create_request_to_proto(&req);
        let settings = self
            .state
            .settings_repo
            .load()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        apply_dn42_defaults(&mut proto, &settings);
```

Also update the existing `let proto = create_request_to_proto(&req);` to `let mut proto = ...`.

- [ ] **Step 3: Wire auto-fill into update_peer**

In `update_peer()`, after `peer.apply_proto(&update_request_to_proto(&req));` (line 377), add auto-fill for the merged peer:

```rust
        // Apply DN42 defaults for any still-empty fields
        let settings = self
            .state
            .settings_repo
            .load()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let mut proto: crate::grpc::generated::Peer = (&peer).into();
        apply_dn42_defaults(&mut proto, &settings);
        peer = proto.into();
```

Note: This requires `Peer` to implement `Into<proto::Peer>` and `From<proto::Peer>` — check if `peer.rs` already has these. If `apply_proto` handles the merge, we may need to apply defaults to the proto before `apply_proto` instead.

**Alternative approach for update_peer:** Apply defaults to the request proto before `apply_proto`:

```rust
        let settings = self
            .state
            .settings_repo
            .load()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let mut update_proto = update_request_to_proto(&req);
        apply_dn42_defaults(&mut update_proto, &settings);
        peer.apply_proto(&update_proto);
```

- [ ] **Step 4: Verify compilation**

```bash
source "$HOME/.cargo/env" && cargo check 2>&1 | tail -20
```

Expected: Compiles without errors.

- [ ] **Step 5: Run all tests**

```bash
source "$HOME/.cargo/env" && cargo test 2>&1 | tail -30
```

Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/grpc/peer_service.rs
git commit -m "feat: auto-fill DN42 defaults in create_peer and update_peer"
```

---

### Task 7: Frontend PeerForm Simplification

**Files:**
- Modify: `frontend/src/components/peers/PeerForm.tsx`

- [ ] **Step 1: Add DN42 helper functions**

At the bottom of `PeerForm.tsx`, replace `sanitizeInterfaceName` with:

```typescript
function sanitizeInterfaceName(name: string): string {
  return 'wg-' + name
    .toLowerCase()
    .replace(/[^a-z0-9]/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
    .substring(0, 15);
}

function listenPortFromAsn(asn: string): number {
  return 20000 + (Number(asn) % 10000);
}

function linkLocalFromAsn(asn: string): string {
  const last4 = Number(asn) % 10000;
  return `fe80::${last4.toString(16)}`;
}
```

- [ ] **Step 2: Update state defaults**

Update the initial state values:

```typescript
  const [wgRemotePort, setWgRemotePort] = useState('0');  // will auto-fill from ASN
  const [wgListenPort, setWgListenPort] = useState('0');   // will auto-fill from settings
```

Remove the `wgPrivateKey` state entirely. Remove the `handleGenerate` function.

- [ ] **Step 3: Update useEffect for settings defaults**

Replace the existing settings useEffect:

```typescript
  useEffect(() => {
    if (!isEdit && settings) {
      setWgListenPort('0'); // will be auto-filled by backend
      setLocalAsn(settings.localAsn.toString());
    }
  }, [isEdit, settings]);
```

- [ ] **Step 4: Add auto-derive effect for remote port**

Add a new useEffect to auto-derive remote port from ASN:

```typescript
  useEffect(() => {
    if (!isEdit && asn && asn !== '0') {
      setWgRemotePort(String(listenPortFromAsn(asn)));
    }
  }, [isEdit, asn]);
```

- [ ] **Step 5: Simplify the form JSX**

Replace the entire `<form>` content with the simplified layout. The key changes:

1. **WireGuard section:** Remove "Generate Keypair" button, remove Private Key input, remove Public Key input. Show only Remote Address and Remote Port.
2. **Add auto-derived hints** below Name and ASN fields.
3. **Move advanced fields** into the existing `<details>` element.
4. **Remove** the separate Tunnel section — move IPv4/IPv6 fields into advanced.

The simplified form structure:

```tsx
      <form onSubmit={handleSubmit} className="space-y-lg">
        {/* Identity */}
        <fieldset className="card">
          <legend className="text-body-sm-strong text-ink mb-md">Identity</legend>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-sm">
            <div className="flex flex-col gap-1">
              <label className="text-caption text-mute">Target Node</label>
              <select value={originNodeId} onChange={(e) => setOriginNodeId(e.target.value)} className="form-input">
                <option value="">This node (local)</option>
                {nodes.filter(n => n.online).map(n => (
                  <option key={n.id} value={n.id}>{n.name} ({n.listenAddr})</option>
                ))}
              </select>
            </div>
            <div>
              <Input label="Name" value={name} onChange={setName} required />
              {name && <p className="text-caption text-mute mt-1">Interface: {sanitizeInterfaceName(name)}</p>}
            </div>
            <Input label="Description" value={description} onChange={setDescription} />
            <div>
              <Input label="Remote ASN" value={asn} onChange={setAsn} placeholder="424242XXXX" />
              {asn && asn !== '0' && (
                <p className="text-caption text-mute mt-1">
                  Remote link-local: {linkLocalFromAsn(asn)} | Default port: {listenPortFromAsn(asn)}
                </p>
              )}
            </div>
          </div>
          <details className="mt-sm">
            <summary className="text-caption text-mute cursor-pointer select-none">Advanced Identity</summary>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-sm mt-sm">
              <Input label="Local ASN" value={localAsn} onChange={setLocalAsn} />
            </div>
          </details>
        </fieldset>

        {/* WireGuard */}
        <fieldset className="card">
          <legend className="text-body-sm-strong text-ink mb-md">WireGuard</legend>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-sm">
            <Input label="Peer Public Key" value={wgPublicKey} onChange={setWgPublicKey} mono required />
            <Input label="Remote Address" value={wgRemoteAddress} onChange={setWgRemoteAddress} required />
            <Input label="Remote Port" value={wgRemotePort} onChange={setWgRemotePort} type="number" />
          </div>
          <details className="mt-sm">
            <summary className="text-caption text-mute cursor-pointer select-none">Advanced WireGuard</summary>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-sm mt-sm">
              <Input label="Listen Port" value={wgListenPort} onChange={setWgListenPort} type="number" placeholder="Auto from ASN" />
              <Input label="Interface Name" value={wgInterfaceName} onChange={setWgInterfaceName} placeholder={name ? sanitizeInterfaceName(name) : 'wg-peer-name'} />
            </div>
          </details>
        </fieldset>

        {/* Tunnel + BGP combined in advanced */}
        <fieldset className="card">
          <legend className="text-body-sm-strong text-ink mb-md">Tunnel & BGP</legend>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-sm">
            <Toggle label="Multiprotocol" checked={multiprotocol} onChange={setMultiprotocol} />
            <Toggle label="Extended Nexthop" checked={extendedNexthop} onChange={setExtendedNexthop} />
            <Toggle label="Passive" checked={passive} onChange={setPassive} />
            <div className="flex flex-col gap-1">
              <label className="text-caption text-mute">Sessions</label>
              <select value={sessions} onChange={(e) => setSessions(Number(e.target.value))} className="form-input">
                <option value={2}>Both</option>
                <option value={0}>IPv4</option>
                <option value={1}>IPv6</option>
              </select>
            </div>
          </div>
          <details className="mt-sm">
            <summary className="text-caption text-mute cursor-pointer select-none">Advanced Tunnel</summary>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-sm mt-sm">
              <Input label="IPv4 Local" value={ipv4TunnelLocal} onChange={setIpv4TunnelLocal} />
              <Input label="IPv4 Remote" value={ipv4TunnelRemote} onChange={setIpv4TunnelRemote} />
              <Input label="IPv6 Local (link-local)" value={ipv6TunnelLocal} onChange={setIpv6TunnelLocal} placeholder="fe80::1" />
              <Input label="IPv6 Remote (link-local)" value={ipv6TunnelRemote} onChange={setIpv6TunnelRemote} placeholder="fe80::2" />
              <Input label="Max Prefix (import)" value={importMaxPrefix} onChange={setImportMaxPrefix} type="number" />
              <Input label="Max Prefix (export)" value={exportMaxPrefix} onChange={setExportMaxPrefix} type="number" />
            </div>
          </details>
        </fieldset>

        <div className="flex items-center gap-2">
          <button type="submit" disabled={saving} className="btn-primary">
            {saving ? 'Saving...' : isEdit ? 'Update Peer' : 'Create Peer'}
          </button>
          <button type="button" onClick={() => navigate(-1)} className="btn-secondary">
            Cancel
          </button>
        </div>
      </form>
```

- [ ] **Step 6: Update handleSubmit to remove wgPrivateKey**

In `handleSubmit`, remove `wgPrivateKey` from the `base` object. The backend will auto-fill it.

```typescript
    const base = {
      name,
      description,
      asn: BigInt(asn || '0'),
      localAsn: BigInt(localAsn || '0'),
      wgPrivateKey: '',  // auto-filled by backend
      wgPublicKey,
      wgRemoteAddress,
      wgRemotePort: Number(wgRemotePort || '0'),
      wgListenPort: Number(wgListenPort || '0'),
      wgInterfaceName,
      ipv4TunnelLocal,
      ipv4TunnelRemote,
      ipv6TunnelLocal,
      ipv6TunnelRemote,
      multiprotocol,
      extendedNexthop,
      sessions,
      passive,
      importMaxPrefix: Number(importMaxPrefix || '0'),
      exportMaxPrefix: Number(exportMaxPrefix || '0'),
      originNodeId,
    };
```

- [ ] **Step 7: Update edit mode useEffect**

In the edit mode useEffect, remove `setWgPrivateKey(existingPeer.wgPrivateKey)`. Keep all other fields.

- [ ] **Step 8: Type-check**

```bash
cd /home/cc/peerman/frontend && pnpm exec tsc --noEmit 2>&1 | tail -20
```

Expected: No type errors.

- [ ] **Step 9: Commit**

```bash
git add frontend/src/components/peers/PeerForm.tsx
git commit -m "feat: simplify PeerForm with DN42 defaults and 5-field layout"
```

---

### Task 8: Frontend PeerDetail Update

**Files:**
- Modify: `frontend/src/components/peers/PeerDetail.tsx:81-87` (WireGuard section)

- [ ] **Step 1: Remove Private Key from WireGuard section**

In `PeerDetail.tsx`, the WireGuard section currently shows Public Key, Remote Address, Remote Port, Listen Port, Interface. This is already correct — no Private Key is shown (it's redacted by the API). No changes needed to the WireGuard section.

However, add the Node Public Key as read-only info. Import `useSettings`:

```typescript
import { useSettings } from '../../hooks/useSettings';
```

Add in the component:

```typescript
  const { settings } = useSettings();
```

Add a new field in the WireGuard section:

```tsx
            <Field label="Node Public Key" value={settings?.nodeWgPublicKey || '—'} mono />
```

- [ ] **Step 2: Type-check**

```bash
cd /home/cc/peerman/frontend && pnpm exec tsc --noEmit 2>&1 | tail -20
```

Expected: No type errors.

- [ ] **Step 3: Commit**

```bash
git add frontend/src/components/peers/PeerDetail.tsx
git commit -m "feat: show node public key in peer detail view"
```

---

### Task 9: Integration Tests

**Files:**
- Modify: `src/services/wireguard.rs` (test module)
- Modify: `src/services/dn42.rs` (test module)

- [ ] **Step 1: Add AllowedIPs test for fe80::/10**

In `src/services/wireguard.rs` tests, add:

```rust
    #[test]
    fn test_generate_config_includes_link_local_allowed_ips() {
        let peer = make_test_peer();
        let settings = test_settings();
        let config = generate_config(&peer, &settings);
        assert!(config.contains("fe80::/10"), "AllowedIPs should include fe80::/10");
    }
```

- [ ] **Step 2: Add test for DN42 defaults in peer_service**

In `src/grpc/peer_service.rs` tests, add a test for `apply_dn42_defaults`:

```rust
    #[test]
    fn test_apply_dn42_defaults_fills_empty_fields() {
        let settings = crate::models::settings::Settings {
            local_asn: 4242420365,
            node_wg_private_key: "test-private-key".into(),
            node_wg_public_key: "test-public-key".into(),
            ..make_test_settings()
        };
        let mut peer = Peer {
            wg_private_key: String::new(),
            wg_listen_port: 0,
            wg_interface_name: String::new(),
            local_asn: 0,
            ipv6_tunnel_local: String::new(),
            ipv6_tunnel_remote: String::new(),
            wg_remote_port: 0,
            asn: 4242421234,
            name: "testpeer".into(),
            ..make_test_proto_peer()
        };
        apply_dn42_defaults(&mut peer, &settings);
        assert_eq!(peer.wg_private_key, "test-private-key");
        assert_eq!(peer.wg_listen_port, 20365);
        assert_eq!(peer.wg_interface_name, "wg-testpeer");
        assert_eq!(peer.local_asn, 4242420365);
        assert_eq!(peer.ipv6_tunnel_local, "fe80::365");
        assert_eq!(peer.ipv6_tunnel_remote, "fe80::1234");
        assert_eq!(peer.wg_remote_port, 21234);
    }

    #[test]
    fn test_apply_dn42_defaults_preserves_existing_values() {
        let settings = crate::models::settings::Settings {
            local_asn: 4242420365,
            node_wg_private_key: "node-key".into(),
            ..make_test_settings()
        };
        let mut peer = Peer {
            wg_private_key: "existing-key".into(),
            wg_listen_port: 51820,
            ..make_test_proto_peer()
        };
        apply_dn42_defaults(&mut peer, &settings);
        assert_eq!(peer.wg_private_key, "existing-key");
        assert_eq!(peer.wg_listen_port, 51820);
    }
```

Note: You'll need to create `make_test_settings()` and `make_test_proto_peer()` helpers if they don't exist.

- [ ] **Step 3: Run all tests**

```bash
source "$HOME/.cargo/env" && cargo test 2>&1 | tail -30
```

Expected: All tests pass.

- [ ] **Step 4: Run clippy and fmt**

```bash
source "$HOME/.cargo/env" && cargo fmt && cargo clippy 2>&1 | tail -20
```

Expected: No warnings.

- [ ] **Step 5: Commit**

```bash
git add src/services/wireguard.rs src/services/dn42.rs src/grpc/peer_service.rs
git commit -m "test: add unit tests for DN42 defaults and AllowedIPs"
```

---

### Task 10: Final Verification

- [ ] **Step 1: Full build**

```bash
source "$HOME/.cargo/env" && cargo build 2>&1 | tail -20
```

Expected: Builds successfully (includes frontend build + proto gen).

- [ ] **Step 2: Full test suite**

```bash
source "$HOME/.cargo/env" && cargo test 2>&1 | tail -30
```

Expected: All tests pass.

- [ ] **Step 3: TypeScript type-check**

```bash
cd /home/cc/peerman/frontend && pnpm exec tsc --noEmit 2>&1 | tail -20
```

Expected: No type errors.

- [ ] **Step 4: Commit any remaining changes**

```bash
git add -A && git status
```

Review and commit if needed.

- [ ] **Step 5: Push to CI**

```bash
git push origin master
```

Monitor CI: `gh run list --limit 3` then `gh run watch <id> --exit-status`
