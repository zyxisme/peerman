use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "peerman", about = "DN42 Peer Management Web Application")]
pub struct Config {
    /// Address to listen on
    #[arg(long, default_value = "0.0.0.0:3000")]
    pub listen_addr: String,

    /// Path to SQLite database file
    #[arg(long, default_value = "data/peerman.db")]
    pub db_path: String,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// This node's name in the cluster (empty = standalone mode)
    #[arg(long, default_value = "")]
    pub node_name: String,

    /// Bootstrap cluster nodes (comma-separated "host:port")
    #[arg(long, default_value = "")]
    pub cluster_nodes: String,

    /// Probe interval in seconds (0 = disabled)
    #[arg(long, default_value = "60")]
    pub probe_interval_secs: u64,

    /// Sync interval in seconds
    #[arg(long, default_value = "30")]
    pub sync_interval_secs: u64,
}
