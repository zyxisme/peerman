# Network Configuration Enhancement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align peerman's network configuration with DN42 best practices (lantian blog + DN42 wiki), adding BGP communities integration, BFD, cross-node Looking Glass, WireGuard lifecycle management, and IPv6 cluster tunnels.

**Architecture:** Four-phase approach: (1) Wire CommunityMapper into BIRD config generation with DN42 standard filter functions, (2) Add BFD support and complete cross-node Looking Glass, (3) WireGuard interface lifecycle + IPv6 cluster tunnels, (4) Optional BGP Confederation.

**Tech Stack:** Rust (tonic, tokio), BIRD2 config generation, protobuf, React frontend

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/services/bird.rs` | BIRD2 config generation — community functions, BFD blocks, peer blocks, full config assembly |
| `src/services/community_mapper.rs` | Community tier computation — latency/bandwidth/crypto tier extraction |
| `src/grpc/bird_service.rs` | BirdService gRPC — cross-node command forwarding |
| `src/cluster/aggregator.rs` | Cluster fanout — add bird command forwarding method |
| `src/grpc/peer_service.rs` | PeerService — wire CommunityMapper into auto_apply_wg_bird |
| `src/services/wireguard.rs` | WireGuard lifecycle — up/down/restart |
| `src/cluster/tunnel.rs` | IPv6 tunnel assignment |
| `proto/peerman.proto` | New settings fields (BFD, community toggle, IPv6 range) |
| `frontend/src/components/settings/SettingsForm.tsx` | BFD + community filter settings UI |

---

## Phase 1: BGP Communities Full Integration

### Task 1: Add community tier extraction helpers to CommunityMapper

**Files:**
- Modify: `src/services/community_mapper.rs`
- Test: inline `#[cfg(test)]` module

- [ ] **Step 1: Write failing tests for tier extraction**

```rust
// Add to bottom of src/services/community_mapper.rs, inside #[cfg(test)] mod tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_to_tier_metro() {
        assert_eq!(CommunityMapper::latency_to_tier(3.0), 1);
    }

    #[test]
    fn test_latency_to_tier_regional() {
        assert_eq!(CommunityMapper::latency_to_tier(15.0), 2);
    }

    #[test]
    fn test_latency_to_tier_continental() {
        assert_eq!(CommunityMapper::latency_to_tier(35.0), 3);
    }

    #[test]
    fn test_latency_to_tier_intercontinental() {
        assert_eq!(CommunityMapper::latency_to_tier(100.0), 4);
    }

    #[test]
    fn test_latency_to_tier_high() {
        assert_eq!(CommunityMapper::latency_to_tier(200.0), 5);
    }

    #[test]
    fn test_parse_community_tier() {
        assert_eq!(CommunityMapper::parse_community_tier("4242420000,10"), 10);
        assert_eq!(CommunityMapper::parse_community_tier("4242420000,620"), 620);
    }

    #[test]
    fn test_parse_community_tier_invalid() {
        assert_eq!(CommunityMapper::parse_community_tier("invalid"), 0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib services::community_mapper::tests 2>&1 | tail -20`
Expected: compilation error — `latency_to_tier` and `parse_community_tier` not found

- [ ] **Step 3: Implement tier extraction methods**

Add these methods to `impl CommunityMapper` block in `src/services/community_mapper.rs`:

```rust
    /// Map latency (ms) to DN42 community tier (1-5).
    pub fn latency_to_tier(latency_ms: f64) -> i32 {
        if latency_ms <= 5.0 { 1 }
        else if latency_ms <= 20.0 { 2 }
        else if latency_ms <= 50.0 { 3 }
        else if latency_ms <= 150.0 { 4 }
        else { 5 }
    }

    /// Extract the numeric tier from a community string like "4242420000,10".
    pub fn parse_community_tier(community: &str) -> i32 {
        community
            .split(',')
            .nth(1)
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(0)
    }

    /// Extract the best (lowest) latency tier from a list of community strings.
    pub fn best_latency_tier(communities: &[String]) -> i32 {
        communities.iter()
            .map(|c| Self::parse_community_tier(c))
            .min()
            .unwrap_or(0)
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib services::community_mapper::tests 2>&1 | tail -20`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/services/community_mapper.rs
git commit -m "feat(community): add tier extraction helpers for DN42 standard communities"
```

---

### Task 2: Add DN42 standard community filter functions to BIRD config

**Files:**
- Modify: `src/services/bird.rs:155-179` (generate_filter_functions)
- Test: inline `#[cfg(test)]` module

- [ ] **Step 1: Write failing test for community functions**

Add to `src/services/bird.rs` tests module:

```rust
    #[test]
    fn test_generate_full_config_has_community_functions() {
        let config = generate_full_config(&[], &test_settings(), "");
        assert!(config.contains("function update_latency"));
        assert!(config.contains("function update_bandwidth"));
        assert!(config.contains("function update_crypto"));
        assert!(config.contains("function update_flags"));
        assert!(config.contains("function dn42_import_filter"));
        assert!(config.contains("function dn42_export_filter"));
    }

    #[test]
    fn test_generate_full_config_community_functions_use_64511() {
        let config = generate_full_config(&[], &test_settings(), "");
        assert!(config.contains("bgp_community.add((64511,"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib services::bird::tests::test_generate_full_config_has_community_functions 2>&1 | tail -10`
Expected: FAIL — functions not found in output

- [ ] **Step 3: Add generate_community_functions() to bird.rs**

Add new function after `generate_filter_functions` (line 179):

```rust
fn generate_community_functions() -> String {
    "\
function update_latency(int link_latency) {\n\
    bgp_community.add((64511, link_latency));\n\
}\n\n\
function update_bandwidth(int link_bandwidth) {\n\
    bgp_community.add((64511, 10 + link_bandwidth));\n\
}\n\n\
function update_crypto(int link_crypto) {\n\
    bgp_community.add((64511, 30 + link_crypto));\n\
}\n\n\
function update_flags(int link_latency; int link_bandwidth; int link_crypto) {\n\
    update_latency(link_latency);\n\
    update_bandwidth(link_bandwidth);\n\
    update_crypto(link_crypto);\n\
}\n\n\
function dn42_import_filter(int link_latency; int link_bandwidth; int link_crypto) {\n\
    if is_valid_network() && !is_self_net() then {\n\
        if (roa_check(dn42_roa, net, bgp_path.last) != ROA_VALID) then {\n\
            print \"[dn42] ROA check failed for \", net, \" ASN \", bgp_path.last;\n\
            reject;\n\
        }\n\
        update_flags(link_latency, link_bandwidth, link_crypto);\n\
        if (bgp_path.len = 1) then {\n\
            bgp_local_pref = bgp_local_pref + 500;\n\
        }\n\
        accept;\n\
    } else reject;\n\
}\n\n\
function dn42_export_filter(int link_latency; int link_bandwidth; int link_crypto) {\n\
    if is_valid_network() || is_valid_network_v6() then {\n\
        update_flags(link_latency, link_bandwidth, link_crypto);\n\
        bgp_med = bgp_med + 4 * link_crypto;\n\
        bgp_med = bgp_med + 9 * link_bandwidth;\n\
        bgp_med = bgp_med + link_latency;\n\
        accept;\n\
    } else reject;\n\
}\n\n\
".to_string()
}
```

- [ ] **Step 4: Wire community functions into generate_full_config()**

In `generate_full_config()` (line 182), add after `generate_filter_functions` call (line 204):

```rust
    // Community filter functions (DN42 standard AS 64511)
    config.push_str(&generate_community_functions());
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib services::bird::tests 2>&1 | tail -20`
Expected: all tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/services/bird.rs
git commit -m "feat(bird): add DN42 standard community filter functions (AS 64511)"
```

---

### Task 3: Wire CommunityMapper into peer block generation

**Files:**
- Modify: `src/services/bird.rs:182-268` (generate_full_config signature and body)
- Modify: `src/grpc/peer_service.rs:60-107` (auto_apply_wg_bird)
- Test: inline tests

- [ ] **Step 1: Write failing test for community-aware peer blocks**

Add to `src/services/bird.rs` tests:

```rust
    #[test]
    fn test_generate_peer_block_with_community_tiers() {
        let block = generate_peer_block_with_communities(
            &test_peer(),
            &test_settings(),
            &["4242420000,10".into()],
            &["4242420000,610".into()],
        );
        // Should contain import/export filter calls with tier values
        assert!(block.contains("dn42_import_filter") || block.contains("export filter"));
    }
```

- [ ] **Step 2: Run test to verify behavior**

Run: `cargo test --lib services::bird::tests::test_generate_peer_block_with_community_tiers 2>&1 | tail -10`

- [ ] **Step 3: Modify generate_full_config to accept community data**

Change the signature of `generate_full_config` at line 182:

```rust
/// Generate a complete BIRD2 configuration with template, filters, and all peer blocks.
/// `peer_communities` maps peer ID to (v4_communities, v6_communities).
pub fn generate_full_config(
    peers: &[Peer],
    settings: &Settings,
    template_body: &str,
    peer_communities: &std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
) -> String {
```

Update the peer block generation loop (line 262-265):

```rust
    // Peer blocks
    for peer in peers.iter().filter(|p| p.enabled) {
        let (v4, v6) = peer_communities
            .get(&peer.id)
            .map(|(a, b)| (a.as_slice(), b.as_slice()))
            .unwrap_or((&[], &[]));
        config.push_str(&generate_peer_block_with_communities(peer, settings, v4, v6));
    }
```

- [ ] **Step 4: Update all callers of generate_full_config**

In `src/grpc/peer_service.rs` line 88, change:

```rust
        let mut bird_config = crate::services::bird::generate_full_config(&peers, &settings, "", &std::collections::HashMap::new());
```

Search for other callers with: `grep -rn "generate_full_config" src/`

Update each caller to pass `&std::collections::HashMap::new()` as the 4th argument.

- [ ] **Step 5: Run tests to verify compilation**

Run: `cargo test --lib services::bird::tests 2>&1 | tail -20`
Expected: all tests PASS (existing tests pass empty HashMap)

- [ ] **Step 6: Commit**

```bash
git add src/services/bird.rs src/grpc/peer_service.rs
git commit -m "feat(bird): add peer_communities parameter to generate_full_config"
```

---

### Task 4: Add enable_community_filters setting

**Files:**
- Modify: `proto/peerman.proto:70-93` (Settings message)
- Modify: `src/models/settings.rs` (Settings struct)
- Modify: `src/services/bird.rs` (use setting in config generation)
- Modify: `frontend/src/components/settings/SettingsForm.tsx`

- [ ] **Step 1: Add field to proto**

In `proto/peerman.proto`, add after line 92 (`string bird_import_filter = 19;`):

```protobuf
  // Community filters
  bool enable_community_filters = 20;
```

- [ ] **Step 2: Regenerate proto stubs**

Run: `cargo build 2>&1 | head -20`

- [ ] **Step 3: Add field to Settings model**

In `src/models/settings.rs`, add the field to the `Settings` struct and the `From`/`Into` implementations.

- [ ] **Step 4: Update generate_full_config to use community functions conditionally**

In `generate_full_config`, only add community functions when enabled:

```rust
    // Community filter functions (DN42 standard AS 64511)
    if settings.enable_community_filters {
        config.push_str(&generate_community_functions());
    }
```

- [ ] **Step 5: Add frontend toggle**

In `frontend/src/components/settings/SettingsForm.tsx`, add a toggle for `enable_community_filters`.

- [ ] **Step 6: Run full test suite**

Run: `cargo test 2>&1 | tail -20`
Expected: all tests PASS

- [ ] **Step 7: Commit**

```bash
git add proto/peerman.proto src/models/settings.rs src/services/bird.rs frontend/src/components/settings/SettingsForm.tsx
git commit -m "feat(settings): add enable_community_filters toggle"
```

---

### Task 5: Integrate CommunityMapper into auto_apply_wg_bird

**Files:**
- Modify: `src/grpc/peer_service.rs:60-107`

- [ ] **Step 1: Add CommunityMapper computation in auto_apply_wg_bird**

Replace the BIRD config generation section in `auto_apply_wg_bird` (lines 87-104):

```rust
        // 2. BIRD: full regenerate bird.conf + apply
        let settings_val = settings.clone();

        // Compute communities for each peer if enabled
        let mut peer_communities = std::collections::HashMap::new();
        if settings_val.enable_community_filters {
            let probe_repo = crate::models::probe::ProbeResultRepository::new(/* pool clone */);
            let rule_repo = crate::models::community::CommunityRuleRepository::new(/* pool clone */);
            let local_node_id = self.listen_addr.clone();

            for peer in &peers {
                if !peer.enabled { continue; }
                match crate::services::community_mapper::CommunityMapper::compute_communities(
                    peer, &local_node_id, &probe_repo, &rule_repo,
                ).await {
                    Ok((v4, v6)) => { peer_communities.insert(peer.id.clone(), (v4, v6)); }
                    Err(_) => {} // Skip on error, use empty communities
                }
            }
        }

        let mut bird_config = crate::services::bird::generate_full_config(
            &peers, &settings_val, "", &peer_communities,
        );
```

Note: The actual implementation needs to pass the database pool to create repositories. Check how the service gets its pool reference.

- [ ] **Step 2: Run tests**

Run: `cargo test 2>&1 | tail -20`

- [ ] **Step 3: Commit**

```bash
git add src/grpc/peer_service.rs
git commit -m "feat(peer): wire CommunityMapper into auto_apply_wg_bird"
```

---

## Phase 2: BFD + Cross-node Looking Glass

### Task 6: Add BFD support to BIRD config generation

**Files:**
- Modify: `proto/peerman.proto` (Settings message)
- Modify: `src/models/settings.rs`
- Modify: `src/services/bird.rs`
- Test: inline tests

- [ ] **Step 1: Write failing test for BFD**

Add to `src/services/bird.rs` tests:

```rust
    #[test]
    fn test_generate_full_config_has_bfd_when_enabled() {
        let mut s = test_settings();
        s.enable_bfd = true;
        s.bfd_interval_ms = 300;
        s.bfd_multiplier = 3;
        let config = generate_full_config(&[], &s, "", &std::collections::HashMap::new());
        assert!(config.contains("protocol bfd"));
        assert!(config.contains("interval 300ms"));
        assert!(config.contains("multiplier 3"));
    }

    #[test]
    fn test_generate_full_config_no_bfd_when_disabled() {
        let mut s = test_settings();
        s.enable_bfd = false;
        let config = generate_full_config(&[], &s, "", &std::collections::HashMap::new());
        assert!(!config.contains("protocol bfd"));
    }
```

- [ ] **Step 2: Add BFD fields to proto**

In `proto/peerman.proto`, add after `enable_community_filters`:

```protobuf
  // BFD
  bool enable_bfd = 21;
  uint32 bfd_interval_ms = 22;    // default 300
  uint32 bfd_multiplier = 23;     // default 3
```

- [ ] **Step 3: Update Settings model**

Add `enable_bfd`, `bfd_interval_ms`, `bfd_multiplier` fields to `src/models/settings.rs`.

- [ ] **Step 4: Implement BFD block generation**

Add to `src/services/bird.rs`:

```rust
fn generate_bfd_section(settings: &Settings) -> String {
    if !settings.enable_bfd {
        return String::new();
    }
    let interval = if settings.bfd_interval_ms > 0 { settings.bfd_interval_ms } else { 300 };
    let multiplier = if settings.bfd_multiplier > 0 { settings.bfd_multiplier } else { 3 };
    format!(
        "protocol bfd {{\n\
         \x20   interface \"wg*\" {{\n\
         \x20       interval {interval}ms;\n\
         \x20       multiplier {multiplier};\n\
         \x20   }};\n\
         }}\n\n"
    )
}
```

In `generate_full_config()`, add after the community functions section:

```rust
    // BFD
    config.push_str(&generate_bfd_section(settings));
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib services::bird::tests 2>&1 | tail -20`
Expected: all tests PASS

- [ ] **Step 6: Commit**

```bash
git add proto/peerman.proto src/models/settings.rs src/services/bird.rs
git commit -m "feat(bird): add BFD support for fast failure detection"
```

---

### Task 7: Implement cross-node BIRD command execution

**Files:**
- Modify: `src/grpc/bird_service.rs` (add cluster_key, node_repo fields, implement forwarding)
- Modify: `src/cluster/aggregator.rs` (add execute_bird_command method)

- [ ] **Step 1: Add execute_bird_command to ClusterAggregator**

Add to `src/cluster/aggregator.rs`:

```rust
    /// Execute a BIRD command on a specific remote node.
    pub async fn execute_bird_command(
        &self,
        node_addr: &str,
        command: &str,
        jwt_secret: &str,
    ) -> Result<String, String> {
        use crate::grpc::generated::bird_service_client::BirdServiceClient;
        use crate::grpc::generated::ExecuteCommandRequest;

        let uri = format!("http://{}", node_addr);
        let channel = Endpoint::from_shared(uri)
            .map_err(|e| format!("invalid uri: {e}"))?
            .connect()
            .await
            .map_err(|e| format!("connect failed: {e}"))?;

        let mut client = BirdServiceClient::new(channel);
        let mut req = Request::new(ExecuteCommandRequest {
            command: command.to_string(),
            target_node_id: String::new(), // empty = local on remote node
        });

        // Use cluster key auth for inter-node
        if !self.cluster_key.is_empty() {
            if let Ok(val) = self.cluster_key.parse() {
                req.metadata_mut().insert("x-cluster-key", val);
            }
        }

        let response = timeout(FANOUT_TIMEOUT, client.execute_command(req))
            .await
            .map_err(|_| "timeout".to_string())?
            .map_err(|e| format!("rpc: {e}"))?;

        let results = response.into_inner().results;
        results.first()
            .filter(|r| r.status_code == 0)
            .map(|r| r.output.clone())
            .ok_or_else(|| results.first()
                .map(|r| r.error.clone())
                .unwrap_or_else(|| "no result".to_string()))
    }
```

- [ ] **Step 2: Update BirdServiceImpl to include cluster dependencies**

In `src/grpc/bird_service.rs`, add fields:

```rust
pub struct BirdServiceImpl {
    pub node_name: String,
    pub jwt_secret: std::sync::Arc<String>,
    pub cluster_key: std::sync::Arc<String>,
    pub node_repo: crate::models::node::NodeRepository,
    pub cache: crate::cluster::cache::ClusterCache,
}
```

- [ ] **Step 3: Implement cross-node forwarding**

Replace the stub in `execute_command` (lines 34-43):

```rust
        } else {
            // Remote node — forward via cluster
            let aggregator = crate::cluster::aggregator::ClusterAggregator::new(
                self.cache.clone(),
                self.cluster_key.as_ref().clone(),
            );

            // Look up target node address
            let nodes = self.node_repo.list_all().await
                .map_err(|e| Status::internal(e.to_string()))?;
            let target_node = nodes.iter()
                .find(|n| n.name == req.target_node_id || n.id == req.target_node_id)
                .ok_or_else(|| Status::not_found(format!("node {} not found", req.target_node_id)))?;

            match aggregator.execute_bird_command(
                &target_node.listen_addr,
                &req.command,
                &self.jwt_secret,
            ).await {
                Ok(output) => vec![NodeBirdResult {
                    node_id: req.target_node_id.clone(),
                    node_name: target_node.name.clone(),
                    output,
                    status_code: 0,
                    error: String::new(),
                }],
                Err(e) => vec![NodeBirdResult {
                    node_id: req.target_node_id.clone(),
                    node_name: target_node.name.clone(),
                    output: String::new(),
                    status_code: 1,
                    error: e,
                }],
            }
        };
```

- [ ] **Step 4: Update BirdServiceImpl instantiation in main.rs**

Find where `BirdServiceImpl` is created and add the new fields.

- [ ] **Step 5: Run tests**

Run: `cargo test 2>&1 | tail -20`

- [ ] **Step 6: Commit**

```bash
git add src/grpc/bird_service.rs src/cluster/aggregator.rs
git commit -m "feat(bird): implement cross-node BIRD command execution"
```

---

## Phase 3: WireGuard Lifecycle + IPv6 Cluster Tunnels

### Task 8: Add WireGuard interface lifecycle management

**Files:**
- Modify: `proto/peerman.proto` (PeerService)
- Modify: `src/services/wireguard.rs`
- Modify: `src/grpc/peer_service.rs`

- [ ] **Step 1: Add RPCs to proto**

In `proto/peerman.proto`, add to PeerService:

```protobuf
  rpc ApplyWireGuard(ApplyWireGuardRequest) returns (ApplyWireGuardResponse);
  rpc RestartWireGuard(RestartWireGuardRequest) returns (RestartWireGuardResponse);
```

Add messages:

```protobuf
message ApplyWireGuardRequest {
  string interface_name = 1;  // default "wg0"
}

message ApplyWireGuardResponse {}

message RestartWireGuardRequest {
  string interface_name = 1;
}

message RestartWireGuardResponse {}
```

- [ ] **Step 2: Implement lifecycle methods**

Add to `src/services/wireguard.rs`:

```rust
/// Restart a WireGuard interface (down + up).
pub fn restart_interface(iface: &str) -> Result<(), crate::error::AppError> {
    let _ = std::process::Command::new("wg-quick")
        .args(["down", iface])
        .output(); // Ignore error on down (interface may not be up)

    let output = std::process::Command::new("wg-quick")
        .args(["up", iface])
        .output()
        .map_err(|e| crate::error::AppError::Internal(format!("wg-quick up failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(crate::error::AppError::Internal(
            format!("wg-quick up {iface} failed: {stderr}")
        ));
    }
    Ok(())
}
```

- [ ] **Step 3: Implement gRPC handlers**

Add to `impl PeerService for PeerServiceImpl` in `src/grpc/peer_service.rs`:

```rust
    async fn apply_wire_guard(
        &self,
        request: Request<ApplyWireGuardRequest>,
    ) -> Result<Response<ApplyWireGuardResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        let iface = if req.interface_name.is_empty() { "wg0" } else { &req.interface_name };
        crate::services::wireguard::restart_interface(iface)
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(ApplyWireGuardResponse {}))
    }

    async fn restart_wire_guard(
        &self,
        request: Request<RestartWireGuardRequest>,
    ) -> Result<Response<RestartWireGuardResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        let iface = if req.interface_name.is_empty() { "wg0" } else { &req.interface_name };
        crate::services::wireguard::restart_interface(iface)
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(RestartWireGuardResponse {}))
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1 | tail -20`

- [ ] **Step 5: Commit**

```bash
git add proto/peerman.proto src/services/wireguard.rs src/grpc/peer_service.rs
git commit -m "feat(wg): add WireGuard interface lifecycle management"
```

---

### Task 9: Add IPv6 cluster tunnel support

**Files:**
- Modify: `proto/peerman.proto` (NodeInfo, Settings)
- Modify: `src/models/node.rs` (add tunnel_ipv6 field)
- Modify: `src/cluster/tunnel.rs` (IPv6 assignment)
- Modify: `src/services/bird.rs` (IPv6 iBGP blocks)
- Modify: `migrations/` (add tunnel_ipv6 column)

- [ ] **Step 1: Add proto fields**

In `proto/peerman.proto`:

```protobuf
message NodeInfo {
  // ... existing fields ...
  string tunnel_ipv6 = 8;
}
```

In Settings:
```protobuf
  string cluster_tunnel_ipv6_range = 24;  // e.g. "fd42:cluster::/48"
```

- [ ] **Step 2: Add migration**

Create `migrations/006_ipv6_tunnels.sql`:

```sql
ALTER TABLE nodes ADD COLUMN tunnel_ipv6 TEXT NOT NULL DEFAULT '';
```

- [ ] **Step 3: Update Node model**

Add `tunnel_ipv6: String` to `src/models/node.rs` Node struct.

- [ ] **Step 4: Implement IPv6 tunnel assignment**

In `src/cluster/tunnel.rs`, extend `init_local_node()` to assign IPv6:

```rust
    // Assign IPv6 tunnel IP if range configured
    if !settings.cluster_tunnel_ipv6_range.is_empty() {
        let ipv6_prefix = &settings.cluster_tunnel_ipv6_range;
        // Assign from prefix + node index (similar to IPv4 logic)
        let tunnel_ipv6 = assign_ipv6_tunnel_ip(&pool, ipv6_prefix).await?;
        node.tunnel_ipv6 = tunnel_ipv6;
    }
```

- [ ] **Step 5: Update cluster WG config for IPv6**

In `generate_cluster_wg_config()`, add IPv6 AllowedIPs:

```rust
    // Add IPv6 tunnel IP if available
    if !node.tunnel_ipv6.is_empty() {
        peer_block.push_str(&format!("AllowedIPs = {}/128\n", node.tunnel_ipv6));
    }
```

- [ ] **Step 6: Update iBGP blocks for IPv6**

In `generate_ibgp_blocks()`, if a node has `tunnel_ipv6`, add IPv6 neighbor:

```rust
    if !node.tunnel_ipv6.is_empty() {
        blocks.push_str(&format!(
            "    neighbor {tunnel_ipv6} as {local_asn};\n",
            tunnel_ipv6 = node.tunnel_ipv6,
            local_asn = settings.local_asn
        ));
    }
```

- [ ] **Step 7: Run tests**

Run: `cargo test 2>&1 | tail -20`

- [ ] **Step 8: Commit**

```bash
git add proto/peerman.proto src/models/node.rs src/cluster/tunnel.rs src/services/bird.rs migrations/
git commit -m "feat(cluster): add IPv6 tunnel support for cluster interconnect"
```

---

## Phase 4: BGP Confederation (Optional)

### Task 10: Add BGP Confederation support

**Files:**
- Modify: `proto/peerman.proto` (Settings)
- Modify: `src/models/settings.rs`
- Modify: `src/services/bird.rs`
- Test: inline tests

- [ ] **Step 1: Write failing test**

```rust
    #[test]
    fn test_generate_ibgp_blocks_confederation() {
        let mut s = test_settings();
        s.enable_confederation = true;
        s.confederation_local_asn = 65000;
        let nodes = vec![
            crate::models::node::Node {
                id: "n1".into(), name: "node-a".into(),
                listen_addr: "1.2.3.4:3000".into(), local_asn: 4242420000,
                description: None, online: true,
                last_seen_at: String::new(), created_at: String::new(),
                updated_at: String::new(),
                wg_pubkey: "pk-a".into(), tunnel_ip: "10.255.0.1".into(),
            },
        ];
        let blocks = generate_ibgp_blocks(&nodes, &s, "10.255.0.2");
        assert!(blocks.contains("confederation"));
        assert!(blocks.contains("confederation member yes"));
    }
```

- [ ] **Step 2: Add proto fields**

```protobuf
  // BGP Confederation
  bool enable_confederation = 25;
  int64 confederation_local_asn = 26;
```

- [ ] **Step 3: Implement confederation-aware config generation**

In `generate_full_config()`, when `enable_confederation` is true:
- Add `confederation <local_asn>;` and `confederation member yes;` to the template
- Change iBGP blocks to use `neighbor <ip> external;` instead of `neighbor <ip> as <asn>;`

- [ ] **Step 4: Run tests**

Run: `cargo test 2>&1 | tail -20`

- [ ] **Step 5: Commit**

```bash
git add proto/peerman.proto src/models/settings.rs src/services/bird.rs
git commit -m "feat(bird): add BGP Confederation support for multi-node AS"
```

---

## Verification Checklist

After all phases:

- [ ] `cargo test` — all unit tests pass
- [ ] `cargo clippy` — no warnings
- [ ] `cd frontend && pnpm exec tsc --noEmit` — TypeScript type-check passes
- [ ] `SKIP_FRONTEND_BUILD=1 cargo build` — builds successfully
- [ ] Manual test: generate bird.conf with community filters enabled, verify DN42 standard functions present
- [ ] Manual test: Looking Glass executes command on remote node
- [ ] Manual test: WireGuard restart works from frontend
