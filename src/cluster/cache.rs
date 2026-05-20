use std::collections::HashMap;
use std::time::Instant;

use crate::grpc::generated::{CommunityRule, Peer, ProbeResult};

#[derive(Clone, Debug)]
pub struct NodeCacheEntry {
    pub peers: Vec<Peer>,
    pub probe_results: Vec<ProbeResult>,
    pub community_rules: Vec<CommunityRule>,
    pub fetched_at: Instant,
    pub stale: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ClusterCache {
    by_node: std::sync::Arc<tokio::sync::RwLock<HashMap<String, NodeCacheEntry>>>,
}

impl ClusterCache {
    pub fn new() -> Self {
        Self {
            by_node: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    pub async fn update_peers(&self, node_addr: &str, peers: Vec<Peer>) {
        let mut map = self.by_node.write().await;
        let entry = map.entry(node_addr.to_string()).or_insert_with(|| NodeCacheEntry {
            peers: vec![],
            probe_results: vec![],
            community_rules: vec![],
            fetched_at: Instant::now(),
            stale: false,
        });
        entry.peers = peers;
        entry.fetched_at = Instant::now();
        entry.stale = false;
    }

    pub async fn update_probe_results(&self, node_addr: &str, results: Vec<ProbeResult>) {
        let mut map = self.by_node.write().await;
        let entry = map.entry(node_addr.to_string()).or_insert_with(|| NodeCacheEntry {
            peers: vec![],
            probe_results: vec![],
            community_rules: vec![],
            fetched_at: Instant::now(),
            stale: false,
        });
        entry.probe_results = results;
        entry.fetched_at = Instant::now();
        entry.stale = false;
    }

    pub async fn update_community_rules(&self, node_addr: &str, rules: Vec<CommunityRule>) {
        let mut map = self.by_node.write().await;
        let entry = map.entry(node_addr.to_string()).or_insert_with(|| NodeCacheEntry {
            peers: vec![],
            probe_results: vec![],
            community_rules: vec![],
            fetched_at: Instant::now(),
            stale: false,
        });
        entry.community_rules = rules;
        entry.fetched_at = Instant::now();
        entry.stale = false;
    }

    pub async fn get(&self, node_addr: &str) -> Option<NodeCacheEntry> {
        let map = self.by_node.read().await;
        map.get(node_addr).cloned()
    }

    pub async fn mark_stale(&self, node_addr: &str) {
        let mut map = self.by_node.write().await;
        if let Some(entry) = map.get_mut(node_addr) {
            entry.stale = true;
        }
    }

    pub async fn invalidate(&self, node_addr: &str) {
        let mut map = self.by_node.write().await;
        map.remove(node_addr);
    }
}
