use clap::Parser;
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::Path;
use tracing_subscriber::EnvFilter;

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
#[derive(Default)]
pub struct Config {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub logging: LoggingConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    pub cluster: ClusterConfig,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct ServerConfig {
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct StorageConfig {
    #[serde(default = "default_db_path")]
    pub db_path: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
}

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
    pub tunnel_ip_range: String,
    #[serde(default)]
    pub tunnel_ipv6_range: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct AuthConfig {
    #[serde(default = "default_username")]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub password_hash: String,
    #[serde(default)]
    pub jwt_secret: String,
}

// ---------------------------------------------------------------------------
// Default value functions (for serde field-level defaults)
// ---------------------------------------------------------------------------

fn default_listen_addr() -> String {
    "0.0.0.0:3000".into()
}

fn default_db_path() -> String {
    "data/peerman.db".into()
}

fn default_log_level() -> String {
    "info".into()
}

fn default_probe_interval() -> u64 {
    60
}

fn default_sync_interval() -> u64 {
    30
}

fn default_username() -> String {
    "admin".into()
}

// ---------------------------------------------------------------------------
// Default impls (for when entire sections are absent from TOML)
// ---------------------------------------------------------------------------

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: default_listen_addr(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            db_path: default_db_path(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
        }
    }
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            node_name: String::new(),
            cluster_key: String::new(),
            peer_nodes: Vec::new(),
            probe_interval_secs: default_probe_interval(),
            sync_interval_secs: default_sync_interval(),
            tunnel_ip_range: String::new(),
            tunnel_ipv6_range: String::new(),
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            username: default_username(),
            password: String::new(),
            password_hash: String::new(),
            jwt_secret: String::new(),
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

    pub fn validate(&self) -> anyhow::Result<()> {
        // Validate listen address is parseable
        self.server
            .listen_addr
            .parse::<SocketAddr>()
            .map_err(|e| {
                anyhow::anyhow!(
                    "Invalid server.listen_addr '{}': {}",
                    self.server.listen_addr,
                    e
                )
            })?;

        // Validate log level
        EnvFilter::try_new(&self.logging.level).map_err(|e| {
            anyhow::anyhow!(
                "Invalid logging.level '{}': {}",
                self.logging.level,
                e
            )
        })?;

        // Validate cluster config consistency
        if !self.cluster.node_name.is_empty() && self.cluster.cluster_key.is_empty() {
            tracing::warn!(
                "Cluster mode enabled (node_name set) but cluster_key is empty — inter-node auth disabled"
            );
        }

        // Validate tunnel IP range format if set
        if !self.cluster.tunnel_ip_range.is_empty() {
            if !self.cluster.tunnel_ip_range.contains('/') {
                anyhow::bail!(
                    "Invalid cluster.tunnel_ip_range '{}': must be CIDR format (e.g., 10.255.0.0/24)",
                    self.cluster.tunnel_ip_range
                );
            }
        }

        Ok(())
    }
}
