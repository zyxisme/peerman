# DN42 Standard Peer Simplification

**Date:** 2026-06-08
**Status:** Approved
**Goal:** Align peer creation with DN42 community conventions — one node keypair, auto-derived fields, minimal user input.

## Motivation

Current peer creation requires users to manually generate WG keypairs, fill in ~20 fields, and know DN42 conventions (port formulas, link-local addressing). In standard DN42 practice, a node has one WG keypair, listen ports follow `2 + ASN_last4` convention, and link-local IPv6 follows `fe80::ASN_last4`. Users should only need to provide the remote peer's identity.

## DN42 Conventions (Source of Truth)

| Field | Convention | Example |
|-------|-----------|---------|
| Node keypair | One per machine, shared across all interfaces | Same PrivateKey in every `[Interface]` |
| ListenPort | `2` + last 4 digits of local ASN | ASN 4242420365 → port `20365` |
| Link-local IPv6 | `fe80::` + ASN last 4 digits (strip leading zeros) | 4242420365 → `fe80::365` |
| Endpoint port | Remote peer's `2` + their ASN last 4 digits | Peer ASN 4242421234 → `21234` |
| Interface naming | `wg-` + peer nickname | `wg-aleksana` |
| AllowedIPs | Full DN42 ranges + link-local | `172.20.0.0/14, fd00::/8, fe80::/10` |
| PersistentKeepalive | 25 | Fixed |
| PresharedKey | Optional, per-peer | Extra layer of security |

## Design

### 1. Settings: Node-Level WG Keypair

**New fields in Settings model/proto:**

```protobuf
message Settings {
  // ... existing fields ...
  string node_wg_private_key = 27;
  string node_wg_public_key = 28;
}
```

**Auto-generation logic (in `SettingsRepository::load()`):**
- After loading settings from DB, check if `node_wg_private_key` is empty
- If empty: generate keypair via `services::wireguard::generate_keypair()`, save to DB
- Return settings with populated keypair

**Migration:** `ALTER TABLE settings ADD COLUMN node_wg_private_key TEXT NOT NULL DEFAULT ''`
**Migration:** `ALTER TABLE settings ADD COLUMN node_wg_public_key TEXT NOT NULL DEFAULT ''`

### 2. Peer Create/Update: Auto-Fill Defaults

**Applies to both `create_peer()` and `update_peer()` — after validation, fill empty/zero fields:**

> **Note on update_peer:** Auto-fill only applies to fields the user leaves empty/zero. If a peer already has a value, it's preserved. This lets users create with defaults then selectively override later.

```
wg_private_key:
  if empty → settings.node_wg_private_key

wg_listen_port:
  if 0 → 2 + (settings.local_asn % 10000)

wg_interface_name:
  if empty → "wg-" + sanitize(name)
  sanitize: lowercase, non-alnum → '-', collapse dashes, trim, max 12 chars

ipv6_tunnel_local:
  if empty and local_asn > 0 → "fe80::" + (local_asn % 10000) with leading zeros stripped

ipv6_tunnel_remote:
  if empty and asn > 0 → "fe80::" + (asn % 10000) with leading zeros stripped

AllowedIPs (in generate_config):
  append "fe80::/10" to existing DN42 prefix list
```

### 3. Frontend: Simplified PeerForm

**Default view — 5 fields:**

| Field | Type | Default | Required |
|-------|------|---------|----------|
| Peer Name | text | — | Yes |
| Remote ASN | number | — | Yes |
| Peer Public Key | text (mono) | — | Yes |
| Remote Address | text | — | Yes |
| Remote Port | number | `2 + (peer_asn % 10000)` — this is the **remote peer's** listen port | No |

**Auto-derived display (read-only hints below each field):**
- Interface Name → `wg-{name}`
- Listen Port → `2 + local_asn % 10000`
- Local Link-local → `fe80::{local_asn % 10000, no leading zeros}`
- Remote Link-local → `fe80::{peer_asn % 10000, no leading zeros}`

**Advanced options (collapsible):**
- Local ASN (default from settings)
- Listen Port (override auto-calculated)
- Interface Name (override auto-generated)
- IPv4 Tunnel Local / Remote
- IPv6 Tunnel Local / Remote (override auto-derived)
- BGP: Multiprotocol (default: true), Extended Nexthop (default: true), Sessions (default: Both), Passive (default: false)
- BGP: Max Prefix import/export (default: 0)
- PresharedKey (optional)

**Removed:**
- "Generate Keypair" button
- Private Key input field

### 4. WireGuard Config Generation Update

**`services::wireguard::generate_config()` changes:**

```rust
// AllowedIPs — add link-local
let allowed_ips = format!(
    "{}, {}, fe80::/10",
    settings.dn42_ipv4_prefix, settings.dn42_ipv6_prefix
);
```

This is the only change to config generation. The PrivateKey already comes from `peer.wg_private_key`, which will now be populated from the node keypair.

### 5. PeerDetail Page Update

- Remove "Private Key" from the WireGuard section (it's never exposed via API anyway — `peer_to_proto` redacts it)
- Show "Node Public Key" as read-only info (from settings)

## Files Changed

| File | Change |
|------|--------|
| `proto/peerman.proto` | Add `node_wg_private_key`, `node_wg_public_key` to Settings |
| `src/models/settings.rs` | Add fields to Settings struct, update SQL |
| `src/grpc/settings_service.rs` | Auto-generate keypair in load |
| `src/grpc/peer_service.rs` | Auto-fill defaults in create_peer |
| `src/services/wireguard.rs` | Add `fe80::/10` to AllowedIPs |
| `frontend/src/components/peers/PeerForm.tsx` | Simplified form with DN42 defaults |
| `frontend/src/components/peers/PeerDetail.tsx` | Remove private key display |
| `frontend/src/lib/peerman_pb.ts` | Regenerated proto stubs |
| `migrations/013_node_wg_keypair.sql` | Add columns to settings table |

## Backward Compatibility

- **Existing peers:** Keep their own keypairs unchanged. `generate_config` uses whatever `wg_private_key` the peer has — no change needed.
- **New peers:** Use the node keypair by default (auto-filled in `create_peer`).
- **Mixed mode:** A node can have peers with individual keypairs (old) and peers using the node keypair (new) simultaneously. No migration of existing peers needed.
- **Settings migration:** Adds columns with empty defaults; keypair auto-generated on first `load()`.
- **Frontend:** Existing edit flow still works — all fields are populated from the peer record. The simplified form only affects the "New Peer" experience.

## Testing

- Unit test: auto-fill logic produces correct ListenPort from ASN
- Unit test: link-local IPv6 derivation from ASN (with/without leading zeros)
- Unit test: AllowedIPs includes `fe80::/10`
- Unit test: settings auto-generates keypair when empty
- Manual: create peer with only 5 fields, verify generated WG config is valid
- Manual: create peer with advanced overrides, verify overrides take effect
