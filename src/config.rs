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
