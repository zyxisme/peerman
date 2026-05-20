# Network Configuration Optimization — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply Lantian Blog + DN42 Wiki best practices: ROA filtering, prefix limits, WireGuard advanced config, Community MED 3-dimension enhancement.

**Architecture:** Expand Settings (DB → model → proto → gRPC → frontend) and CommunityRule in parallel, then rewrite config generation (`wireguard.rs`, `bird.rs`) to use new fields, finally update frontend forms. All layers follow existing codebase patterns.

**Tech Stack:** Rust (tonic + axum + sqlx), TypeScript (React + Vite + Tailwind + @connectrpc/connect), protobuf (protoc-gen-es), SQLite WAL.

---

## File Map

| File | Action | Role |
|------|--------|------|
| `migrations/005_network_optimization.sql` | Create | New DB columns for settings + community_rules |
| `proto/peerman.proto` | Modify | New Settings + CommunityRule fields (regenerate) |
| `src/models/settings.rs` | Modify | New fields on Settings struct + repository queries |
| `src/models/community.rs` | Modify | New fields on CommunityRule struct + repository + seed |
| `src/services/wireguard.rs` | Modify | Rewrite `generate_config()` |
| `src/services/bird.rs` | Modify | Rewrite `generate_full_config()` + ROA helpers |
| `src/services/community_mapper.rs` | Modify | 3-dimension matching + MED function |
| `src/grpc/settings_service.rs` | Modify | Wire new Settings fields |
| `src/grpc/cluster_service.rs` | Modify | Wire new CommunityRule fields |
| `frontend/src/lib/peerman_pb.ts` | Regenerate | `protoc` from modified proto |
| `frontend/src/components/settings/SettingsForm.tsx` | Modify | Add WG advanced, ROA, BIRD filter sections |
| `frontend/src/components/communities/CommunityRules.tsx` | Modify | Add bandwidth/crypto/MED fields |

---

### Task 1: Database Migration

**Files:**
- Create: `migrations/005_network_optimization.sql`

- [ ] **Step 1: Write migration SQL**

```sql
-- Add WireGuard advanced settings columns
ALTER TABLE settings ADD COLUMN wg_mtu INTEGER NOT NULL DEFAULT 1420;
ALTER TABLE settings ADD COLUMN wg_fwmark INTEGER NOT NULL DEFAULT 0;
ALTER TABLE settings ADD COLUMN wg_post_up TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN wg_post_down TEXT NOT NULL DEFAULT '';

-- Add ROA/RPKI settings columns
ALTER TABLE settings ADD COLUMN roa_mode TEXT NOT NULL DEFAULT 'none';
ALTER TABLE settings ADD COLUMN roa_static_v4_url TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN roa_static_v6_url TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN roa_rtr_address TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN roa_rtr_port INTEGER NOT NULL DEFAULT 323;

-- Add BIRD filter settings columns
ALTER TABLE settings ADD COLUMN bird_import_limit INTEGER NOT NULL DEFAULT 9000;
ALTER TABLE settings ADD COLUMN bird_export_filter TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN bird_import_filter TEXT NOT NULL DEFAULT '';

-- Add community rule multi-dimension columns
ALTER TABLE community_rules ADD COLUMN min_bandwidth_mbps REAL NOT NULL DEFAULT 0;
ALTER TABLE community_rules ADD COLUMN crypto_weight INTEGER NOT NULL DEFAULT 0;
ALTER TABLE community_rules ADD COLUMN med_penalty INTEGER NOT NULL DEFAULT 0;
```

- [ ] **Step 2: Verify migration**

Run: `cargo test 2>&1 | tail -5`
Expected: All 23 tests pass (sqlx picks up new migration automatically).

- [ ] **Step 3: Commit**

```bash
git add migrations/005_network_optimization.sql
git commit -m "feat: add migration 005 — WG advanced, ROA, BIRD filter, community MED columns"
```

---

### Task 2: Proto Changes

**Files:**
- Modify: `proto/peerman.proto`

- [ ] **Step 1: Add new Settings fields to proto**

Edit `proto/peerman.proto`, replace the `message Settings` block:

```protobuf
message Settings {
  int64 local_asn = 1;
  string bird_template_name = 2;
  string bird_router_id = 3;
  uint32 wg_default_listen_port = 4;
  string dn42_ipv4_prefix = 5;
  string dn42_ipv6_prefix = 6;
  string wg_table = 7;
  // WireGuard advanced
  uint32 wg_mtu = 8;
  uint32 wg_fwmark = 9;
  string wg_post_up = 10;
  string wg_post_down = 11;
  // ROA/RPKI
  string roa_mode = 12;
  string roa_static_v4_url = 13;
  string roa_static_v6_url = 14;
  string roa_rtr_address = 15;
  uint32 roa_rtr_port = 16;
  // BIRD filter
  uint32 bird_import_limit = 17;
  string bird_export_filter = 18;
  string bird_import_filter = 19;
}
```

- [ ] **Step 2: Add new CommunityRule fields to proto**

Edit `proto/peerman.proto`, replace the `message CommunityRule` block:

```protobuf
message CommunityRule {
  string id = 1;
  string description = 2;
  double max_latency_ms = 3;
  double max_packet_loss_pct = 4;
  string community_ipv4 = 5;
  string community_ipv6 = 6;
  bool enabled = 7;
  double min_bandwidth_mbps = 8;
  int32 crypto_weight = 9;
  int32 med_penalty = 10;
}
```

- [ ] **Step 3: Build (regenerates Rust stubs via build.rs)**

Run: `cargo build 2>&1 | tail -10`
Expected: Compiles. `build.rs` runs `tonic_build::compile_protos` which picks up the new fields. Note: the build will fail if any Rust code references old struct sizes. That's fine — we fix in next tasks.

- [ ] **Step 4: Regenerate frontend TS stubs**

Run: `cd frontend && PATH="node_modules/.bin:$PATH" protoc -I ../proto --es_out src/lib --es_opt target=ts ../proto/peerman.proto`
Expected: `src/lib/peerman_pb.ts` updated with new Settings + CommunityRule fields. No output on success.

- [ ] **Step 5: Commit**

```bash
git add proto/peerman.proto frontend/src/lib/peerman_pb.ts
git commit -m "feat: add Settings WG/ROA/BIRD fields and CommunityRule MED fields to proto"
```

---

### Task 3: Rust Settings Model + Repository

**Files:**
- Modify: `src/models/settings.rs`

- [ ] **Step 1: Add new fields to Settings struct**

Edit `src/models/settings.rs`, update the struct:

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Settings {
    pub local_asn: i64,
    pub bird_template_name: String,
    pub bird_router_id: String,
    pub wg_default_listen_port: i64,
    pub dn42_ipv4_prefix: String,
    pub dn42_ipv6_prefix: String,
    pub wg_table: String,
    pub wg_mtu: i64,
    pub wg_fwmark: i64,
    pub wg_post_up: String,
    pub wg_post_down: String,
    pub roa_mode: String,
    pub roa_static_v4_url: String,
    pub roa_static_v6_url: String,
    pub roa_rtr_address: String,
    pub roa_rtr_port: i64,
    pub bird_import_limit: i64,
    pub bird_export_filter: String,
    pub bird_import_filter: String,
}
```

- [ ] **Step 2: Update load() query with all columns**

Replace the `sqlx::query_as` in `load()`:

```rust
pub async fn load(&self) -> Result<Settings, AppError> {
    let row = sqlx::query_as::<_, Settings>(
        "SELECT local_asn, bird_template_name, bird_router_id,
         wg_default_listen_port, dn42_ipv4_prefix, dn42_ipv6_prefix, wg_table,
         wg_mtu, wg_fwmark, wg_post_up, wg_post_down,
         roa_mode, roa_static_v4_url, roa_static_v6_url, roa_rtr_address, roa_rtr_port,
         bird_import_limit, bird_export_filter, bird_import_filter
         FROM settings WHERE id = 1",
    )
    .fetch_one(&self.pool)
    .await?;

    Ok(row)
}
```

- [ ] **Step 3: Update save() query with all columns**

Replace the `sqlx::query` in `save()`:

```rust
pub async fn save(&self, settings: &Settings) -> Result<Settings, AppError> {
    sqlx::query(
        "UPDATE settings SET
         local_asn = ?, bird_template_name = ?, bird_router_id = ?,
         wg_default_listen_port = ?, dn42_ipv4_prefix = ?, dn42_ipv6_prefix = ?,
         wg_table = ?,
         wg_mtu = ?, wg_fwmark = ?, wg_post_up = ?, wg_post_down = ?,
         roa_mode = ?, roa_static_v4_url = ?, roa_static_v6_url = ?,
         roa_rtr_address = ?, roa_rtr_port = ?,
         bird_import_limit = ?, bird_export_filter = ?, bird_import_filter = ?
         WHERE id = 1",
    )
    .bind(settings.local_asn)
    .bind(&settings.bird_template_name)
    .bind(&settings.bird_router_id)
    .bind(settings.wg_default_listen_port)
    .bind(&settings.dn42_ipv4_prefix)
    .bind(&settings.dn42_ipv6_prefix)
    .bind(&settings.wg_table)
    .bind(settings.wg_mtu)
    .bind(settings.wg_fwmark)
    .bind(&settings.wg_post_up)
    .bind(&settings.wg_post_down)
    .bind(&settings.roa_mode)
    .bind(&settings.roa_static_v4_url)
    .bind(&settings.roa_static_v6_url)
    .bind(&settings.roa_rtr_address)
    .bind(settings.roa_rtr_port)
    .bind(settings.bird_import_limit)
    .bind(&settings.bird_export_filter)
    .bind(&settings.bird_import_filter)
    .execute(&self.pool)
    .await?;

    self.load().await
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check 2>&1 | tail -5`
Expected: Compiles. Tests in wireguard.rs may have Settings construction that needs updating — we fix in Task 5.

- [ ] **Step 5: Commit**

```bash
git add src/models/settings.rs
git commit -m "feat: add new Settings fields — WG advanced, ROA, BIRD filter"
```

---

### Task 4: Rust CommunityRule Model + Repository + Seed

**Files:**
- Modify: `src/models/community.rs`

- [ ] **Step 1: Add new fields to CommunityRule struct**

Edit `src/models/community.rs`, update the struct:

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommunityRule {
    pub id: String,
    pub description: Option<String>,
    pub max_latency_ms: f64,
    pub max_packet_loss_pct: f64,
    pub community_ipv4: String,
    pub community_ipv6: String,
    pub enabled: bool,
    pub min_bandwidth_mbps: f64,
    pub crypto_weight: i32,
    pub med_penalty: i32,
    pub created_at: String,
    pub updated_at: String,
}
```

- [ ] **Step 2: Update list_all() query**

Replace the SELECT in `list_all()`:

```rust
"SELECT id, description, max_latency_ms, max_packet_loss_pct,
 community_ipv4, community_ipv6, enabled,
 min_bandwidth_mbps, crypto_weight, med_penalty,
 created_at, updated_at
 FROM community_rules ORDER BY max_latency_ms ASC"
```

- [ ] **Step 3: Update list_enabled() query**

```rust
"SELECT id, description, max_latency_ms, max_packet_loss_pct,
 community_ipv4, community_ipv6, enabled,
 min_bandwidth_mbps, crypto_weight, med_penalty,
 created_at, updated_at
 FROM community_rules WHERE enabled = 1
 ORDER BY max_latency_ms ASC"
```

- [ ] **Step 4: Update save() SQL**

```rust
"INSERT INTO community_rules
 (id, description, max_latency_ms, max_packet_loss_pct,
  community_ipv4, community_ipv6, enabled,
  min_bandwidth_mbps, crypto_weight, med_penalty,
  created_at, updated_at)
 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
 ON CONFLICT(id) DO UPDATE SET
 description = excluded.description,
 max_latency_ms = excluded.max_latency_ms,
 max_packet_loss_pct = excluded.max_packet_loss_pct,
 community_ipv4 = excluded.community_ipv4,
 community_ipv6 = excluded.community_ipv6,
 enabled = excluded.enabled,
 min_bandwidth_mbps = excluded.min_bandwidth_mbps,
 crypto_weight = excluded.crypto_weight,
 med_penalty = excluded.med_penalty,
 updated_at = excluded.updated_at
 RETURNING id, description, max_latency_ms, max_packet_loss_pct,
 community_ipv4, community_ipv6, enabled,
 min_bandwidth_mbps, crypto_weight, med_penalty,
 created_at, updated_at"
```

Add `.bind(rule.min_bandwidth_mbps)`, `.bind(rule.crypto_weight)`, `.bind(rule.med_penalty)` after the `enabled` bind, before the timestamp binds (adjust existing `.bind(&now)` → those stay at the end, but make sure order matches the 12 placeholders).

Full bind order:
```rust
.bind(&rule.id)
.bind(&rule.description)
.bind(rule.max_latency_ms)
.bind(rule.max_packet_loss_pct)
.bind(&rule.community_ipv4)
.bind(&rule.community_ipv6)
.bind(rule.enabled)
.bind(rule.min_bandwidth_mbps)
.bind(rule.crypto_weight)
.bind(rule.med_penalty)
.bind(&now)
.bind(&now)
```

- [ ] **Step 5: Update seed_defaults() with 3D values**

Update seed tuples in `seed_defaults()`:

```rust
let defaults = vec![
    ("Metro (<5ms)", 5.0, 1.0, "<asn>,10", "<asn>,610", 1000.0, 1, 0),
    ("Regional (5-20ms)", 20.0, 1.0, "<asn>,20", "<asn>,620", 500.0, 1, 100),
    ("Continental (20-50ms)", 50.0, 2.0, "<asn>,30", "<asn>,630", 200.0, 2, 200),
    ("Intercontinental (50-150ms)", 150.0, 5.0, "<asn>,40", "<asn>,640", 50.0, 3, 400),
    ("High latency (>150ms)", 100_000.0, 100.0, "<asn>,50", "<asn>,650", 0.0, 0, 800),
];
```

Update the destructure and CommunityRule construction in the loop:
```rust
for (desc, max_lat, max_loss, c4, c6, min_bw, crypto_w, med_p) in defaults {
    let rule = CommunityRule {
        id: Uuid::new_v4().to_string(),
        description: Some(desc.to_string()),
        max_latency_ms: max_lat,
        max_packet_loss_pct: max_loss,
        community_ipv4: c4.to_string(),
        community_ipv6: c6.to_string(),
        enabled: true,
        min_bandwidth_mbps: min_bw,
        crypto_weight: crypto_w,
        med_penalty: med_p,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    };
    self.save(&rule).await?;
}
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check 2>&1 | tail -10`
Expected: Compiles. Any unmatched patterns in `cluster_service.rs` (which constructs CommunityRule) will show errors — we fix in Task 9.

- [ ] **Step 7: Commit**

```bash
git add src/models/community.rs
git commit -m "feat: add CommunityRule MED fields — bandwidth, crypto, penalty"
```

---

### Task 5: WireGuard Config Generation Rewrite

**Files:**
- Modify: `src/services/wireguard.rs`

- [ ] **Step 1: Rewrite generate_config()**

Replace the entire `generate_config()` function:

```rust
/// Generate a complete WireGuard configuration for a single peer.
/// Returns the INI-format config string.
pub fn generate_config(peer: &Peer, settings: &crate::models::settings::Settings) -> String {
    let mut config = String::new();

    // [Interface] section
    config.push_str("[Interface]\n");
    if let Some(ref key) = peer.wg_private_key {
        config.push_str(&format!("PrivateKey = {key}\n"));
    }
    config.push_str(&format!("ListenPort = {}\n", peer.wg_listen_port));
    config.push_str(&format!("Table = {}\n", settings.wg_table));

    // MTU (only if > 0)
    if settings.wg_mtu > 0 {
        config.push_str(&format!("MTU = {}\n", settings.wg_mtu));
    }

    // FwMark (only if > 0)
    if settings.wg_fwmark > 0 {
        config.push_str(&format!("FwMark = {}\n", settings.wg_fwmark));
    }

    // Build Address line from tunnel IPs
    let mut addresses = Vec::new();
    if let Some(ref ipv4) = peer.ipv4_tunnel_local {
        addresses.push(format!("{ipv4}/32"));
    }
    if let Some(ref ipv6) = peer.ipv6_tunnel_local {
        addresses.push(format!("{ipv6}/128"));
    }
    if !addresses.is_empty() {
        config.push_str(&format!("Address = {}\n", addresses.join(", ")));
    }

    // PostUp — auto-generate + user custom
    let mut post_up = format!(
        "PostUp = ip link set %i up; sysctl -w net.ipv6.conf.%i.autoconf=0"
    );
    if let Some(ref ipv4) = peer.ipv4_tunnel_local {
        post_up.push_str(&format!("; ip addr add {ipv4}/32 dev %i"));
    }
    if let Some(ref ipv6) = peer.ipv6_tunnel_local {
        post_up.push_str(&format!("; ip addr add {ipv6}/128 dev %i"));
    }
    if !settings.wg_post_up.is_empty() {
        post_up.push_str(&format!("; {}", settings.wg_post_up));
    }
    config.push_str(&format!("{post_up}\n"));

    // PostDown — mirror
    let mut post_down = String::from("PostDown = ip link set %i down");
    if !settings.wg_post_down.is_empty() {
        post_down.push_str(&format!("; {}", settings.wg_post_down));
    }
    config.push_str(&format!("{post_down}\n"));

    config.push('\n');

    // [Peer] section
    config.push_str("[Peer]\n");
    if let Some(ref key) = peer.wg_public_key {
        config.push_str(&format!("PublicKey = {key}\n"));
    }
    config.push_str(&format!(
        "Endpoint = {}:{}\n",
        peer.wg_remote_address, peer.wg_remote_port
    ));

    // AllowedIPs — full DN42 prefixes + link-local
    let allowed_ips = format!(
        "{}, {}, fe80::/10",
        settings.dn42_ipv4_prefix, settings.dn42_ipv6_prefix
    );
    config.push_str(&format!("AllowedIPs = {allowed_ips}\n"));

    config.push_str("PersistentKeepalive = 25\n");

    config
}
```

- [ ] **Step 2: Update test_settings() helper**

Add new fields to `test_settings()`:

```rust
fn test_settings() -> Settings {
    Settings {
        local_asn: 4242420000,
        bird_template_name: "test".into(),
        bird_router_id: "1.2.3.4".into(),
        wg_default_listen_port: 42420,
        dn42_ipv4_prefix: "172.20.0.0/14".into(),
        dn42_ipv6_prefix: "fd00::/8".into(),
        wg_table: "off".into(),
        wg_mtu: 1420,
        wg_fwmark: 0,
        wg_post_up: String::new(),
        wg_post_down: String::new(),
        roa_mode: "none".into(),
        roa_static_v4_url: String::new(),
        roa_static_v6_url: String::new(),
        roa_rtr_address: String::new(),
        roa_rtr_port: 323,
        bird_import_limit: 9000,
        bird_export_filter: String::new(),
        bird_import_filter: String::new(),
    }
}
```

- [ ] **Step 3: Update existing tests and add new**

Update `test_generate_config_contains_sections` to check new output:

```rust
#[test]
fn test_generate_config_contains_sections() {
    let config = generate_config(&test_peer(), &test_settings());
    assert!(config.contains("[Interface]"));
    assert!(config.contains("[Peer]"));
    assert!(config.contains("PrivateKey = privkey"));
    assert!(config.contains("PublicKey = pubkey"));
    assert!(config.contains("Table = off"));
    assert!(config.contains("MTU = 1420"));
    assert!(config.contains("PostUp ="));
    assert!(config.contains("PostDown ="));
    assert!(config.contains("PersistentKeepalive = 25"));
    assert!(config.contains("fe80::/10"));
}

#[test]
fn test_generate_config_fwmark_omitted_when_zero() {
    let config = generate_config(&test_peer(), &test_settings());
    assert!(!config.contains("FwMark"));
}

#[test]
fn test_generate_config_fwmark_included_when_set() {
    let mut s = test_settings();
    s.wg_fwmark = 51820;
    let config = generate_config(&test_peer(), &s);
    assert!(config.contains("FwMark = 51820"));
}

#[test]
fn test_generate_config_post_up_has_tunnel_ips() {
    let config = generate_config(&test_peer(), &test_settings());
    assert!(config.contains("172.20.1.1/32"));
    assert!(config.contains("fd00::1/128"));
}

#[test]
fn test_generate_config_custom_post_up_appended() {
    let mut s = test_settings();
    s.wg_post_up = "ip route add 10.0.0.0/8 via 172.20.1.2".into();
    let config = generate_config(&test_peer(), &s);
    assert!(config.contains("ip route add 10.0.0.0/8 via 172.20.1.2"));
}
```

- [ ] **Step 4: Run WireGuard tests**

Run: `cargo test wireguard -- --nocapture 2>&1`
Expected: All 7 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/services/wireguard.rs
git commit -m "feat: rewrite WG config — Table=off, MTU, fwmark, PostUp/Down auto-gen"
```

---

### Task 6: BIRD Config Generation Rewrite

**Files:**
- Modify: `src/services/bird.rs`

- [ ] **Step 1: Add ROA helper functions and rewrite generate_full_config()**

Replace the content of `src/services/bird.rs` (keeping existing imports, `sanitize_name`, `generate_peer_block*` functions, and test module — only rewrite `generate_full_config` and add ROA helpers):

```rust
/// Generate ROA table definitions based on settings.roa_mode
fn generate_roa_section(settings: &Settings) -> String {
    match settings.roa_mode.as_str() {
        "static_file" => {
            let mut s = String::new();
            if !settings.roa_static_v4_url.is_empty() {
                s.push_str(&format!(
                    "# ROA data (static) — regenerate via cron every 15 min:\n\
                     #   curl -sfSL -o /etc/bird/roa_dn42_v4.conf {}\n\
                     #   curl -sfSL -o /etc/bird/roa_dn42_v6.conf {}\n",
                    settings.roa_static_v4_url, settings.roa_static_v6_url
                ));
            }
            s.push_str("include \"/etc/bird/roa_dn42_v4.conf\";\n");
            s.push_str("include \"/etc/bird/roa_dn42_v6.conf\";\n\n");
            s
        }
        "rtr" => {
            format!(
                "protocol rpki roa_dn42 {{\n\
                 \x20   roa4 {{ table dn42_roa; }};\n\
                 \x20   roa6 {{ table dn42_roa_v6; }};\n\
                 \x20   remote \"{addr}\";\n\
                 \x20   port {port};\n\
                 \x20   refresh 600;\n\
                 \x20   retry 300;\n\
                 \x20   expire 7200;\n\
                 }}\n\n",
                addr = settings.roa_rtr_address,
                port = settings.roa_rtr_port
            )
        }
        _ => {
            // none mode — create empty tables so roa_check doesn't fail
            "roa4 table dn42_roa;\nroa6 table dn42_roa_v6;\n\n".to_string()
        }
    }
}

/// Generate BIRD filter functions for DN42 best-practice prefix validation.
fn generate_filter_functions(settings: &Settings) -> String {
    format!(
        "function is_valid_network() -> bool {{\n\
         \x20 return net ~ [\n\
         \x20   {ipv4_prefix}{{21,29}},    # dn42\n\
         \x20   {ipv4_prefix}{{28,32}},    # dn42 Anycast\n\
         \x20   172.21.0.0/24{{28,32}},    # dn42 Anycast\n\
         \x20   172.22.0.0/24{{28,32}},    # dn42 Anycast\n\
         \x20   172.23.0.0/24{{28,32}},    # dn42 Anycast\n\
         \x20   172.31.0.0/16+,           # ChaosVPN\n\
         \x20   10.100.0.0/14+,           # ChaosVPN\n\
         \x20   10.127.0.0/16+,           # neonetwork\n\
         \x20   10.0.0.0/8{{15,24}}        # Freifunk.net\n\
         \x20 ];\n\
         }}\n\n\
         function is_valid_network_v6() -> bool {{\n\
         \x20 return net ~ [ {ipv6_prefix}{{44,64}} ];  # ULA\n\
         }}\n\n\
         function is_self_net() -> bool {{\n\
         \x20 return net ~ OWNNETSET;\n\
         }}\n\n",
        ipv4_prefix = settings.dn42_ipv4_prefix,
        ipv6_prefix = settings.dn42_ipv6_prefix,
    )
}
```

- [ ] **Step 2: Rewrite generate_full_config()**

```rust
pub fn generate_full_config(
    peers: &[Peer],
    settings: &Settings,
    template_body: &str,
) -> String {
    let mut config = String::new();

    // Router definition
    config.push_str(&format!("router id {};\n\n", settings.bird_router_id));

    // ASN and routing tables
    config.push_str(&format!("define OWNAS = {};\n", settings.local_asn));
    config.push_str(&format!(
        "define OWNNETSET = [{}+, {}];\n\n",
        settings.dn42_ipv4_prefix, settings.dn42_ipv6_prefix
    ));

    // Enable RPKI if configured
    if settings.roa_mode != "none" {
        config.push_str("roa4 table dn42_roa;\nroa6 table dn42_roa_v6;\n\n");
    }

    // ROA data section
    if settings.roa_mode != "none" {
        config.push_str(&generate_roa_section(settings));
    }

    // Prefix validation filters
    config.push_str(&generate_filter_functions(settings));

    // Default import/export filter bodies
    let import_filter = if settings.bird_import_filter.is_empty() {
        format!(
            "if is_valid_network() && !is_self_net() then {{\n\
             \x20   if (roa_check(dn42_roa, net, bgp_path.last) != ROA_VALID) then {{\n\
             \x20     print \"[dn42] ROA check failed for \", net, \" ASN \", bgp_path.last;\n\
             \x20     reject;\n\
             \x20   }} else accept;\n\
             \x20 }} else reject;"
        )
    } else {
        settings.bird_import_filter.clone()
    };

    let export_filter = if settings.bird_export_filter.is_empty() {
        "if is_valid_network() && source ~ [RTS_STATIC, RTS_BGP] then accept; else reject;"
            .to_string()
    } else {
        settings.bird_export_filter.clone()
    };

    // BGP template
    config.push_str(&format!(
        "template bgp {tpl} {{\n",
        tpl = settings.bird_template_name
    ));
    if !template_body.is_empty() {
        config.push_str(template_body);
    } else {
        config.push_str("    local as OWNAS;\n");
        config.push_str("    path metric 1;\n");
        config.push_str(&format!(
            "    ipv4 {{\n\
             \x20       import filter {{\n\
             \x20         {import_filter}\n\
             \x20       }};\n\
             \x20       export filter {{ {export_filter} }};\n\
             \x20       import limit {} action block;\n\
             \x20     }};\n",
            settings.bird_import_limit
        ));
        config.push_str(&format!(
            "    ipv6 {{\n\
             \x20       import filter {{\n\
             \x20         if is_valid_network_v6() && !is_self_net() then {{\n\
             \x20           if (roa_check(dn42_roa_v6, net, bgp_path.last) != ROA_VALID) then {{\n\
             \x20             print \"[dn42] ROA check failed for \", net, \" ASN \", bgp_path.last;\n\
             \x20             reject;\n\
             \x20           }} else accept;\n\
             \x20         }} else reject;\n\
             \x20       }};\n\
             \x20       export filter {{ if is_valid_network_v6() && source ~ [RTS_STATIC, RTS_BGP] then accept; else reject; }};\n\
             \x20       import limit {} action block;\n\
             \x20     }};\n",
            settings.bird_import_limit
        ));
        config.push_str("    import table;\n");
    }
    config.push_str("}\n\n");

    // Peer blocks
    for peer in peers.iter().filter(|p| p.enabled) {
        config.push_str(&generate_peer_block(peer, settings));
    }

    config
}
```

- [ ] **Step 3: Update test_settings() in bird.rs test module**

```rust
fn test_settings() -> Settings {
    Settings {
        local_asn: 4242420000,
        bird_template_name: "dnpeers".into(),
        bird_router_id: "172.20.0.1".into(),
        wg_default_listen_port: 42420,
        dn42_ipv4_prefix: "172.20.0.0/14".into(),
        dn42_ipv6_prefix: "fd00::/8".into(),
        wg_table: "off".into(),
        wg_mtu: 1420,
        wg_fwmark: 0,
        wg_post_up: String::new(),
        wg_post_down: String::new(),
        roa_mode: "none".into(),
        roa_static_v4_url: String::new(),
        roa_static_v6_url: String::new(),
        roa_rtr_address: String::new(),
        roa_rtr_port: 323,
        bird_import_limit: 9000,
        bird_export_filter: String::new(),
        bird_import_filter: String::new(),
    }
}
```

- [ ] **Step 4: Add new tests**

```rust
#[test]
fn test_generate_full_config_has_roa_when_mode_is_rtr() {
    let mut s = test_settings();
    s.roa_mode = "rtr".into();
    s.roa_rtr_address = "rpki.dn42.example".into();
    let config = generate_full_config(&[], &s, "");
    assert!(config.contains("protocol rpki roa_dn42"));
    assert!(config.contains("rpki.dn42.example"));
}

#[test]
fn test_generate_full_config_has_static_roa() {
    let mut s = test_settings();
    s.roa_mode = "static_file".into();
    s.roa_static_v4_url = "https://example.com/roa_v4.conf".into();
    let config = generate_full_config(&[], &s, "");
    assert!(config.contains("include \"/etc/bird/roa_dn42_v4.conf\""));
}

#[test]
fn test_generate_full_config_has_filter_functions() {
    let config = generate_full_config(&[], &test_settings(), "");
    assert!(config.contains("function is_valid_network()"));
    assert!(config.contains("function is_valid_network_v6()"));
    assert!(config.contains("function is_self_net()"));
}

#[test]
fn test_generate_full_config_has_import_limit() {
    let config = generate_full_config(&[], &test_settings(), "");
    assert!(config.contains("import limit 9000 action block"));
}

#[test]
fn test_generate_full_config_has_roa_check() {
    let config = generate_full_config(&[], &test_settings(), "");
    assert!(config.contains("roa_check(dn42_roa, net, bgp_path.last)"));
}

#[test]
fn test_generate_full_config_uses_custom_export_filter() {
    let mut s = test_settings();
    s.bird_export_filter = "accept;".into();
    let config = generate_full_config(&[], &s, "");
    assert!(config.contains("export filter { accept; }"));
}
```

- [ ] **Step 5: Run BIRD tests**

Run: `cargo test bird -- --nocapture 2>&1`
Expected: All tests PASS.

- [ ] **Step 6: Commit**

```bash
git add src/services/bird.rs
git commit -m "feat: rewrite BIRD config — ROA filtering, prefix validation, import limit, import table"
```

---

### Task 7: Community Mapper 3-Dimension + MED

**Files:**
- Modify: `src/services/community_mapper.rs`

- [ ] **Step 1: Add compute_med() function and update compute_communities()**

Replace the file content:

```rust
use crate::models::community::CommunityRuleRepository;
use crate::models::peer::Peer;
use crate::models::probe::ProbeResultRepository;

pub struct CommunityMapper;

impl CommunityMapper {
    /// Compute which community tags match a peer based on the latest probe
    /// result between the local node and the peer's origin node.
    ///
    /// Matching is 3-dimensional: latency, bandwidth, crypto weight.
    pub async fn compute_communities(
        peer: &Peer,
        local_node_id: &str,
        probe_repo: &ProbeResultRepository,
        rule_repo: &CommunityRuleRepository,
    ) -> Result<(Vec<String>, Vec<String>), crate::error::AppError> {
        let rules = rule_repo.list_enabled().await?;

        let origin_node_id = peer.origin_node_id.as_deref().unwrap_or(local_node_id);

        // Default probe values for local (no cross-node latency)
        let (latency, loss_pct, _bandwidth_mbps) = if origin_node_id == local_node_id {
            (0.0, 0.0, 0.0)
        } else {
            match probe_repo
                .latest_between(local_node_id, origin_node_id)
                .await?
            {
                Some(probe) => (probe.avg_latency_ms, probe.packet_loss_pct, 0.0),
                None => return Ok((Vec::new(), Vec::new())),
            }
        };

        // Infer crypto weight from peer config — WireGuard uses 1 by default
        let crypto_weight: i32 = if peer.wg_private_key.is_some() { 1 } else { 0 };

        let mut v4 = Vec::new();
        let mut v6 = Vec::new();

        for rule in &rules {
            if !rule.enabled {
                continue;
            }
            // 3-dimension match:
            //   latency <= max_latency_ms (0 = infinity)
            //   loss <= max_packet_loss_pct
            //   min_bandwidth_mbps (0 = no requirement)
            //   crypto_weight match (0 = no requirement)
            let lat_ok = rule.max_latency_ms <= 0.0 || latency <= rule.max_latency_ms;
            let loss_ok = loss_pct <= rule.max_packet_loss_pct;
            let bw_ok = rule.min_bandwidth_mbps <= 0.0; // bandwidth not yet measured, skip for now
            let crypto_ok = rule.crypto_weight == 0 || crypto_weight >= rule.crypto_weight;

            if lat_ok && loss_ok && bw_ok && crypto_ok {
                v4.push(rule.community_ipv4.clone());
                v6.push(rule.community_ipv6.clone());
            }
        }

        Ok((v4, v6))
    }

    /// Compute BGP MED value for a peer based on matched community rules.
    /// MED = sum of matched rule penalties, where higher MED = less preferred.
    pub async fn compute_med(
        peer: &Peer,
        local_node_id: &str,
        probe_repo: &ProbeResultRepository,
        rule_repo: &CommunityRuleRepository,
    ) -> Result<i32, crate::error::AppError> {
        let rules = rule_repo.list_enabled().await?;

        let origin_node_id = peer.origin_node_id.as_deref().unwrap_or(local_node_id);

        let latency = if origin_node_id == local_node_id {
            0.0
        } else {
            match probe_repo
                .latest_between(local_node_id, origin_node_id)
                .await?
            {
                Some(probe) => probe.avg_latency_ms,
                None => return Ok(1000), // no probe data → high MED (deprioritize)
            }
        };

        let crypto_weight: i32 = if peer.wg_private_key.is_some() { 1 } else { 0 };

        let mut med: i32 = 0;

        for rule in &rules {
            if !rule.enabled {
                continue;
            }
            let lat_ok = rule.max_latency_ms <= 0.0 || latency <= rule.max_latency_ms;
            let crypto_ok = rule.crypto_weight == 0 || crypto_weight >= rule.crypto_weight;

            if lat_ok && crypto_ok {
                med += rule.med_penalty;
            }
        }

        Ok(med)
    }

    /// Generate BIRD export filter lines for community tags.
    pub fn to_bird_filter_lines(communities_v4: &[String], communities_v6: &[String]) -> String {
        let mut lines = String::new();

        if !communities_v4.is_empty() {
            lines.push_str("    ipv4 {\n        export filter {\n");
            for c in communities_v4 {
                lines.push_str(&format!(
                    "            bgp_community.add(({}));\n",
                    c
                ));
            }
            lines.push_str("            accept;\n        };\n    };\n");
        }

        if !communities_v6.is_empty() {
            lines.push_str("    ipv6 {\n        export filter {\n");
            for c in communities_v6 {
                lines.push_str(&format!(
                    "            bgp_community.add(({}));\n",
                    c
                ));
            }
            lines.push_str("            accept;\n        };\n    };\n");
        }

        lines
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check 2>&1 | tail -5`
Expected: Compiles. The `bandwidth_mbps` field isn't yet on `ProbeResult` — we use 0.0 as placeholder, future expansion.

- [ ] **Step 3: Commit**

```bash
git add src/services/community_mapper.rs
git commit -m "feat: add 3D community matching + MED computation"
```

---

### Task 8: Settings gRPC Service Wire

**Files:**
- Modify: `src/grpc/settings_service.rs`

- [ ] **Step 1: Update settings_to_proto() with new fields**

Replace the function:

```rust
fn settings_to_proto(s: &crate::models::settings::Settings) -> Settings {
    Settings {
        local_asn: s.local_asn,
        bird_template_name: s.bird_template_name.clone(),
        bird_router_id: s.bird_router_id.clone(),
        wg_default_listen_port: s.wg_default_listen_port as u32,
        dn42_ipv4_prefix: s.dn42_ipv4_prefix.clone(),
        dn42_ipv6_prefix: s.dn42_ipv6_prefix.clone(),
        wg_table: s.wg_table.clone(),
        wg_mtu: s.wg_mtu as u32,
        wg_fwmark: s.wg_fwmark as u32,
        wg_post_up: s.wg_post_up.clone(),
        wg_post_down: s.wg_post_down.clone(),
        roa_mode: s.roa_mode.clone(),
        roa_static_v4_url: s.roa_static_v4_url.clone(),
        roa_static_v6_url: s.roa_static_v6_url.clone(),
        roa_rtr_address: s.roa_rtr_address.clone(),
        roa_rtr_port: s.roa_rtr_port as u32,
        bird_import_limit: s.bird_import_limit as u32,
        bird_export_filter: s.bird_export_filter.clone(),
        bird_import_filter: s.bird_import_filter.clone(),
    }
}
```

- [ ] **Step 2: Update apply_settings() with new fields**

Replace the function:

```rust
fn apply_settings(s: &mut crate::models::settings::Settings, proto: &Settings) {
    if proto.local_asn != 0 {
        s.local_asn = proto.local_asn;
    }
    if !proto.bird_template_name.is_empty() {
        s.bird_template_name = proto.bird_template_name.clone();
    }
    if !proto.bird_router_id.is_empty() {
        s.bird_router_id = proto.bird_router_id.clone();
    }
    if proto.wg_default_listen_port != 0 {
        s.wg_default_listen_port = proto.wg_default_listen_port as i64;
    }
    if !proto.dn42_ipv4_prefix.is_empty() {
        s.dn42_ipv4_prefix = proto.dn42_ipv4_prefix.clone();
    }
    if !proto.dn42_ipv6_prefix.is_empty() {
        s.dn42_ipv6_prefix = proto.dn42_ipv6_prefix.clone();
    }
    if !proto.wg_table.is_empty() {
        s.wg_table = proto.wg_table.clone();
    }
    if proto.wg_mtu != 0 {
        s.wg_mtu = proto.wg_mtu as i64;
    }
    if proto.wg_fwmark != 0 {
        s.wg_fwmark = proto.wg_fwmark as i64;
    }
    if !proto.wg_post_up.is_empty() {
        s.wg_post_up = proto.wg_post_up.clone();
    }
    if !proto.wg_post_down.is_empty() {
        s.wg_post_down = proto.wg_post_down.clone();
    }
    if !proto.roa_mode.is_empty() {
        s.roa_mode = proto.roa_mode.clone();
    }
    if !proto.roa_static_v4_url.is_empty() {
        s.roa_static_v4_url = proto.roa_static_v4_url.clone();
    }
    if !proto.roa_static_v6_url.is_empty() {
        s.roa_static_v6_url = proto.roa_static_v6_url.clone();
    }
    if !proto.roa_rtr_address.is_empty() {
        s.roa_rtr_address = proto.roa_rtr_address.clone();
    }
    if proto.roa_rtr_port != 0 {
        s.roa_rtr_port = proto.roa_rtr_port as i64;
    }
    if proto.bird_import_limit != 0 {
        s.bird_import_limit = proto.bird_import_limit as i64;
    }
    if !proto.bird_export_filter.is_empty() {
        s.bird_export_filter = proto.bird_export_filter.clone();
    }
    if !proto.bird_import_filter.is_empty() {
        s.bird_import_filter = proto.bird_import_filter.clone();
    }
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check 2>&1 | tail -5`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add src/grpc/settings_service.rs
git commit -m "feat: wire new Settings fields in gRPC service"
```

---

### Task 9: Community gRPC Service Wire

**Files:**
- Modify: `src/grpc/cluster_service.rs`

- [ ] **Step 1: Update community_rule_to_proto() with new fields**

Replace the function:

```rust
fn community_rule_to_proto(r: &crate::models::community::CommunityRule) -> CommunityRule {
    CommunityRule {
        id: r.id.clone(),
        description: r.description.clone().unwrap_or_default(),
        max_latency_ms: r.max_latency_ms,
        max_packet_loss_pct: r.max_packet_loss_pct,
        community_ipv4: r.community_ipv4.clone(),
        community_ipv6: r.community_ipv6.clone(),
        enabled: r.enabled,
        min_bandwidth_mbps: r.min_bandwidth_mbps,
        crypto_weight: r.crypto_weight,
        med_penalty: r.med_penalty,
    }
}
```

- [ ] **Step 2: Update save_community_rule() to map new fields**

Replace the CommunityRule construction in `save_community_rule()`:

```rust
let rule = crate::models::community::CommunityRule {
    id: if proto.id.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        proto.id.clone()
    },
    description: if proto.description.is_empty() {
        None
    } else {
        Some(proto.description.clone())
    },
    max_latency_ms: proto.max_latency_ms,
    max_packet_loss_pct: proto.max_packet_loss_pct,
    community_ipv4: proto.community_ipv4.clone(),
    community_ipv6: proto.community_ipv6.clone(),
    enabled: proto.enabled,
    min_bandwidth_mbps: proto.min_bandwidth_mbps,
    crypto_weight: proto.crypto_weight,
    med_penalty: proto.med_penalty,
    created_at: String::new(),
    updated_at: String::new(),
};
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check 2>&1 | tail -5`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add src/grpc/cluster_service.rs
git commit -m "feat: wire new CommunityRule fields in gRPC service"
```

---

### Task 10: Full Build + Test Checkpoint

**Files:** (verification only, no edits)

- [ ] **Step 1: Full build**

Run: `cargo build 2>&1 | tail -15`
Expected: Compiles successfully. Frontend built via build.rs.

- [ ] **Step 2: Run all tests**

Run: `cargo test 2>&1 | tail -10`
Expected: All tests pass.

- [ ] **Step 3: Run clippy**

Run: `cargo clippy 2>&1 | tail -10`
Expected: No warnings.

- [ ] **Step 4: TypeScript type-check**

Run: `cd frontend && pnpm exec tsc --noEmit 2>&1 | tail -10`
Expected: May show errors for SettingsForm.tsx and CommunityRules.tsx (proto types changed, frontend not yet updated). Note them — we fix in Tasks 11-12.

- [ ] **Step 5: No commit** (verification checkpoint only)

---

### Task 11: Frontend Settings Page

**Files:**
- Modify: `frontend/src/components/settings/SettingsForm.tsx`

- [ ] **Step 1: Expand form state and rewrite component**

Rewrite `frontend/src/components/settings/SettingsForm.tsx`:

```tsx
import { useState, useEffect, type FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { ArrowLeft } from 'lucide-react';
import { create } from '@bufbuild/protobuf';
import { SettingsSchema } from '../../lib/peerman_pb';
import { useSettings } from '../../hooks/useSettings';

const DEFAULT_FORM = {
  localAsn: '4242420000',
  birdTemplateName: 'dnpeers',
  birdRouterId: '172.20.0.1',
  wgDefaultListenPort: '42420',
  dn42Ipv4Prefix: '172.20.0.0/14',
  dn42Ipv6Prefix: 'fd00::/8',
  wgTable: 'off',
  wgMtu: '1420',
  wgFwmark: '0',
  wgPostUp: '',
  wgPostDown: '',
  roaMode: 'none',
  roaStaticV4Url: '',
  roaStaticV6Url: '',
  roaRtrAddress: '',
  roaRtrPort: '323',
  birdImportLimit: '9000',
  birdExportFilter: '',
  birdImportFilter: '',
};

export default function SettingsPage() {
  const navigate = useNavigate();
  const { settings, loading, saveSettings } = useSettings();
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const [saved, setSaved] = useState(false);
  const [form, setForm] = useState(DEFAULT_FORM);

  useEffect(() => {
    if (settings) {
      setForm({
        localAsn: settings.localAsn.toString(),
        birdTemplateName: settings.birdTemplateName,
        birdRouterId: settings.birdRouterId,
        wgDefaultListenPort: String(settings.wgDefaultListenPort),
        dn42Ipv4Prefix: settings.dn42Ipv4Prefix,
        dn42Ipv6Prefix: settings.dn42Ipv6Prefix,
        wgTable: settings.wgTable,
        wgMtu: String(settings.wgMtu),
        wgFwmark: String(settings.wgFwmark),
        wgPostUp: settings.wgPostUp,
        wgPostDown: settings.wgPostDown,
        roaMode: settings.roaMode || 'none',
        roaStaticV4Url: settings.roaStaticV4Url,
        roaStaticV6Url: settings.roaStaticV6Url,
        roaRtrAddress: settings.roaRtrAddress,
        roaRtrPort: String(settings.roaRtrPort || '323'),
        birdImportLimit: String(settings.birdImportLimit || '9000'),
        birdExportFilter: settings.birdExportFilter,
        birdImportFilter: settings.birdImportFilter,
      });
    }
  }, [settings]);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setError('');
    setSaved(false);

    const s = create(SettingsSchema, {
      localAsn: BigInt(form.localAsn || '0'),
      birdTemplateName: form.birdTemplateName,
      birdRouterId: form.birdRouterId,
      wgDefaultListenPort: Number(form.wgDefaultListenPort || '0'),
      dn42Ipv4Prefix: form.dn42Ipv4Prefix,
      dn42Ipv6Prefix: form.dn42Ipv6Prefix,
      wgTable: form.wgTable,
      wgMtu: Number(form.wgMtu || '0'),
      wgFwmark: Number(form.wgFwmark || '0'),
      wgPostUp: form.wgPostUp,
      wgPostDown: form.wgPostDown,
      roaMode: form.roaMode,
      roaStaticV4Url: form.roaStaticV4Url,
      roaStaticV6Url: form.roaStaticV6Url,
      roaRtrAddress: form.roaRtrAddress,
      roaRtrPort: Number(form.roaRtrPort || '0'),
      birdImportLimit: Number(form.birdImportLimit || '0'),
      birdExportFilter: form.birdExportFilter,
      birdImportFilter: form.birdImportFilter,
    });

    try {
      await saveSettings(s);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  if (loading) return <div className="p-xl text-body">Loading settings...</div>;

  const f = <K extends keyof typeof form>(k: K) => form[k];

  return (
    <div className="max-w-2xl mx-auto animate-fade-in">
      <div className="flex items-center gap-md mb-lg">
        <button onClick={() => navigate(-1)} className="btn-ghost">
          <ArrowLeft className="w-4 h-4" />
        </button>
        <h1 className="text-display-md text-ink">Settings</h1>
      </div>

      {error && (
        <div className="bg-error-soft text-error-deep text-body-sm px-md py-sm rounded-sm mb-lg">{error}</div>
      )}
      {saved && (
        <div className="bg-cyan-soft text-cyan-deep text-body-sm px-md py-sm rounded-sm mb-lg">Settings saved.</div>
      )}

      <form onSubmit={handleSubmit} className="space-y-lg">
        {/* Global Configuration */}
        <div className="card space-y-md">
          <h2 className="text-body-sm-strong text-ink">Global Configuration</h2>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-sm">
            <Input label="Local ASN" value={f('localAsn')} onChange={(v) => setForm((p) => ({ ...p, localAsn: v }))} />
            <Input label="BIRD Template Name" value={f('birdTemplateName')} onChange={(v) => setForm((p) => ({ ...p, birdTemplateName: v }))} />
            <Input label="BIRD Router ID" value={f('birdRouterId')} onChange={(v) => setForm((p) => ({ ...p, birdRouterId: v }))} />
            <Input label="Default WG Listen Port" value={f('wgDefaultListenPort')} onChange={(v) => setForm((p) => ({ ...p, wgDefaultListenPort: v }))} type="number" />
            <Input label="DN42 IPv4 Prefix" value={f('dn42Ipv4Prefix')} onChange={(v) => setForm((p) => ({ ...p, dn42Ipv4Prefix: v }))} />
            <Input label="DN42 IPv6 Prefix" value={f('dn42Ipv6Prefix')} onChange={(v) => setForm((p) => ({ ...p, dn42Ipv6Prefix: v }))} />
            <Input label="WG Table" value={f('wgTable')} onChange={(v) => setForm((p) => ({ ...p, wgTable: v }))} />
          </div>
        </div>

        {/* WireGuard Advanced */}
        <div className="card space-y-md">
          <h2 className="text-body-sm-strong text-ink">WireGuard Advanced</h2>
          <p className="text-body-sm text-mute">Tunnel-level settings applied to each peer's config.</p>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-sm">
            <Input label="MTU" value={f('wgMtu')} onChange={(v) => setForm((p) => ({ ...p, wgMtu: v }))} type="number" />
            <Input label="FwMark (0 = disabled)" value={f('wgFwmark')} onChange={(v) => setForm((p) => ({ ...p, wgFwmark: v }))} type="number" />
          </div>
          <div className="grid grid-cols-1 gap-sm">
            <Textarea label="PostUp Script" value={f('wgPostUp')} onChange={(v) => setForm((p) => ({ ...p, wgPostUp: v }))} placeholder="Additional commands after interface up" />
            <Textarea label="PostDown Script" value={f('wgPostDown')} onChange={(v) => setForm((p) => ({ ...p, wgPostDown: v }))} placeholder="Additional commands before interface down" />
          </div>
        </div>

        {/* ROA/RPKI */}
        <div className="card space-y-md">
          <h2 className="text-body-sm-strong text-ink">ROA / RPKI Filtering</h2>
          <p className="text-body-sm text-mute">Reject routes with invalid or unknown ROA status.</p>
          <div className="flex items-center gap-sm">
            {(['none', 'static_file', 'rtr'] as const).map((mode) => (
              <button
                key={mode}
                type="button"
                onClick={() => setForm((p) => ({ ...p, roaMode: mode }))}
                className={f('roaMode') === mode ? 'tab-active' : 'tab-ghost'}
              >
                {mode === 'none' ? 'None' : mode === 'static_file' ? 'Static File' : 'RTR'}
              </button>
            ))}
          </div>
          {f('roaMode') === 'static_file' && (
            <div className="grid grid-cols-1 gap-sm">
              <Input label="ROA v4 URL" value={f('roaStaticV4Url')} onChange={(v) => setForm((p) => ({ ...p, roaStaticV4Url: v }))} placeholder="https://dn42.burble.com/roa/dn42_roa_bird2_4.conf" />
              <Input label="ROA v6 URL" value={f('roaStaticV6Url')} onChange={(v) => setForm((p) => ({ ...p, roaStaticV6Url: v }))} placeholder="https://dn42.burble.com/roa/dn42_roa_bird2_6.conf" />
            </div>
          )}
          {f('roaMode') === 'rtr' && (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-sm">
              <Input label="RTR Address" value={f('roaRtrAddress')} onChange={(v) => setForm((p) => ({ ...p, roaRtrAddress: v }))} placeholder="rpki.akae.re" />
              <Input label="RTR Port" value={f('roaRtrPort')} onChange={(v) => setForm((p) => ({ ...p, roaRtrPort: v }))} type="number" />
            </div>
          )}
        </div>

        {/* BIRD Filter */}
        <div className="card space-y-md">
          <h2 className="text-body-sm-strong text-ink">BIRD Filter Templates</h2>
          <p className="text-body-sm text-mute">Custom filter bodies override auto-generated best-practice defaults. Leave empty for defaults.</p>
          <div className="grid grid-cols-1 gap-sm">
            <Input label="Import Prefix Limit" value={f('birdImportLimit')} onChange={(v) => setForm((p) => ({ ...p, birdImportLimit: v }))} type="number" />
          </div>
          <div className="grid grid-cols-1 gap-sm">
            <Textarea label="Import Filter Body" value={f('birdImportFilter')} onChange={(v) => setForm((p) => ({ ...p, birdImportFilter: v }))} placeholder="if is_valid_network() && !is_self_net() then { ... }" code />
            <Textarea label="Export Filter Body" value={f('birdExportFilter')} onChange={(v) => setForm((p) => ({ ...p, birdExportFilter: v }))} placeholder="if is_valid_network() && source ~ [RTS_STATIC, RTS_BGP] then accept; else reject;" code />
          </div>
        </div>

        <button type="submit" disabled={saving} className="btn-primary">
          {saving ? 'Saving...' : 'Save Settings'}
        </button>
      </form>
    </div>
  );
}

function Input({
  label, value, onChange, type = 'text', placeholder,
}: {
  label: string; value: string; onChange: (v: string) => void; type?: string; placeholder?: string;
}) {
  return (
    <div className="flex flex-col gap-1">
      <label className="text-caption text-mute">{label}</label>
      <input
        type={type}
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        className="form-input"
      />
    </div>
  );
}

function Textarea({
  label, value, onChange, placeholder, code,
}: {
  label: string; value: string; onChange: (v: string) => void; placeholder?: string; code?: boolean;
}) {
  return (
    <div className="flex flex-col gap-1">
      <label className="text-caption text-mute">{label}</label>
      <textarea
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        rows={3}
        className={code ? 'form-input font-mono text-code' : 'form-input'}
        style={code ? { fontFamily: 'Geist Mono, ui-monospace, monospace', fontSize: '13px' } : undefined}
      />
    </div>
  );
}
```

- [ ] **Step 2: TypeScript type-check**

Run: `cd frontend && pnpm exec tsc --noEmit 2>&1`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add frontend/src/components/settings/SettingsForm.tsx
git commit -m "feat: expand Settings UI — WG advanced, ROA/RPKI, BIRD filter templates"
```

---

### Task 12: Frontend Community Rules Page

**Files:**
- Modify: `frontend/src/components/communities/CommunityRules.tsx`

- [ ] **Step 1: Add new form fields and update table**

Rewrite `frontend/src/components/communities/CommunityRules.tsx`:

```tsx
import { useState } from 'react';
import { Plus, Trash2, Check, X } from 'lucide-react';
import { create } from '@bufbuild/protobuf';
import { CommunityRuleSchema } from '../../lib/peerman_pb';
import type { CommunityRule } from '../../lib/peerman_pb';
import { useCommunityRules, useSaveCommunityRule, useDeleteCommunityRule } from '../../hooks/useCommunities';

type Form = {
  description: string;
  maxLatencyMs: string;
  maxPacketLossPct: string;
  communityIpv4: string;
  communityIpv6: string;
  minBandwidthMbps: string;
  cryptoWeight: string;
  medPenalty: string;
};

const emptyForm: Form = {
  description: '', maxLatencyMs: '', maxPacketLossPct: '100',
  communityIpv4: '', communityIpv6: '',
  minBandwidthMbps: '0', cryptoWeight: '0', medPenalty: '0',
};

function ruleToForm(r: CommunityRule): Form {
  return {
    description: r.description,
    maxLatencyMs: String(r.maxLatencyMs),
    maxPacketLossPct: String(r.maxPacketLossPct),
    communityIpv4: r.communityIpv4,
    communityIpv6: r.communityIpv6,
    minBandwidthMbps: String(r.minBandwidthMbps),
    cryptoWeight: String(r.cryptoWeight),
    medPenalty: String(r.medPenalty),
  };
}

export default function CommunityRules() {
  const { rules, loading, error, refetch } = useCommunityRules();
  const { save, loading: saving } = useSaveCommunityRule();
  const { del, loading: deleting } = useDeleteCommunityRule();

  const [editing, setEditing] = useState<CommunityRule | null>(null);
  const [form, setForm] = useState<Form>(emptyForm);

  const startNew = () => {
    setEditing(create(CommunityRuleSchema, {
      id: '', description: '', maxLatencyMs: 0, maxPacketLossPct: 100,
      communityIpv4: '', communityIpv6: '', enabled: true,
      minBandwidthMbps: 0, cryptoWeight: 0, medPenalty: 0,
    }));
    setForm(emptyForm);
  };

  const startEdit = (rule: CommunityRule) => {
    setEditing(rule);
    setForm(ruleToForm(rule));
  };

  const handleSave = async () => {
    if (!editing) return;
    const rule = create(CommunityRuleSchema, {
      id: editing.id,
      description: form.description,
      maxLatencyMs: parseFloat(form.maxLatencyMs) || 100000,
      maxPacketLossPct: parseFloat(form.maxPacketLossPct) ?? 100,
      communityIpv4: form.communityIpv4,
      communityIpv6: form.communityIpv6,
      enabled: true,
      minBandwidthMbps: parseFloat(form.minBandwidthMbps) || 0,
      cryptoWeight: parseInt(form.cryptoWeight) || 0,
      medPenalty: parseInt(form.medPenalty) || 0,
    });
    await save(rule);
    setEditing(null);
    refetch();
  };

  const handleDelete = async (id: string) => {
    if (!confirm('Delete this rule?')) return;
    await del(id);
    refetch();
  };

  if (loading) return <div className="text-mute p-lg">Loading...</div>;
  if (error) return <div className="text-error p-lg">{error}</div>;

  const fmtNum = (n: number) => n <= 0 ? '∞' : String(n);
  const fmtInf = (n: number, suffix: string) => n <= 0 ? '∞' : `${n}${suffix}`;

  return (
    <div className="space-y-lg animate-fade-in max-w-3xl">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-display-md text-ink">Community Rules</h1>
          <p className="text-body-sm text-mute mt-xxs">
            Auto-tag BGP communities based on latency, bandwidth, and crypto weight
          </p>
        </div>
        <button onClick={startNew} className="btn-primary inline-flex items-center gap-1.5">
          <Plus className="w-4 h-4" /> Add Rule
        </button>
      </div>

      {/* Inline Editor */}
      {editing && (
        <div className="card border border-link/20 bg-canvas-soft">
          <h3 className="text-body-md-strong text-ink mb-md">{editing.id ? 'Edit Rule' : 'New Rule'}</h3>
          <div className="grid grid-cols-2 gap-md mb-md">
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">Description</label>
              <input className="form-input w-full" value={form.description}
                onChange={e => setForm({ ...form, description: e.target.value })}
                placeholder="e.g., Metro (<5ms)" />
            </div>
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">Max Latency (ms, 0=∞)</label>
              <input className="form-input w-full" type="number" value={form.maxLatencyMs}
                onChange={e => setForm({ ...form, maxLatencyMs: e.target.value })} />
            </div>
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">Max Packet Loss (%)</label>
              <input className="form-input w-full" type="number" value={form.maxPacketLossPct}
                onChange={e => setForm({ ...form, maxPacketLossPct: e.target.value })} />
            </div>
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">Min Bandwidth (Mbps, 0=∞)</label>
              <input className="form-input w-full" type="number" value={form.minBandwidthMbps}
                onChange={e => setForm({ ...form, minBandwidthMbps: e.target.value })} />
            </div>
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">Crypto Weight</label>
              <input className="form-input w-full" type="number" value={form.cryptoWeight}
                onChange={e => setForm({ ...form, cryptoWeight: e.target.value })} />
            </div>
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">MED Penalty</label>
              <input className="form-input w-full" type="number" value={form.medPenalty}
                onChange={e => setForm({ ...form, medPenalty: e.target.value })} />
            </div>
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">IPv4 Community</label>
              <input className="form-input w-full" value={form.communityIpv4}
                onChange={e => setForm({ ...form, communityIpv4: e.target.value })}
                placeholder="<asn>,10" />
            </div>
            <div>
              <label className="block text-caption-mono text-mute mb-xxs">IPv6 Community</label>
              <input className="form-input w-full" value={form.communityIpv6}
                onChange={e => setForm({ ...form, communityIpv6: e.target.value })}
                placeholder="<asn>,610" />
            </div>
          </div>
          <div className="flex items-center gap-sm">
            <button onClick={handleSave} disabled={saving} className="btn-primary text-body-sm inline-flex items-center gap-1">
              <Check className="w-3.5 h-3.5" /> Save
            </button>
            <button onClick={() => setEditing(null)} className="btn-secondary text-body-sm inline-flex items-center gap-1">
              <X className="w-3.5 h-3.5" /> Cancel
            </button>
          </div>
        </div>
      )}

      {/* Rules Table */}
      <div className="card overflow-hidden !p-0">
        <table className="data-table w-full">
          <thead>
            <tr>
              <th>Description</th>
              <th>Latency</th>
              <th>Loss</th>
              <th>Bandwidth</th>
              <th>Crypto</th>
              <th>MED</th>
              <th>IPv4</th>
              <th>IPv6</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {rules.map(r => (
              <tr key={r.id}>
                <td className="text-body-sm font-medium">{r.description}</td>
                <td className="text-body-sm text-mute">{fmtInf(r.maxLatencyMs, 'ms')}</td>
                <td className="text-body-sm text-mute">{r.maxPacketLossPct}%</td>
                <td className="text-body-sm text-mute">{fmtInf(r.minBandwidthMbps, ' Mbps')}</td>
                <td className="text-body-sm text-mute">{r.cryptoWeight || '-'}</td>
                <td className="text-body-sm text-mute">{r.medPenalty || '-'}</td>
                <td><code className="text-code text-body-sm">{r.communityIpv4}</code></td>
                <td><code className="text-code text-body-sm">{r.communityIpv6}</code></td>
                <td>
                  <div className="flex items-center gap-1">
                    <button onClick={() => startEdit(r)} className="btn-secondary text-caption px-xs py-0.5">Edit</button>
                    <button onClick={() => handleDelete(r.id)} disabled={deleting} className="p-1 rounded-sm hover:bg-error-soft text-mute hover:text-error">
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: TypeScript type-check**

Run: `cd frontend && pnpm exec tsc --noEmit 2>&1`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add frontend/src/components/communities/CommunityRules.tsx
git commit -m "feat: expand Community Rules UI — bandwidth, crypto, MED penalty columns"
```

---

### Task 13: Final Verification

**Files:** (verification only)

- [ ] **Step 1: Full build**

Run: `cargo build 2>&1 | tail -5`
Expected: Compiles successfully.

- [ ] **Step 2: Run all Rust tests**

Run: `cargo test 2>&1 | tail -5`
Expected: All tests pass.

- [ ] **Step 3: Clippy**

Run: `cargo clippy 2>&1 | tail -5`
Expected: No warnings.

- [ ] **Step 4: TypeScript type-check**

Run: `cd frontend && pnpm exec tsc --noEmit 2>&1`
Expected: No errors.

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "chore: final verification — all tests pass, clippy clean, tsc clean"
```
