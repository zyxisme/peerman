# WG & BIRD Direct Management — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Auto-apply WG + BIRD config on peer CRUD, plus auto-manage cluster inter-node WG tunnels and iBGP full mesh.

**Architecture:** New cluster tunnel module manages inter-node WG keypairs and peer config. Peer CRUD hooks call into WG/BIRD apply functions. New ManagementService gRPC exposes WG/BIRD status. Frontend adds read-only Status page.

**Tech Stack:** Rust (tonic 0.12, axum 0.7, sqlx, tokio), TypeScript (React, @connectrpc/connect v2), Protobuf, SQLite.

---

### Task 1: Proto — Add ManagementService + NodeInfo fields

**Files:**
- Modify: `proto/peerman.proto`
- Regenerate: `frontend/src/lib/peerman_pb.ts`
- Regenerate: Rust generated code (build.rs handles this automatically)

- [ ] **Step 1: Add ManagementService messages and service to proto**

Add after the `NodeTracerouteResult` message (before `FlapService`):

```proto
// ── Management Service ──

service ManagementService {
  rpc GetWireGuardStatus(GetWGStatusRequest) returns (WGStatusResponse);
  rpc GetBirdStatus(GetBirdStatusRequest) returns (BirdStatusResponse);
}

message GetWGStatusRequest {
  string interface = 1;
}

message WGStatusResponse {
  repeated WGInterface interfaces = 1;
}

message WGInterface {
  string name = 1;
  string public_key = 2;
  uint32 listen_port = 3;
  repeated WGPeerStatus peers = 4;
}

message WGPeerStatus {
  string public_key = 1;
  string endpoint = 2;
  string allowed_ips = 3;
  string latest_handshake = 4;
  string transfer_rx = 5;
  string transfer_tx = 6;
}

message GetBirdStatusRequest {}

message BirdStatusResponse {
  repeated BirdProtocol protocols = 1;
}

message BirdProtocol {
  string name = 1;
  string proto = 2;
  string table = 3;
  string state = 4;
  string since = 5;
  string info = 6;
}
```

Add `wg_public_key` and `tunnel_ip` fields to `NodeInfo`:

```proto
message NodeInfo {
  string name = 1;
  string listen_addr = 2;
  int64 local_asn = 3;
  string description = 4;
  string last_seen_at = 5;
  string wg_public_key = 6;   // new
  string tunnel_ip = 7;        // new
}
```

- [ ] **Step 2: Regenerate frontend TypeScript stubs**

Run: `PATH="frontend/node_modules/.bin:$PATH" protoc -I proto --es_out frontend/src/lib --es_opt target=ts proto/peerman.proto`

- [ ] **Step 3: Verify Rust stubs compile**

Run: `cargo check 2>&1 | head -20`
Expected: should compile successfully (build.rs auto-regenerates Rust proto code)

- [ ] **Step 4: Commit**

```bash
git add proto/peerman.proto frontend/src/lib/peerman_pb.ts
git commit -m "feat(proto): add ManagementService, WG/BIRD status messages, NodeInfo wg_pubkey+tunnel_ip

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2: Config — Add tunnel_ip_range to ClusterConfig

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add tunnel_ip_range field and default**

In `src/config.rs`, add after `sync_interval_secs` in `ClusterConfig`:

```rust
#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
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
    #[serde(default)]
    pub tunnel_ip_range: String,     // new
}
```

Add the field in `Default for ClusterConfig`:

```rust
impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            node_name: String::new(),
            cluster_key: String::new(),
            peer_nodes: Vec::new(),
            probe_interval_secs: default_probe_interval(),
            sync_interval_secs: default_sync_interval(),
            tunnel_ip_range: String::new(),  // new
        }
    }
}
```

- [ ] **Step 2: Verify compile**

Run: `cargo check 2>&1 | head -20`

- [ ] **Step 3: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): add tunnel_ip_range to ClusterConfig

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 3: DB Migration — Add wg_pubkey and tunnel_ip to nodes

**Files:**
- Create: `migrations/006_cluster_wg.sql`

- [ ] **Step 1: Create migration SQL**

```sql
ALTER TABLE nodes ADD COLUMN wg_pubkey TEXT NOT NULL DEFAULT '';
ALTER TABLE nodes ADD COLUMN tunnel_ip TEXT NOT NULL DEFAULT '';
```

- [ ] **Step 2: Verify migration runs**

Run: `sqlite3 /tmp/test-peerman.db < migrations/002_cluster.sql && sqlite3 /tmp/test-peerman.db < migrations/006_cluster_wg.sql && echo "OK"`
Expected: "OK"

- [ ] **Step 3: Commit**

```bash
git add migrations/006_cluster_wg.sql
git commit -m "feat(db): add wg_pubkey and tunnel_ip columns to nodes

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 4: Node Model — Add wg_pubkey and tunnel_ip fields

**Files:**
- Modify: `src/models/node.rs`

- [ ] **Step 1: Add fields to Node struct**

Add `wg_pubkey` and `tunnel_ip` after `updated_at`:

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub listen_addr: String,
    pub local_asn: i64,
    pub description: Option<String>,
    pub online: bool,
    pub last_seen_at: String,
    pub created_at: String,
    pub updated_at: String,
    pub wg_pubkey: String,     // new
    pub tunnel_ip: String,     // new
}
```

- [ ] **Step 2: Update list_all query**

Change the SELECT in `list_all` to include new columns:

```rust
pub async fn list_all(&self) -> Result<Vec<Node>, AppError> {
    sqlx::query_as::<_, Node>(
        "SELECT id, name, listen_addr, local_asn, description, online,
         last_seen_at, created_at, updated_at, wg_pubkey, tunnel_ip
         FROM nodes ORDER BY name",
    )
    .fetch_all(&self.pool)
    .await
    .map_err(Into::into)
}
```

- [ ] **Step 3: Update find_by_id, find_by_listen_addr, find_by_name queries**

Same pattern — add `wg_pubkey, tunnel_ip` to the SELECT columns in `find_by_id`, `find_by_listen_addr`, and `find_by_name`.

- [ ] **Step 4: Update create RETURNING clause**

```rust
pub async fn create(
    &self,
    name: &str,
    listen_addr: &str,
    local_asn: i64,
    description: &str,
) -> Result<Node, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query_as::<_, Node>(
        "INSERT INTO nodes (id, name, listen_addr, local_asn, description, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         RETURNING id, name, listen_addr, local_asn, description, online,
         last_seen_at, created_at, updated_at, wg_pubkey, tunnel_ip",
    )
    .bind(&id)
    .bind(name)
    .bind(listen_addr)
    .bind(local_asn)
    .bind(description)
    .bind(&now)
    .bind(&now)
    .fetch_one(&self.pool)
    .await
    .map_err(Into::into)
}
```

- [ ] **Step 5: Update update RETURNING clause**

```rust
pub async fn update(&self, node: &Node) -> Result<Node, AppError> {
    let now = Utc::now().to_rfc3339();

    sqlx::query_as::<_, Node>(
        "UPDATE nodes SET name = ?, listen_addr = ?, local_asn = ?, description = ?,
         updated_at = ?, wg_pubkey = ?, tunnel_ip = ?
         WHERE id = ?
         RETURNING id, name, listen_addr, local_asn, description, online,
         last_seen_at, created_at, updated_at, wg_pubkey, tunnel_ip",
    )
    .bind(&node.name)
    .bind(&node.listen_addr)
    .bind(node.local_asn)
    .bind(&node.description)
    .bind(&now)
    .bind(&node.wg_pubkey)
    .bind(&node.tunnel_ip)
    .bind(&node.id)
    .fetch_one(&self.pool)
    .await
    .map_err(Into::into)
}
```

- [ ] **Step 6: Add update_cluster_fields method**

```rust
pub async fn update_cluster_fields(
    &self,
    id: &str,
    wg_pubkey: &str,
    tunnel_ip: &str,
) -> Result<(), AppError> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE nodes SET wg_pubkey = ?, tunnel_ip = ?, updated_at = ? WHERE id = ?",
    )
    .bind(wg_pubkey)
    .bind(tunnel_ip)
    .bind(&now)
    .bind(id)
    .execute(&self.pool)
    .await?;
    Ok(())
}
```

- [ ] **Step 7: Verify compile**

Run: `cargo check 2>&1 | head -20`

- [ ] **Step 8: Verify tests**

Run: `cargo test 2>&1 | tail -5`
Expected: all tests pass

- [ ] **Step 9: Commit**

```bash
git add src/models/node.rs
git commit -m "feat(node): add wg_pubkey and tunnel_ip fields + update_cluster_fields

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 5: WireGuard Service — Apply + Status + Cluster Config

**Files:**
- Modify: `src/services/wireguard.rs`

- [ ] **Step 1: Write failing tests for apply_syncconf and get_wg_status**

Add at the bottom of the test module in `src/services/wireguard.rs`:

```rust
#[test]
fn test_generate_cluster_wg_config_has_interface_and_peers() {
    let nodes = vec![
        crate::models::node::Node {
            id: "n1".into(), name: "node-a".into(),
            listen_addr: "1.2.3.4:3000".into(), local_asn: 4242420000,
            description: None, online: true,
            last_seen_at: String::new(), created_at: String::new(),
            updated_at: String::new(),
            wg_pubkey: "pubkey-a".into(), tunnel_ip: "10.255.0.1".into(),
        },
        crate::models::node::Node {
            id: "n2".into(), name: "node-b".into(),
            listen_addr: "5.6.7.8:3000".into(), local_asn: 4242420001,
            description: None, online: true,
            last_seen_at: String::new(), created_at: String::new(),
            updated_at: String::new(),
            wg_pubkey: "pubkey-b".into(), tunnel_ip: "10.255.0.2".into(),
        },
    ];
    let config = generate_cluster_wg_config(&nodes, "key-a", 51821);
    assert!(config.contains("[Interface]"));
    assert!(config.contains("PrivateKey = key-a"));
    assert!(config.contains("ListenPort = 51821"));
    assert!(config.contains("[Peer]"));
    assert!(config.contains("PublicKey = pubkey-b"));
    assert!(config.contains("Endpoint = 5.6.7.8:51821"));
}

#[test]
fn test_generate_cluster_wg_config_empty_nodes() {
    let nodes: Vec<crate::models::node::Node> = vec![];
    let config = generate_cluster_wg_config(&nodes, "key-a", 51821);
    assert!(config.contains("[Interface]"));
    assert!(!config.contains("[Peer]"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test wireguard -- --nocapture 2>&1 | tail -20`
Expected: FAIL — `generate_cluster_wg_config` not found

- [ ] **Step 3: Add apply_syncconf, get_wg_status, generate_cluster_wg_config functions**

Add these after the `generate_keypair` function, before the `#[cfg(test)]` block:

```rust
/// Apply WG config using wg syncconf — differential update without down/up.
pub fn apply_syncconf(interface: &str, config_path: &str) -> Result<(), AppError> {
    let status = std::process::Command::new("wg")
        .args(["syncconf", interface, config_path])
        .output()
        .map_err(|e| AppError::Internal(format!("wg syncconf failed: {e}")))?;

    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr);
        return Err(AppError::Internal(format!("wg syncconf error: {stderr}")));
    }
    Ok(())
}

/// Parse `wg show <interface> dump` output into structured status.
pub fn get_wg_status(interface: &str) -> Result<Vec<crate::grpc::generated::WgInterface>, AppError> {
    let output = std::process::Command::new("wg")
        .args(["show", interface, "dump"])
        .output()
        .map_err(|e| AppError::Internal(format!("wg show failed: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut interfaces: Vec<crate::grpc::generated::WgInterface> = Vec::new();
    let mut current_iface: Option<crate::grpc::generated::WgInterface> = None;

    for line in stdout.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.is_empty() {
            continue;
        }
        // First field determines line type
        match fields[0] {
            // Interface line: private_key, public_key, listen_port, fwmark
            key if fields.len() >= 4 && !key.is_empty() => {
                if let Some(iface) = current_iface.take() {
                    interfaces.push(iface);
                }
                current_iface = Some(crate::grpc::generated::WgInterface {
                    name: interface.to_string(),
                    public_key: fields[1].to_string(),
                    listen_port: fields[2].parse().unwrap_or(0),
                    peers: Vec::new(),
                });
            }
            // Peer line: public_key, preshared_key, endpoint, allowed_ips, latest_handshake, transfer_rx, transfer_tx, persistent_keepalive
            peer_key if fields.len() >= 8 => {
                if let Some(ref mut iface) = current_iface {
                    iface.peers.push(crate::grpc::generated::WgPeerStatus {
                        public_key: fields[0].to_string(),
                        endpoint: fields.get(2).map(|s| s.to_string()).unwrap_or_default(),
                        allowed_ips: fields.get(3).map(|s| s.to_string()).unwrap_or_default(),
                        latest_handshake: fields.get(4).map(|s| s.to_string()).unwrap_or_default(),
                        transfer_rx: fields.get(5).map(|s| s.to_string()).unwrap_or_default(),
                        transfer_tx: fields.get(6).map(|s| s.to_string()).unwrap_or_default(),
                    });
                }
            }
            _ => {}
        }
    }
    if let Some(iface) = current_iface {
        interfaces.push(iface);
    }
    Ok(interfaces)
}

/// Generate WG config for the cluster interconnect interface (wg-cluster).
pub fn generate_cluster_wg_config(
    nodes: &[crate::models::node::Node],
    private_key: &str,
    listen_port: u16,
) -> String {
    let mut config = String::new();

    config.push_str("[Interface]\n");
    config.push_str(&format!("PrivateKey = {private_key}\n"));
    config.push_str(&format!("ListenPort = {listen_port}\n"));
    config.push_str("Table = off\n\n");

    for node in nodes {
        if node.wg_pubkey.is_empty() {
            continue;
        }
        config.push_str("[Peer]\n");
        config.push_str(&format!("PublicKey = {}\n", node.wg_pubkey));
        // Extract host from listen_addr (strip port, keep host)
        let host = node.listen_addr.rsplit_once(':')
            .map(|(h, _)| h)
            .unwrap_or(&node.listen_addr);
        config.push_str(&format!("Endpoint = {host}:{listen_port}\n"));
        // Allow only tunnel IP range traffic through this interface
        config.push_str(&format!("AllowedIPs = {}/32\n", node.tunnel_ip));
        config.push_str("PersistentKeepalive = 25\n\n");
    }

    config
}
```

Add the import at the top of the file:
```rust
use crate::error::AppError;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test wireguard -- --nocapture 2>&1 | tail -20`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/services/wireguard.rs
git commit -m "feat(wireguard): add apply_syncconf, get_wg_status, generate_cluster_wg_config

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 6: BIRD Service — Apply + Status + iBGP blocks

**Files:**
- Modify: `src/services/bird.rs`

- [ ] **Step 1: Write failing test for generate_ibgp_blocks**

Add to the test module:

```rust
#[test]
fn test_generate_ibgp_blocks_creates_protocol_blocks() {
    let nodes = vec![
        crate::models::node::Node {
            id: "n1".into(), name: "node-a".into(),
            listen_addr: "1.2.3.4:3000".into(), local_asn: 4242420000,
            description: None, online: true,
            last_seen_at: String::new(), created_at: String::new(),
            updated_at: String::new(),
            wg_pubkey: "pk-a".into(), tunnel_ip: "10.255.0.1".into(),
        },
        crate::models::node::Node {
            id: "n2".into(), name: "node-b".into(),
            listen_addr: "5.6.7.8:3000".into(), local_asn: 4242420000,
            description: None, online: true,
            last_seen_at: String::new(), created_at: String::new(),
            updated_at: String::new(),
            wg_pubkey: "pk-b".into(), tunnel_ip: "10.255.0.2".into(),
        },
    ];
    let settings = test_settings();
    let blocks = generate_ibgp_blocks(&nodes, &settings, "10.255.0.1");
    // Should contain 1 block (to node-b, skip self)
    assert!(blocks.contains("protocol bgp node_node_b from"));
    assert!(blocks.contains("neighbor 10.255.0.2 as 4242420000"));
    assert!(blocks.contains("next hop self yes"));
}

#[test]
fn test_generate_ibgp_blocks_skips_self_and_no_tunnel_ip() {
    let nodes = vec![
        crate::models::node::Node {
            id: "n1".into(), name: "self-node".into(),
            listen_addr: "1.2.3.4:3000".into(), local_asn: 4242420000,
            description: None, online: true,
            last_seen_at: String::new(), created_at: String::new(),
            updated_at: String::new(),
            wg_pubkey: "pk-a".into(), tunnel_ip: "10.255.0.1".into(),
        },
        crate::models::node::Node {
            id: "n2".into(), name: "no-tunnel".into(),
            listen_addr: "5.6.7.8:3000".into(), local_asn: 4242420000,
            description: None, online: false,
            last_seen_at: String::new(), created_at: String::new(),
            updated_at: String::new(),
            wg_pubkey: String::new(), tunnel_ip: String::new(),
        },
    ];
    let settings = test_settings();
    let blocks = generate_ibgp_blocks(&nodes, &settings, "10.255.0.1");
    assert!(blocks.is_empty()); // no other valid node
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test bird -- --nocapture 2>&1 | tail -20`
Expected: FAIL

- [ ] **Step 3: Add apply_config, get_bird_status, generate_ibgp_blocks functions**

Add these after the existing `generate_full_config` function, before the `sanitize_name` helper:

```rust
/// Write bird.conf to /etc/bird/ and hot-reload via birdc configure.
pub fn apply_config(config: &str) -> Result<(), crate::error::AppError> {
    use std::io::Write;

    let config_path = "/etc/bird/bird.conf";
    let tmp_path = "/etc/bird/bird.conf.tmp";

    // Atomic write: tmp then rename
    {
        let mut f = std::fs::File::create(tmp_path)
            .map_err(|e| crate::error::AppError::Internal(format!("Cannot create bird.conf.tmp: {e}")))?;
        f.write_all(config.as_bytes())
            .map_err(|e| crate::error::AppError::Internal(format!("Cannot write bird.conf.tmp: {e}")))?;
    }
    std::fs::rename(tmp_path, config_path)
        .map_err(|e| crate::error::AppError::Internal(format!("Cannot rename bird.conf: {e}")))?;

    // Hot reload
    let output = std::process::Command::new("birdc")
        .arg("configure")
        .output()
        .map_err(|e| crate::error::AppError::Internal(format!("birdc not found: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        return Err(crate::error::AppError::Internal(
            format!("birdc configure failed: {stdout} {stderr}")
        ));
    }
    Ok(())
}

/// Parse `birdc show protocols` output into structured status.
pub fn get_bird_status() -> Result<Vec<crate::grpc::generated::BirdProtocol>, crate::error::AppError> {
    let output = std::process::Command::new("birdc")
        .args(["show", "protocols"])
        .output()
        .map_err(|e| crate::error::AppError::Internal(format!("birdc failed: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut protocols: Vec<crate::grpc::generated::BirdProtocol> = Vec::new();

    for line in stdout.lines().skip(2) {
        // Format: "name   proto   table   state   since   info"
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            protocols.push(crate::grpc::generated::BirdProtocol {
                name: parts[0].to_string(),
                proto: parts[1].to_string(),
                table: parts[2].to_string(),
                state: parts[3].to_string(),
                since: parts[4].to_string(),
                info: parts.get(5..).map(|s| s.join(" ")).unwrap_or_default(),
            });
        }
    }

    Ok(protocols)
}

/// Generate iBGP full-mesh protocol blocks for cluster nodes.
/// `my_tunnel_ip` is the local node's tunnel IP, used to skip self.
pub fn generate_ibgp_blocks(
    nodes: &[crate::models::node::Node],
    settings: &crate::models::settings::Settings,
    my_tunnel_ip: &str,
) -> String {
    let mut blocks = String::new();

    for node in nodes {
        // Skip self and nodes without tunnel IP
        if node.tunnel_ip == my_tunnel_ip || node.tunnel_ip.is_empty() {
            continue;
        }

        let name = sanitize_name(&node.name);
        blocks.push_str(&format!(
            "protocol bgp node_{name} from {tpl} {{\n",
            tpl = settings.bird_template_name
        ));
        blocks.push_str(&format!(
            "    neighbor {tunnel_ip} as {local_asn};\n",
            tunnel_ip = node.tunnel_ip,
            local_asn = settings.local_asn
        ));
        blocks.push_str("    direct;\n");
        blocks.push_str("    ipv4 {\n");
        blocks.push_str("        next hop self yes;\n");
        blocks.push_str("        import where source = RTS_BGP && is_valid_network() && !is_self_net();\n");
        blocks.push_str("        export where source = RTS_BGP && is_valid_network() && !is_self_net();\n");
        blocks.push_str("    };\n");
        blocks.push_str("    ipv6 {\n");
        blocks.push_str("        next hop self yes;\n");
        blocks.push_str("        import where source = RTS_BGP && is_valid_network_v6() && !is_self_net();\n");
        blocks.push_str("        export where source = RTS_BGP && is_valid_network_v6() && !is_self_net();\n");
        blocks.push_str("    };\n");
        blocks.push_str("}\n\n");
    }

    blocks
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test bird -- --nocapture 2>&1 | tail -20`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/services/bird.rs
git commit -m "feat(bird): add apply_config, get_bird_status, generate_ibgp_blocks

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 7: Cluster Tunnel Module — Keypair + IP assignment + Config sync

**Files:**
- Create: `src/cluster/tunnel.rs`
- Modify: `src/cluster/mod.rs`
- Modify: `src/app_state.rs`

- [ ] **Step 1: Create the tunnel module**

Write `src/cluster/tunnel.rs`:

```rust
use std::net::Ipv4Addr;
use std::str::FromStr;

use crate::error::AppError;
use crate::models::node::{Node, NodeRepository};
use crate::services;

const CLUSTER_WG_INTERFACE: &str = "wg-cluster";
const CLUSTER_WG_PORT: u16 = 51821;

/// Ensure this node has a WG keypair and a tunnel IP assigned.
/// Writes wg_pubkey to the DB. Returns (wg_private_key, wg_pubkey, tunnel_ip).
pub async fn init_local_node(
    node_repo: &NodeRepository,
    node_id: &str,
    tunnel_ip_range: &str,
) -> Result<(String, String, String), AppError> {
    let mut node = node_repo.find_by_id(node_id).await?;

    // Generate WG keypair if missing
    let (priv_key, pub_key) = if node.wg_pubkey.is_empty() {
        let (priv_key, pub_key) = services::wireguard::generate_keypair();
        node_repo.update_cluster_fields(node_id, &pub_key, &node.tunnel_ip).await?;
        (priv_key, pub_key)
    } else {
        // We don't store private key in DB — generate fresh keypair on startup
        // and update the pubkey in DB.
        let (priv_key, pub_key) = services::wireguard::generate_keypair();
        node_repo.update_cluster_fields(node_id, &pub_key, &node.tunnel_ip).await?;
        (priv_key, pub_key)
    };

    // Assign tunnel IP if missing
    let tunnel_ip = if node.tunnel_ip.is_empty() {
        let ip = assign_tunnel_ip(node_repo, tunnel_ip_range).await?;
        node_repo.update_cluster_fields(node_id, &pub_key, &ip).await?;
        ip
    } else {
        node.tunnel_ip.clone()
    };

    Ok((priv_key, pub_key, tunnel_ip))
}

/// Assign the first unused IP from the given range (e.g. "10.255.0.0/24").
async fn assign_tunnel_ip(node_repo: &NodeRepository, range: &str) -> Result<String, AppError> {
    let (base, prefix_len) = range
        .split_once('/')
        .ok_or_else(|| AppError::Internal("invalid tunnel_ip_range format".into()))?;

    let base_ip = Ipv4Addr::from_str(base)
        .map_err(|e| AppError::Internal(format!("invalid tunnel_ip_range base: {e}")))?;
    let prefix_len: u8 = prefix_len
        .parse()
        .map_err(|_| AppError::Internal("invalid tunnel_ip_range prefix".into()))?;

    let base_u32 = u32::from(base_ip);
    let mask = !((1u32 << (32 - prefix_len)) - 1);
    let network = base_u32 & mask;
    let broadcast = network | !mask;

    let all_nodes = node_repo.list_all().await?;
    let used_ips: std::collections::HashSet<String> = all_nodes
        .iter()
        .filter_map(|n| {
            if n.tunnel_ip.is_empty() { None } else { Some(n.tunnel_ip.clone()) }
        })
        .collect();

    // Start from network+1, skip network address and broadcast
    for offset in 1..(broadcast - network) {
        let candidate = Ipv4Addr::from(network + offset);
        let candidate_str = candidate.to_string();
        if !used_ips.contains(&candidate_str) {
            return Ok(candidate_str);
        }
    }

    Err(AppError::Internal("no available IP in tunnel_ip_range".into()))
}

/// Resolve IP collision: the node with lexicographically smaller id keeps the IP.
pub async fn resolve_ip_collisions(
    node_repo: &NodeRepository,
    remote_nodes: &[Node],
) {
    let local_nodes = match node_repo.list_all().await {
        Ok(n) => n,
        Err(_) => return,
    };

    for remote in remote_nodes {
        if remote.tunnel_ip.is_empty() {
            continue;
        }
        // Find local nodes with same tunnel IP
        for local in &local_nodes {
            if local.tunnel_ip.is_empty() || local.id == remote.id {
                continue;
            }
            if local.tunnel_ip == remote.tunnel_ip {
                if local.id < remote.id {
                    // Local wins, remote should re-assign (handled by remote on next exchange)
                    tracing::info!(
                        "Tunnel IP collision: {} (local) keeps {}, {} should re-assign",
                        local.name, local.tunnel_ip, remote.name
                    );
                } else {
                    // Remote wins, local re-assigns
                    let new_ip = match assign_tunnel_ip(node_repo, "").await {
                        // If we don't have a range, just clear it
                        Err(_) => String::new(),
                        Ok(ip) => ip,
                    };
                    if !new_ip.is_empty() {
                        let _ = node_repo.update_cluster_fields(
                            &local.id,
                            &local.wg_pubkey,
                            &new_ip,
                        ).await;
                        tracing::warn!(
                            "Re-assigned {} tunnel IP from {} to {} (collision with {})",
                            local.name, local.tunnel_ip, new_ip, remote.name
                        );
                    }
                }
            }
        }
    }
}

/// Rebuild wg-cluster config and apply it.
/// Returns the private key needed by the caller.
pub async fn sync_cluster_wg(
    node_repo: &NodeRepository,
    my_wg_private_key: &str,
) -> Result<(), AppError> {
    let nodes = node_repo.list_all().await?;

    let config = services::wireguard::generate_cluster_wg_config(
        &nodes,
        my_wg_private_key,
        CLUSTER_WG_PORT,
    );

    // Atomic write
    let config_path = format!("/etc/wireguard/{CLUSTER_WG_INTERFACE}.conf");
    let tmp_path = format!("{config_path}.tmp");

    std::fs::write(&tmp_path, config)
        .map_err(|e| AppError::Internal(format!("Cannot write wg-cluster config: {e}")))?;
    std::fs::rename(&tmp_path, &config_path)
        .map_err(|e| AppError::Internal(format!("Cannot rename wg-cluster config: {e}")))
}

/// Rebuild bird.conf (full config with iBGP) and apply it.
pub async fn sync_cluster_bird(
    peer_repo: &crate::models::peer::PeerRepository,
    settings: &crate::models::settings::Settings,
    node_repo: &NodeRepository,
    my_tunnel_ip: &str,
) -> Result<(), AppError> {
    let peers = peer_repo.list_all().await?;
    let nodes = node_repo.list_all().await?;

    let mut config = services::bird::generate_full_config(&peers, settings, "");

    // Append iBGP blocks
    let ibgp = services::bird::generate_ibgp_blocks(&nodes, settings, my_tunnel_ip);
    config.push_str(&ibgp);

    services::bird::apply_config(&config)
}
```

- [ ] **Step 2: Add tunnel module to cluster/mod.rs**

```rust
pub mod aggregator;
pub mod auth;
pub mod cache;
pub mod tunnel;
```

- [ ] **Step 3: Add error variant**

In `src/error.rs`, add the `Internal` variant if not already present — check file first. The tunnel module uses `AppError::Internal(String)`. Verify the variant exists and is used consistently.

- [ ] **Step 4: Verify compile**

Run: `cargo check 2>&1 | head -30`

- [ ] **Step 5: Commit**

```bash
git add src/cluster/tunnel.rs src/cluster/mod.rs
git commit -m "feat(cluster): add tunnel module — keypair init, IP assignment, WG/BIRD sync

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 8: Peer Service — Hook auto-apply after CRUD

**Files:**
- Modify: `src/grpc/peer_service.rs`

- [ ] **Step 1: Add auto-apply helper function**

After the `proxy_push_peer` method in `impl PeerServiceImpl`, add:

```rust
async fn auto_apply_wg_bird(
    &self,
) -> Result<(), Status> {
    let peers = self.peer_repo.list_all()
        .await
        .map_err(|e| Status::internal(e.to_string()))?;
    let settings = self.settings_repo.load()
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    // 1. WireGuard: full regenerate wg0.conf + apply
    let wg_config: String = peers
        .iter()
        .filter(|p| p.enabled)
        .map(|p| crate::services::wireguard::generate_config(p, &settings))
        .collect::<Vec<_>>()
        .join("\n");

    if !wg_config.is_empty() {
        let conf_path = "/etc/wireguard/wg0.conf";
        let tmp_path = "/etc/wireguard/wg0.conf.tmp";
        std::fs::write(tmp_path, &wg_config)
            .map_err(|e| Status::internal(format!("Cannot write wg0.conf: {e}")))?;
        std::fs::rename(tmp_path, conf_path)
            .map_err(|e| Status::internal(format!("Cannot rename wg0.conf: {e}")))?;
        crate::services::wireguard::apply_syncconf("wg0", conf_path)
            .map_err(|e| Status::internal(e.to_string()))?;
    }

    // 2. BIRD: full regenerate bird.conf + apply
    let mut bird_config = crate::services::bird::generate_full_config(&peers, &settings, "");

    // Append cluster iBGP blocks if cluster mode
    if !self.node_repo.list_all().await.map_err(|e| Status::internal(e.to_string()))?.is_empty() {
        let nodes = self.node_repo.list_all().await.map_err(|e| Status::internal(e.to_string()))?;
        let my_tunnel_ip = nodes.iter()
            .find(|n| n.listen_addr == self.listen_addr)
            .map(|n| n.tunnel_ip.clone())
            .unwrap_or_default();
        bird_config.push_str(&crate::services::bird::generate_ibgp_blocks(&nodes, &settings, &my_tunnel_ip));
    }

    crate::services::bird::apply_config(&bird_config)
        .map_err(|e| Status::internal(e.to_string()))?;

    Ok(())
}
```

- [ ] **Step 2: Call auto_apply after create_peer**

In `create_peer`, after the successful update, before `Ok(Response::new(...))`:
```rust
self.auto_apply_wg_bird().await?;
```
Only when the peer is created locally (not proxied — skip for proxy path).

- [ ] **Step 3: Call auto_apply after update_peer**

Same pattern in `update_peer`, after local update, before return.

- [ ] **Step 4: Call auto_apply after delete_peer**

```rust
self.peer_repo.delete(&req.id).await.map_err(|e| Status::not_found(e.to_string()))?;
self.auto_apply_wg_bird().await?;
```

- [ ] **Step 5: Call auto_apply after toggle_peer**

After `toggle_enabled`, before return:
```rust
self.auto_apply_wg_bird().await?;
```

- [ ] **Step 6: Verify compile and tests**

Run: `cargo check 2>&1 | head -20`
Run: `cargo test 2>&1 | tail -5`

- [ ] **Step 7: Commit**

```bash
git add src/grpc/peer_service.rs
git commit -m "feat(peer): auto-apply WG + BIRD config after create/update/delete/toggle

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 9: Cluster Service — ExchangeNodes include wg_pubkey + tunnel_ip

**Files:**
- Modify: `src/grpc/cluster_service.rs`

- [ ] **Step 1: Update ExchangeNodes to include new fields in returned NodeInfo**

In `exchange_nodes`, update the NodeInfo construction:

```rust
let node_infos: Vec<NodeInfo> = nodes
    .iter()
    .map(|n| NodeInfo {
        name: n.name.clone(),
        listen_addr: n.listen_addr.clone(),
        local_asn: n.local_asn,
        description: n.description.clone().unwrap_or_default(),
        last_seen_at: n.last_seen_at.clone(),
        wg_public_key: n.wg_pubkey.clone(),  // new
        tunnel_ip: n.tunnel_ip.clone(),        // new
    })
    .collect();
```

- [ ] **Step 2: Update ExchangeNodes to also save new fields on received nodes**

In `exchange_nodes`, after `upsert_by_name`, also update wg_pubkey and tunnel_ip. Replace the existing loop body:

```rust
for ni in &req.nodes {
    let node = match self
        .node_repo
        .upsert_by_name(&ni.name, &ni.listen_addr, ni.local_asn, &ni.description)
        .await
    {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!("Failed to upsert node {} from exchange: {}", ni.name, e);
            continue;
        }
    };
    // Update cluster fields if they changed
    if !ni.wg_public_key.is_empty() && ni.wg_public_key != node.wg_pubkey {
        let _ = self.node_repo.update_cluster_fields(
            &node.id, &ni.wg_public_key, &ni.tunnel_ip,
        ).await;
    }
    if !ni.tunnel_ip.is_empty() && ni.tunnel_ip != node.tunnel_ip {
        let _ = self.node_repo.update_cluster_fields(
            &node.id, &node.wg_pubkey, &ni.tunnel_ip,
        ).await;
    }
}
```

- [ ] **Step 3: Verify compile**

Run: `cargo check 2>&1 | head -30`

- [ ] **Step 4: Commit**

```bash
git add src/grpc/cluster_service.rs
git commit -m "feat(cluster): ExchangeNodes includes wg_pubkey+tunnel_ip

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 10: ManagementService gRPC Implementation

**Files:**
- Create: `src/grpc/management_service.rs`
- Modify: `src/grpc/mod.rs`

- [ ] **Step 1: Create ManagementServiceImpl**

Write `src/grpc/management_service.rs`:

```rust
use tonic::{Request, Response, Status};

use super::generated::{
    management_service_server::ManagementService,
    GetWGStatusRequest, GetWGStatusResponse,
    GetBirdStatusRequest, GetBirdStatusResponse,
};

pub struct ManagementServiceImpl;

#[tonic::async_trait]
impl ManagementService for ManagementServiceImpl {
    async fn get_wire_guard_status(
        &self,
        request: Request<GetWGStatusRequest>,
    ) -> Result<Response<GetWGStatusResponse>, Status> {
        let req = request.into_inner();
        let iface = if req.interface.is_empty() {
            "all"
        } else {
            &req.interface
        };

        let interfaces = crate::services::wireguard::get_wg_status(iface)
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(GetWGStatusResponse { interfaces }))
    }

    async fn get_bird_status(
        &self,
        _request: Request<GetBirdStatusRequest>,
    ) -> Result<Response<GetBirdStatusResponse>, Status> {
        let protocols = crate::services::bird::get_bird_status()
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(GetBirdStatusResponse { protocols }))
    }
}
```

- [ ] **Step 2: Register in grpc/mod.rs**

```rust
pub mod bird_service;
pub mod cluster_service;
pub mod flap_service;
pub mod generated;
pub mod management_service;
pub mod peer_service;
pub mod settings_service;
```

- [ ] **Step 3: Register in main.rs**

Add import:
```rust
use crate::grpc::generated::management_service_server::ManagementServiceServer;
use crate::grpc::management_service::ManagementServiceImpl;
```

Add service instantiation and registration (after flap_svc):
```rust
let mgmt_svc = ManagementServiceImpl;

// In the tonic router builder:
.add_service(ManagementServiceServer::new(mgmt_svc))
```

- [ ] **Step 4: Verify compile**

Run: `cargo check 2>&1 | head -20`

- [ ] **Step 5: Commit**

```bash
git add src/grpc/management_service.rs src/grpc/mod.rs src/main.rs
git commit -m "feat(grpc): add ManagementService with WG/BIRD status RPCs

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 11: main.rs — Init cluster tunnels on startup

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add cluster tunnel init after self-registration**

In `main.rs`, inside the `if !node_name.is_empty()` block, after the self-registration section and after `// Seed bootstrap peers...` block, add:

```rust
// Init cluster WG tunnel
let wg_private_key = if !cfg.cluster.tunnel_ip_range.is_empty() {
    match crate::cluster::tunnel::init_local_node(
        &state.node_repo,
        &node.id,
        &cfg.cluster.tunnel_ip_range,
    ).await {
        Ok((priv_key, pub_key, tunnel_ip)) => {
            tracing::info!(
                "Cluster tunnel initialized: key={}, ip={}",
                pub_key,
                tunnel_ip
            );

            // Apply initial wg-cluster config
            if let Err(e) = crate::cluster::tunnel::sync_cluster_wg(
                &state.node_repo,
                &priv_key,
            ).await {
                tracing::warn!("Failed to apply initial wg-cluster config: {e}");
            }

            // Apply initial bird config with iBGP
            let settings = state.settings_repo.load().await?;
            if let Err(e) = crate::cluster::tunnel::sync_cluster_bird(
                &state.peer_repo,
                &settings,
                &state.node_repo,
                &tunnel_ip,
            ).await {
                tracing::warn!("Failed to apply initial cluster bird config: {e}");
            }

            priv_key
        }
        Err(e) => {
            tracing::warn!("Cluster tunnel init failed: {e}");
            String::new()
        }
    }
} else {
    tracing::debug!("No tunnel_ip_range configured, skipping cluster WG tunnels");
    String::new()
};
```

- [ ] **Step 2: Also sync cluster WG on ExchangeNodes receipt**

In the periodic anti-entropy task (`sync_ct` spawn), after upserting new nodes, add:

```rust
// After the remote node upserts, sync cluster configs
if !cfg.cluster.tunnel_ip_range.is_empty() {
    let my_tunnel_ip = nodes.iter()
        .find(|n| n.listen_addr == listen_addr_sync)
        .and_then(|n| if n.tunnel_ip.is_empty() { None } else { Some(n.tunnel_ip.clone()) })
        .unwrap_or_default();
    if !my_tunnel_ip.is_empty() {
        if let Ok(settings) = state.settings_repo.load().await {
            let _ = crate::cluster::tunnel::sync_cluster_bird(
                &state.peer_repo, &settings, &state.node_repo, &my_tunnel_ip
            ).await;
        }
    }
}
```

**Note:** This requires making `cfg.cluster.tunnel_ip_range` available in the spawned tasks. We need to clone it before `cfg` moves into `cfg_arc`. Already done in the existing clones at top of main.

- [ ] **Step 3: Verify compile**

Run: `cargo check 2>&1 | head -30`

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat(main): init cluster tunnels on startup + sync on ExchangeNodes

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 12: Frontend — Status page + hooks + NavBar

**Files:**
- Create: `frontend/src/components/status/StatusPage.tsx`
- Create: `frontend/src/hooks/useManagement.ts`
- Modify: `frontend/src/lib/grpc.ts`
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/components/layout/NavBar.tsx`

- [ ] **Step 1: Register ManagementService client in grpc.ts**

Add after `flapClient`:
```typescript
import { ManagementService } from './peerman_pb';

export const mgmtClient = createClient(ManagementService, transport);
```

- [ ] **Step 2: Create useManagement hook**

Write `frontend/src/hooks/useManagement.ts`:
```typescript
import { useState, useEffect, useCallback } from 'react';
import { create } from '@bufbuild/protobuf';
import { GetWGStatusRequestSchema, GetBirdStatusRequestSchema } from '../lib/peerman_pb';
import type { WGInterface, BirdProtocol } from '../lib/peerman_pb';
import { mgmtClient } from '../lib/grpc';

export function useWireGuardStatus(iface: string = '') {
  const [interfaces, setInterfaces] = useState<WGInterface[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetch = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await mgmtClient.getWireGuardStatus(
        create(GetWGStatusRequestSchema, { interface: iface })
      );
      setInterfaces(res.interfaces);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [iface]);

  useEffect(() => { fetch(); }, [fetch]);

  return { interfaces, loading, error, refetch: fetch };
}

export function useBirdStatus() {
  const [protocols, setProtocols] = useState<BirdProtocol[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetch = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await mgmtClient.getBirdStatus(
        create(GetBirdStatusRequestSchema, {})
      );
      setProtocols(res.protocols);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { fetch(); }, [fetch]);

  return { protocols, loading, error, refetch: fetch };
}
```

- [ ] **Step 3: Create StatusPage component**

Write `frontend/src/components/status/StatusPage.tsx`:
```tsx
import { RefreshCw } from 'lucide-react';
import { useWireGuardStatus, useBirdStatus } from '../../hooks/useManagement';

export default function StatusPage() {
  const wg = useWireGuardStatus();
  const bird = useBirdStatus();

  return (
    <div className="space-y-lg animate-fade-in">
      <div className="flex items-center justify-between">
        <h1 className="text-display-md text-ink">System Status</h1>
        <button
          onClick={() => { wg.refetch(); bird.refetch(); }}
          className="btn-ghost text-xs flex items-center gap-1"
        >
          <RefreshCw className="w-3 h-3" />
          Refresh
        </button>
      </div>

      {/* WireGuard */}
      <div className="card">
        <h2 className="text-body-md-strong text-ink mb-md">WireGuard</h2>
        {wg.loading && <div className="text-body-sm text-body">Loading...</div>}
        {wg.error && <div className="text-body-sm text-error">{wg.error}</div>}
        {!wg.loading && !wg.error && wg.interfaces.length === 0 && (
          <div className="text-body-sm text-body">No WireGuard interfaces found</div>
        )}
        {wg.interfaces.map((iface) => (
          <div key={iface.name} className="space-y-sm">
            <div className="text-body-sm text-body">
              {iface.name} — pubkey: <code className="code-block text-xs">{iface.publicKey.substring(0, 12)}...</code>, port: {iface.listenPort}
            </div>
            {iface.peers.length === 0 && (
              <div className="text-caption text-mute ml-md">No peers</div>
            )}
            {iface.peers.map((peer, i) => (
              <div key={i} className="card-soft text-caption">
                <div className="grid grid-cols-2 md:grid-cols-3 gap-xxs">
                  <div><span className="text-mute">Peer:</span> <code>{peer.publicKey.substring(0, 10)}...</code></div>
                  <div><span className="text-mute">Endpoint:</span> {peer.endpoint || '—'}</div>
                  <div><span className="text-mute">Handshake:</span> {peer.latestHandshake || '—'}</div>
                  <div><span className="text-mute">RX:</span> {peer.transferRx || '—'}</div>
                  <div><span className="text-mute">TX:</span> {peer.transferTx || '—'}</div>
                </div>
              </div>
            ))}
          </div>
        ))}
      </div>

      {/* BIRD */}
      <div className="card">
        <h2 className="text-body-md-strong text-ink mb-md">BIRD</h2>
        {bird.loading && <div className="text-body-sm text-body">Loading...</div>}
        {bird.error && <div className="text-body-sm text-error">{bird.error}</div>}
        {!bird.loading && !bird.error && bird.protocols.length === 0 && (
          <div className="text-body-sm text-body">No BIRD protocols found</div>
        )}
        {!bird.loading && !bird.error && bird.protocols.length > 0 && (
          <div className="data-table">
            <table>
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Proto</th>
                  <th>Table</th>
                  <th>State</th>
                  <th>Since</th>
                  <th>Info</th>
                </tr>
              </thead>
              <tbody>
                {bird.protocols.map((p) => (
                  <tr key={p.name}>
                    <td className="font-mono text-caption-mono">{p.name}</td>
                    <td>{p.proto}</td>
                    <td>{p.table}</td>
                    <td>
                      <span className={`badge ${p.state === 'up' ? 'bg-green-500/20 text-green-500' : 'bg-red-500/20 text-red-500'}`}>
                        {p.state}
                      </span>
                    </td>
                    <td className="text-mute">{p.since}</td>
                    <td className="text-mute text-xs">{p.info}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Add route in App.tsx**

Add import:
```typescript
import StatusPage from './components/status/StatusPage';
```

Add route (before `</Routes>`):
```tsx
<Route path="/status" element={<StatusPage />} />
```

- [ ] **Step 5: Add NavBar link**

In NavBar.tsx, add to the `links` array:
```typescript
{ to: '/status', label: 'Status', icon: Activity },
```

- [ ] **Step 6: Type-check frontend**

Run: `cd frontend && pnpm exec tsc --noEmit 2>&1 | head -20`

- [ ] **Step 7: Commit**

```bash
git add frontend/src/components/status/StatusPage.tsx \
        frontend/src/hooks/useManagement.ts \
        frontend/src/lib/grpc.ts \
        frontend/src/App.tsx \
        frontend/src/components/layout/NavBar.tsx
git commit -m "feat(frontend): add Status page with WG interface + BIRD protocol views

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 13: Integration — Wire up ExchangeNodes sync with cluster WG/BIRD

**Files:**
- Modify: `src/grpc/cluster_service.rs`
- Modify: `src/main.rs` (bootstrap exchange section)

- [ ] **Step 1: Add settings_repo to ClusterServiceImpl**

The `ClusterServiceImpl` already has access to `peer_repo` and `node_repo`. It also needs `settings_repo`. Add it:

In `src/grpc/cluster_service.rs`:
```rust
pub struct ClusterServiceImpl {
    pub node_repo: NodeRepository,
    pub peer_repo: PeerRepository,
    pub probe_repo: ProbeResultRepository,
    pub community_repo: CommunityRuleRepository,
    pub settings_repo: crate::models::settings::SettingsRepository,  // new
    pub jwt_secret: std::sync::Arc<String>,
    pub cluster_key: std::sync::Arc<String>,
    pub listen_addr: String,
}
```

Update `main.rs` where `cluster_svc` is constructed:
```rust
let cluster_svc = ClusterServiceImpl {
    node_repo: state.node_repo.clone(),
    peer_repo: state.peer_repo.clone(),
    probe_repo: state.probe_repo.clone(),
    community_repo: state.community_repo.clone(),
    settings_repo: state.settings_repo.clone(),  // new
    jwt_secret: jwt_secret.clone(),
    cluster_key: Arc::new(cluster_key.clone()),
    listen_addr: listen_addr.clone(),
};
```

- [ ] **Step 2: Complete the ExchangeNodes sync to actually call tunnel sync**

Replace the placeholder `todo!()` calls from Task 9 with real code (in `exchange_nodes` handler, after saving nodes):

```rust
// After the node upsert loop, sync cluster configs
if !self.cluster_key.is_empty() {
    let nodes = self.node_repo.list_all().await.unwrap_or_default();
    let my_tunnel_ip = nodes.iter()
        .find(|n| n.listen_addr == self.listen_addr)
        .and_then(|n| if n.tunnel_ip.is_empty() { None } else { Some(n.tunnel_ip.clone()) })
        .unwrap_or_default();

    if !my_tunnel_ip.is_empty() {
        if let Err(e) = crate::cluster::tunnel::sync_cluster_wg(
            &self.node_repo, "",
        ).await {
            tracing::warn!("Failed to sync cluster WG after exchange: {e}");
        }
        if let Ok(settings) = self.settings_repo.load().await {
            if let Err(e) = crate::cluster::tunnel::sync_cluster_bird(
                &self.peer_repo, &settings, &self.node_repo, &my_tunnel_ip,
            ).await {
                tracing::warn!("Failed to sync cluster BIRD after exchange: {e}");
            }
        }
    }
}
```

- [ ] **Step 3: Same for bootstrap exchange in main.rs**

Update the bootstrap exchange loop in `main.rs` (the `for addr in &peer_nodes` block) to also trigger tunnel sync after receiving remote nodes.

- [ ] **Step 4: Verify compile**

Run: `cargo check 2>&1 | head -30`

- [ ] **Step 5: Run all tests**

Run: `cargo test 2>&1 | tail -10`
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/grpc/cluster_service.rs src/main.rs
git commit -m "feat(cluster): wire ExchangeNodes to trigger WG/BIRD cluster sync

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 14: Final Verification

- [ ] **Step 1: Full build check**

Run: `cargo check 2>&1`
Expected: no errors

- [ ] **Step 2: Run all tests**

Run: `cargo test 2>&1`
Expected: all 32+ tests pass

- [ ] **Step 3: Run clippy**

Run: `cargo clippy 2>&1`
Expected: no new warnings

- [ ] **Step 4: Frontend type-check**

Run: `cd frontend && pnpm exec tsc --noEmit 2>&1`
Expected: no errors

- [ ] **Step 5: Commit any remaining fixes**
