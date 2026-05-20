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

    pub async fn update(
        &self,
        node_addr: &str,
        peers: Vec<Peer>,
        probe_results: Vec<ProbeResult>,
        community_rules: Vec<CommunityRule>,
    ) {
        let mut map = self.by_node.write().await;
        map.insert(
            node_addr.to_string(),
            NodeCacheEntry {
                peers,
                probe_results,
                community_rules,
                fetched_at: Instant::now(),
                stale: false,
            },
        );
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
