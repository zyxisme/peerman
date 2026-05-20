# TOML Config File Migration

## Goal

Replace 7 individual CLI flags with a single `-c config.toml` argument. All configuration moves into a TOML file.

## TOML File Structure

```toml
[server]
listen_addr = "0.0.0.0:3000"

[storage]
db_path = "data/peerman.db"

[logging]
level = "info"

[cluster]
node_name = ""
bootstrap_nodes = []
probe_interval_secs = 60
sync_interval_secs = 30
```

- `cluster.node_name = ""` means standalone mode (no cluster features)
- `cluster.bootstrap_nodes` is a TOML array: `["host1:port", "host2:port"]`
- All fields have defaults; users only write what they override
- Mininal config (all defaults): an empty or nearly-empty `config.toml`

## CLI

- `clap` retains a single argument: `-c` / `--config`
- Default path when omitted: `./config.toml`
- `--help` and `--version` come free from clap

```
peerman                      # uses ./config.toml
peerman -c /etc/peerman.toml # explicit path
peerman --config my.toml     # long form
```

## Implementation Plan

### 1. `src/config.rs` — restructure

- `Cli` struct: only `-c`/`--config`, default `config.toml`
- `Config` and sub-section structs: `#[derive(Deserialize)]` + `#[serde(default)]`
- Each sub-struct implements `Default` for default values
- `Config::load(path: &Path) -> Result<Config>`: reads file → `toml::from_str`

### 2. `Cargo.toml` — add dependency

- Add `toml = "0.8"` to `[dependencies]`

### 3. `src/main.rs` — update field paths

| Old | New |
|-----|-----|
| `cfg.log_level` | `cfg.logging.level` |
| `cfg.listen_addr` | `cfg.server.listen_addr` |
| `cfg.db_path` | `cfg.storage.db_path` |
| `cfg.node_name` | `cfg.cluster.node_name` |
| `cfg.cluster_nodes` | `cfg.cluster.bootstrap_nodes` |
| `cfg.sync_interval_secs` | `cfg.cluster.sync_interval_secs` |
| `cfg.probe_interval_secs` | `cfg.cluster.probe_interval_secs` |

- Replace `Config::parse()` with `Cli::parse()` + `Config::load()`
- Replace `cfg.cluster_nodes.split(',')` loop with direct iteration over `Vec<String>`

### 4. `config.toml.example` — example config

Add a commented example config file to the repo root.

## Non-Goals

- Environment variable overrides
- Config hot-reload
- Backward compatibility with old CLI flags

## Build Verification

- `cargo build` succeeds
- `cargo run -- --help` shows only `-c`/`--config` option
- Server starts with defaults when no config file exists (verify error message)
- Server starts with a valid config.toml
