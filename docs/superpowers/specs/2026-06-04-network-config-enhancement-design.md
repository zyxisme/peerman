# Network Configuration Enhancement Design

**Date:** 2026-06-04
**Status:** Approved
**Approach:** Standard Alignment First (Phase-based)

## Overview

Enhance peerman's network configuration capabilities to fully align with DN42 best practices, based on research of lantian's blog and DN42 wiki. Four phases of implementation.

## Research Sources

- [Lantian's DN42 page](https://lantian.pub/en/page/dn42) - Network architecture, Looking Glass, FlapAlerted
- [Lantian's BGP Confederation guide](https://lantian.pub/en/article/modify-website/bird-confederation.lantian) - Confederation setup for multi-node AS
- [Lantian's "How to Kill DN42"](https://lantian.pub/en/article/modify-website/how-to-kill-the-dn42-network.lantian) - Lessons on IGP/BGP isolation, route flapping
- [DN42 Wiki Bird2 guide](https://wiki.dn42.us/howto/Bird2) - Standard BIRD2 config, BFD, ROA/RPKI
- [DN42 Wiki BGP communities](https://wiki.dn42.us/howto/BGP-communities) - AS 64511 community scheme (latency/bandwidth/crypto)
- [DN42 Wiki WireGuard guide](https://dn42.dev/howto/wireguard) - Per-peer interface pattern, AllowedIPs

## Current State Analysis

### What works well
- Per-peer WireGuard config generation (complete INI configs)
- Per-peer BIRD2 config generation (multiprotocol, separate sessions, extended nexthop)
- ROA/RPKI support (static file and RTR modes)
- Auto-apply on peer CRUD (wg syncconf + birdc configure)
- Cluster inter-node WireGuard tunnels (auto keypair, tunnel IP allocation)
- Cluster iBGP full-mesh (automatic BIRD iBGP blocks)
- Live status monitoring (WG interface/peer, BIRD protocol)
- Community rule data model (SQLite, repository, seeded defaults)
- CommunityMapper computation engine (latency/loss/bandwidth/crypto matching)

### Critical gaps
1. **BGP Communities not wired**: CommunityMapper::compute_communities() exists but generate_full_config() always passes empty community arrays to generate_peer_block()
2. **No DN42 standard filter functions**: Missing update_flags(), dn42_import_filter(), dn42_export_filter()
3. **No BFD support**: DN42 wiki recommends BFD for fast failure detection
4. **Cross-node Looking Glass stubbed**: bird_service.rs returns "not yet implemented" for remote nodes
5. **No WireGuard interface lifecycle**: Only syncconf, no up/down/restart
6. **IPv4-only cluster tunnels**: No IPv6 tunnel support
7. **med_penalty unused**: Stored in DB but never applied in filter generation

## Phase 1: BGP Communities Full Integration (Critical)

### Goal
Make generated bird.conf fully compliant with DN42 wiki's BGP communities standard (AS 64511).

### Changes

#### 1.1 Add DN42 standard community functions to BIRD config

New function `generate_community_functions()` in `src/services/bird.rs`:

```bird
function update_latency(int link_latency) {
    bgp_community.add((64511, link_latency));
}

function update_bandwidth(int link_bandwidth) {
    bgp_community.add((64511, 10 + link_bandwidth));
}

function update_crypto(int link_crypto) {
    bgp_community.add((64511, 30 + link_crypto));
}

function update_flags(int link_latency; int link_bandwidth; int link_crypto) {
    update_latency(link_latency);
    update_bandwidth(link_bandwidth);
    update_crypto(link_crypto);
}

function dn42_import_filter(int link_latency; int link_bandwidth; int link_crypto) -> bool {
    if is_valid_network() && !is_self_net() then {
        if (roa_check(dn42_roa, net, bgp_path.last) != ROA_VALID) then {
            print "[dn42] ROA check failed for ", net, " ASN ", bgp_path.last;
            reject;
        }
        update_flags(link_latency, link_bandwidth, link_crypto);
        if (bgp_path.len = 1) then {
            bgp_local_pref = bgp_local_pref + 500;
        }
        accept;
    } else reject;
}

function dn42_export_filter(int link_latency; int link_bandwidth; int link_crypto) -> bool {
    if is_valid_network() || is_valid_network_v6() then {
        update_flags(link_latency, link_bandwidth, link_crypto);
        accept;
    } else reject;
}
```

#### 1.2 Wire CommunityMapper into generate_full_config()

- Modify `generate_full_config()` to accept a `CommunityMapper` reference
- For each enabled peer, call `compute_communities(peer)` to get community strings
- Parse community strings to extract latency/bandwidth/crypto tier values
- Pass these values to `generate_peer_block_with_communities()`

#### 1.3 Modify peer block generation to use standard filter calls

Each peer block should call the standard filter functions with per-peer tier values:

```bird
protocol bgp peer_example from dnpeers {
    neighbor 172.22.76.185 as 4242421234;
    direct;
    ipv4 {
        import where dn42_import_filter(3, 25, 34);
        export where dn42_export_filter(3, 25, 34);
    };
    ipv6 {
        import where dn42_import_filter(3, 25, 34);
        export where dn42_export_filter(3, 25, 34);
    };
}
```

Where `3, 25, 34` are the latency tier, bandwidth tier, and crypto tier computed by CommunityMapper.

#### 1.4 Apply med_penalty in export filter

Modify export filter to apply MED penalty based on community rules:

```bird
function dn42_export_filter(int link_latency; int link_bandwidth; int link_crypto) -> bool {
    if is_valid_network() || is_valid_network_v6() then {
        update_flags(link_latency, link_bandwidth, link_crypto);
        bgp_med = bgp_med + 4 * link_crypto;  # Crypto penalty
        bgp_med = bgp_med + 9 * link_bandwidth;  # Bandwidth penalty
        bgp_med = bgp_med + link_latency;  # Latency penalty
        accept;
    } else reject;
}
```

#### 1.5 Frontend settings

- Add `enable_community_filters` boolean to Settings proto and form
- When enabled, peer blocks use `dn42_import_filter`/`dn42_export_filter` with computed tiers
- When disabled, use existing simple import/export filters (backward compatible)

### Files modified
- `src/services/bird.rs` - Add generate_community_functions(), modify generate_full_config(), modify generate_peer_block_with_communities()
- `src/services/community_mapper.rs` - Add tier extraction helpers (latency_to_tier, bandwidth_to_tier, crypto_to_tier)
- `src/grpc/peer_service.rs` - Pass CommunityMapper to config generation
- `proto/peerman.proto` - Add enable_community_filters to Settings
- `frontend/src/components/settings/SettingsForm.tsx` - Add community filters toggle

## Phase 2: BFD + Cross-node Looking Glass

### Goal
Fast failure detection via BFD + complete cross-node debugging capability.

### 2.1 BFD Support

#### Proto changes
Add to Settings message:
```protobuf
bool enable_bfd = 20;
uint32 bfd_interval_ms = 21;      # default 300
uint32 bfd_multiplier = 22;       # default 3
```

#### BIRD config generation
Add `protocol bfd` block when enabled:
```bird
protocol bfd {
    interface "wg*" {
        interval <interval_ms>ms;
        multiplier <multiplier>;
    };
}
```

Add `bfd on;` to each peer's BGP block when BFD is enabled.

#### Frontend
Add BFD settings section in SettingsForm.

### 2.2 Cross-node Looking Glass

#### Backend changes
Modify `src/grpc/bird_service.rs`:
- When `target_node_id != local_node_name`:
  1. Look up target node from ClusterAggregator's node cache
  2. Connect to target node's BirdService gRPC endpoint
  3. Forward ExecuteCommand RPC with cluster auth
  4. Return result

Add to `src/cluster/aggregator.rs`:
- `execute_bird_command_on_node(node_addr, command)` method
- Reuse existing connect/auth/timeout pattern

#### Frontend
LookingGlass.tsx already has node selector. No changes needed.

### Files modified
- `src/services/bird.rs` - Add BFD block generation
- `src/grpc/bird_service.rs` - Implement cross-node forwarding
- `src/cluster/aggregator.rs` - Add execute_bird_command_on_node()
- `proto/peerman.proto` - Add BFD settings
- `frontend/src/components/settings/SettingsForm.tsx` - Add BFD section

## Phase 3: WireGuard Lifecycle + IPv6 Cluster Tunnels

### Goal
Complete WireGuard interface management + dual-stack cluster tunnels.

### 3.1 WireGuard Interface Lifecycle

#### Proto changes
Add to PeerService:
```protobuf
rpc ApplyWireGuard(ApplyWireGuardRequest) returns (ApplyWireGuardResponse);
rpc RestartWireGuard(RestartWireGuardRequest) returns (RestartWireGuardResponse);
```

#### Backend implementation
- `apply_wg_interface()`: `wg-quick down <iface> && wg-quick up <iface>`
- Handles interface name from peer's `wg_interface` field (default: `wg0`)
- Returns success/error status

#### Frontend
- Status page: Add "Restart WG" button per interface
- Peer detail: Add "Apply & Restart" option alongside existing "Save"

### 3.2 IPv6 Cluster Tunnels

#### Proto changes
Add to NodeInfo:
```protobuf
string tunnel_ipv6 = 8;
```

Add to Settings:
```protobuf
string cluster_tunnel_ipv6_range = 23;  # e.g. "fd42:cluster::/48"
```

#### Backend changes
- `tunnel.rs`: Assign IPv6 tunnel addresses alongside IPv4
- `generate_cluster_wg_config()`: Add IPv6 endpoints and AllowedIPs
- `generate_ibgp_blocks()`: Support IPv6 neighbor addresses for iBGP

#### Migration
Add `tunnel_ipv6` column to `nodes` table.

### Files modified
- `src/services/wireguard.rs` - Add lifecycle methods, IPv6 cluster config
- `src/grpc/peer_service.rs` - Add ApplyWireGuard/RestartWireGuard RPCs
- `src/cluster/tunnel.rs` - IPv6 tunnel IP assignment
- `src/services/bird.rs` - IPv6 iBGP blocks
- `proto/peerman.proto` - New RPCs, NodeInfo.tunnel_ipv6, Settings.cluster_tunnel_ipv6_range
- `migrations/` - Add tunnel_ipv6 column
- `frontend/src/components/status/StatusPage.tsx` - Restart WG button

## Phase 4: BGP Confederation (Optional Advanced)

### Goal
Replace iBGP full mesh with BGP Confederation for more flexible AS internal topology.

### Design
Reference: [Lantian's BGP Confederation guide](https://lantian.pub/en/article/modify-website/bird-confederation.lantian)

- Use DN42 ASN as Confederation Identifier
- Each cluster node uses a private ASN (from configurable range)
- BIRD config adds:
  ```bird
  confederation <dn42_asn>;
  confederation member yes;
  ```
- iBGP blocks become confederation peering blocks:
  ```bird
  protocol bgp node_<name> from lantian_internal {
      neighbor <tunnel_ip> external;
  };
  ```
- Benefits: reduced BGP sessions, more flexible topology, proper AS path within confederation

### Proto changes
Add to Settings:
```protobuf
bool enable_confederation = 24;
int64 confederation_local_asn = 25;  # Private ASN for this node
```

Add to ClusterService:
```protobuf
rpc GetConfederationStatus(google.protobuf.Empty) returns (ConfederationStatus);
```

### Files modified
- `src/services/bird.rs` - Confederation-aware config generation
- `src/cluster/tunnel.rs` - Private ASN assignment
- `proto/peerman.proto` - Confederation settings
- `frontend/src/components/settings/SettingsForm.tsx` - Confederation toggle

## Testing Strategy

### Phase 1
- Unit tests for `generate_community_functions()` output
- Unit tests for community tier extraction
- Integration test: generate_full_config() with mock CommunityMapper, verify output contains dn42_import_filter calls
- Verify generated config passes `bird -c <config> -p` syntax check

### Phase 2
- Unit tests for BFD block generation
- Unit tests for cross-node command forwarding (mock gRPC)
- Manual test: run Looking Glass against remote node

### Phase 3
- Unit tests for WireGuard lifecycle commands
- Unit tests for IPv6 tunnel IP assignment
- Integration test: generate_cluster_wg_config() with IPv6 nodes

### Phase 4
- Unit tests for confederation config generation
- Manual test: verify confederation BGP sessions establish correctly

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Community filter changes break existing BIRD configs | High | Keep backward compatibility via enable_community_filters toggle |
| BFD not supported by all peers | Medium | Make BFD per-peer opt-in, not global |
| Cross-node gRPC auth failures | Medium | Reuse existing cluster_key auth pattern |
| IPv6 tunnel allocation conflicts | Low | Use /48 range with /128 per node |
| Confederation complexity | Medium | Phase 4 is optional, can defer |
