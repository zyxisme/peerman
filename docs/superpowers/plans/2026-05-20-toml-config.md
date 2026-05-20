# TOML Config File Migration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace 7 individual CLI flags with a single `-c config.toml` argument backed by a TOML configuration file.

**Architecture:** `Cli` struct (clap) handles only `-c`/`--config`. `Config` struct (serde + toml) holds business configuration with sub-sections for server, storage, logging, and cluster. `Config::load(path)` reads and deserializes the TOML file. `main.rs` field paths updated for the new nesting.

**Tech Stack:** clap 4, toml 0.8, serde (already present)

---

### Task 1: Add `toml` dependency

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add `toml = "0.8"` to dependencies**

```toml
toml = "0.8"
```

Add this line after `uuid` and before `chrono` in the `[dependencies]` section.

- [ ] **Step 2: Verify it compiles without config changes**

Run: `cargo check 2>&1 | head -5`
Expected: Compilation succeeds (toml crate resolves, no code uses it yet).

---

### Task 2: Rewrite `src/config.rs`

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Write the new `src/config.rs`**

Replace the entire file with:

```rust
use clap::Parser;
use serde::Deserialize;
use std::path::Path;

/// CLI — only the config file path.
#[derive(Parser, Debug)]
#[command(name = "peerman", about = "DN42 Peer Management Web Application")]
pub struct Cli {
    /// Path to TOML configuration file
    #[arg(short = 'c', long = "config", default_value = "config.toml")]
    pub config: std::path::PathBuf,
}

// ---------------------------------------------------------------------------
// TOML config sections
// ---------------------------------------------------------------------------

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub logging: LoggingConfig,
    pub cluster: ClusterConfig,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct ServerConfig {
    pub listen_addr: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct StorageConfig {
    pub db_path: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct ClusterConfig {
    pub node_name: String,
    #[serde(default)]
    pub bootstrap_nodes: Vec<String>,
    pub probe_interval_secs: u64,
    pub sync_interval_secs: u64,
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:3000".into(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            db_path: "data/peerman.db".into(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
        }
    }
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            node_name: String::new(),
            bootstrap_nodes: Vec::new(),
            probe_interval_secs: 60,
            sync_interval_secs: 30,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            storage: StorageConfig::default(),
            logging: LoggingConfig::default(),
            cluster: ClusterConfig::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Load
// ---------------------------------------------------------------------------

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read config file {:?}: {}", path, e))?;
        let cfg: Config = toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse config file {:?}: {}", path, e))?;
        Ok(cfg)
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check 2>&1`
Expected: `src/config.rs` compiles successfully. `src/main.rs` will have errors because we changed the Config API.

---

### Task 3: Update `src/main.rs` field paths

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Replace `Config::parse()` with `Cli::parse()` + `Config::load()`**

Old (line 31):
```rust
let cfg = config::Config::parse();
```

New:
```rust
let cli = config::Cli::parse();
let cfg = config::Config::load(&cli.config)?;
```

- [ ] **Step 2: Update `log_level` → `logging.level` (line 34)**

Old:
```rust
.with_env_filter(EnvFilter::new(&cfg.log_level))
```

New:
```rust
.with_env_filter(EnvFilter::new(&cfg.logging.level))
```

- [ ] **Step 3: Update `listen_addr` → `server.listen_addr` (line 37)**

Old:
```rust
tracing::info!("Starting peerman, listening on {}", cfg.listen_addr);
```

New:
```rust
tracing::info!("Starting peerman, listening on {}", cfg.server.listen_addr);
```

- [ ] **Step 4: Update `db_path` → `storage.db_path` (line 40)**

Old:
```rust
let pool = db::create_pool(&cfg.db_path).await?;
```

New:
```rust
let pool = db::create_pool(&cfg.storage.db_path).await?;
```

- [ ] **Step 5: Update `node_name` references (lines 66, 91, 95, 99, 141, 175)**

All occurrences of `cfg.node_name` → `cfg.cluster.node_name`.

Line 66:
```rust
node_name: cfg.cluster.node_name.clone(),
```

Line 91:
```rust
if !cfg.cluster.node_name.is_empty() {
```

Line 95:
```rust
.upsert_self(&cfg.cluster.node_name, &cfg.server.listen_addr, local_asn)
```

Line 99:
```rust
cfg.cluster.node_name,
```

Line 141:
```rust
let probe_node_name = cfg.cluster.node_name.clone();
```

Line 175:
```rust
let node_name = cfg.cluster.node_name.clone();
```

- [ ] **Step 6: Update `cluster_nodes` → `cluster.bootstrap_nodes` (lines 105-106)**

Old:
```rust
if !cfg.cluster_nodes.is_empty() {
    for addr in cfg.cluster_nodes.split(',') {
```

New:
```rust
for addr in &cfg.cluster.bootstrap_nodes {
```

- [ ] **Step 7: Remove the closing brace of the old `if !cfg.cluster_nodes…` block**

The old block had a closing `}` after the loop. Remove it. The inner loop body remains unchanged.

Old structure:
```rust
        if !cfg.cluster_nodes.is_empty() {
            for addr in cfg.cluster_nodes.split(',') {
                let addr = addr.trim();
                if addr.is_empty() {
                    continue;
                }
                if state.node_repo.find_by_listen_addr(addr).await?.is_none() {
                    let name = format!("node-{}", addr.replace([':', '.'], "-"));
                    match state
                        .node_repo
                        .create(&name, addr, 0, "bootstrap node")
                        .await
                    {
                        Ok(n) => tracing::info!("Added bootstrap node: {} ({})", name, n.id),
                        Err(e) => tracing::warn!("Failed to add bootstrap node {}: {}", addr, e),
                    }
                }
            }
        }
```

New structure:
```rust
        for addr in &cfg.cluster.bootstrap_nodes {
            let addr = addr.trim();
            if addr.is_empty() {
                continue;
            }
            if state.node_repo.find_by_listen_addr(addr).await?.is_none() {
                let name = format!("node-{}", addr.replace([':', '.'], "-"));
                match state
                    .node_repo
                    .create(&name, addr, 0, "bootstrap node")
                    .await
                {
                    Ok(n) => tracing::info!("Added bootstrap node: {} ({})", name, n.id),
                    Err(e) => tracing::warn!("Failed to add bootstrap node {}: {}", addr, e),
                }
            }
        }
```

- [ ] **Step 8: Update `sync_interval_secs` → `cluster.sync_interval_secs` (line 127)**

Old:
```rust
let stale_interval = cfg.sync_interval_secs;
```

New:
```rust
let stale_interval = cfg.cluster.sync_interval_secs;
```

- [ ] **Step 9: Update `probe_interval_secs` → `cluster.probe_interval_secs` (lines 139, 142)**

Old:
```rust
if cfg.probe_interval_secs > 0 {
    let probe_state = state.clone();
    let probe_node_name = cfg.node_name.clone();
    let probe_interval = cfg.probe_interval_secs;
```

New:
```rust
if cfg.cluster.probe_interval_secs > 0 {
    let probe_state = state.clone();
    let probe_node_name = cfg.cluster.node_name.clone();
    let probe_interval = cfg.cluster.probe_interval_secs;
```

- [ ] **Step 10: Update `listen_addr` parse at line 209**

Old:
```rust
let addr: SocketAddr = cfg.listen_addr.parse()?;
```

New:
```rust
let addr: SocketAddr = cfg.server.listen_addr.parse()?;
```

- [ ] **Step 11: Verify full build**

Run: `cargo build 2>&1`
Expected: Build succeeds.

---

### Task 4: Create example config file

**Files:**
- Create: `config.toml.example`

- [ ] **Step 1: Write `config.toml.example`**

```toml
# Peerman configuration
# Copy this file to config.toml and edit as needed.

[server]
# Address to listen on
listen_addr = "0.0.0.0:3000"

[storage]
# Path to SQLite database
db_path = "data/peerman.db"

[logging]
# trace, debug, info, warn, error
level = "info"

[cluster]
# Node name — leave empty for standalone mode (no cluster features)
node_name = ""
# Bootstrap cluster nodes
bootstrap_nodes = []
# ICMP probe interval in seconds (0 = disabled)
probe_interval_secs = 60
# Stale-node sync interval in seconds
sync_interval_secs = 30
```

- [ ] **Step 2: Commit all changes**

```bash
git add Cargo.toml src/config.rs src/main.rs config.toml.example docs/
git commit -m "$(cat <<'EOF'
feat: migrate CLI flags to TOML config file (-c config.toml)

Replace 7 individual CLI flags with a single -c/--config argument backed
by a TOML configuration file. Config is organized into [server], [storage],
[logging], and [cluster] sections with sensible defaults.

Generated with [Claude Code](https://claude.ai/code)
via [Happy](https://happy.engineering)

Co-Authored-By: Claude <noreply@anthropic.com>
Co-Authored-By: Happy <yesreply@happy.engineering>
EOF
)"
```

Expected: Commit succeeds.

---

### Task 5: Verify end-to-end

- [ ] **Step 1: Verify --help shows only -c**

Run: `cargo run -- --help 2>&1`
Expected: Output shows only `-c, --config <CONFIG>` option (plus auto-generated `-h, --help` and `-V, --version`). No old flags like `--listen-addr`, `--db-path`, etc.

- [ ] **Step 2: Verify startup fails with missing config**

Run: `cargo run -- --db-path /tmp/peerman-test.db 2>&1`
Expected: Error — unrecognized option `--db-path`, or the old flag is rejected.

- [ ] **Step 3: Verify startup with valid config**

Run:
```bash
cat > /tmp/test-config.toml <<EOF
[storage]
db_path = "/tmp/peerman-test.db"
EOF
cargo run -- -c /tmp/test-config.toml 2>&1 &
sleep 2
curl -s http://localhost:3000/ | head -c 100
kill %1 2>/dev/null
```

Expected: Server starts with custom db_path, UI is served at localhost:3000.

- [ ] **Step 4: Verify defaults work with minimal config**

Run:
```bash
cat > /tmp/test-defaults.toml <<EOF
[server]
listen_addr = "127.0.0.1:3001"
[storage]
db_path = "/tmp/peerman-defaults.db"
EOF
cargo run -- -c /tmp/test-defaults.toml 2>&1 &
sleep 2
curl -s http://127.0.0.1:3001/ | head -c 100
kill %1 2>/dev/null
```

Expected: Server starts on 127.0.0.1:3001 with logging at default "info" level.
```

Expected: Server starts successfully, UI served at 127.0.0.1:3001.
