use std::sync::LazyLock;
use std::time::Duration;

use dashmap::DashMap;
use tokio::time::timeout;
use tonic::transport::Endpoint;
use tonic::Request;

use crate::cluster::cache::ClusterCache;
use crate::models::node::Node;

use crate::grpc::generated::{
    cluster_service_client::ClusterServiceClient, CommunityRule, ExchangeNodesRequest,
    HealthCheckRequest, ListCommunityRulesRequest, ListProbeResultsRequest, NodeInfo, Peer,
    ProbeResult, PullPeersRequest,
};

const FANOUT_TIMEOUT: Duration = Duration::from_secs(2);

/// Connection pool: cached gRPC clients keyed by address.
static CHANNEL_POOL: LazyLock<DashMap<String, ClusterServiceClient<tonic::transport::Channel>>> =
    LazyLock::new(DashMap::new);

#[allow(dead_code)]
pub struct ClusterAggregator {
    pub cache: ClusterCache,
    pub cluster_key: String,
}

#[allow(dead_code)]
impl ClusterAggregator {
    pub fn new(cache: ClusterCache, cluster_key: String) -> Self {
        Self { cache, cluster_key }
    }

    async fn connect(
        addr: &str,
    ) -> Result<ClusterServiceClient<tonic::transport::Channel>, String> {
        if let Some(client) = CHANNEL_POOL.get(addr) {
            return Ok(client.clone());
        }
        let uri = format!("http://{}", addr);
        let channel = Endpoint::from_shared(uri)
            .map_err(|e| format!("invalid uri: {e}"))?
            .connect()
            .await
            .map_err(|e| format!("connect failed: {e}"))?;
        let client = ClusterServiceClient::new(channel);
        CHANNEL_POOL.insert(addr.to_string(), client.clone());
        Ok(client)
    }

    fn set_cluster_key<T>(&self, req: &mut Request<T>) {
        if !self.cluster_key.is_empty() {
            if let Ok(val) = self.cluster_key.parse() {
                req.metadata_mut().insert("x-cluster-key", val);
            }
        }
    }

    /// Execute an RPC on a single node with timeout and cache fallback.
    /// Handles the "established connection → timeout → cache fallback" pattern.
    /// Returns (items, node_status) tuple.
    async fn call_node<T: Clone>(
        node_name: &str,
        node_addr: &str,
        cache: &ClusterCache,
        rpc_call: impl std::future::Future<Output = Result<Vec<T>, String>>,
        cache_fallback: impl std::future::Future<Output = Option<Vec<T>>>,
    ) -> (Vec<T>, Vec<NodeStatus>) {
        match timeout(FANOUT_TIMEOUT, rpc_call).await {
            Ok(Ok(items)) => (items, vec![NodeStatus::online(node_name, node_addr)]),
            _ => {
                let cached = cache_fallback.await;
                cache.mark_stale(node_addr).await;
                let items = cached.unwrap_or_default();
                let status = if items.is_empty() {
                    NodeStatus::unknown(node_name, node_addr, "fanout timeout")
                } else {
                    NodeStatus::offline(node_name, node_addr, "stale")
                };
                (items, vec![status])
            }
        }
    }

    /// Fan-out PullPeers to all online nodes in parallel, return merged peers + per-node status.
    /// Updates cache for successful responses; falls back to cache for failed nodes.
    pub async fn fanout_peers(
        &self,
        local_addr: &str,
        online_nodes: &[Node],
    ) -> AggregatedResult<Peer> {
        let futures: Vec<_> = online_nodes
            .iter()
            .filter(|n| n.listen_addr != local_addr)
            .map(|node| {
                let node_addr = node.listen_addr.clone();
                let node_name = node.name.clone();
                let cache = self.cache.clone();
                let cluster_key = self.cluster_key.clone();
                async move {
                    let mut client = match Self::connect(&node_addr).await {
                        Ok(c) => c,
                        Err(e) => {
                            let cached = cache.get(&node_addr).await;
                            cache.mark_stale(&node_addr).await;
                            return (
                                cached.map(|c| c.peers).unwrap_or_default(),
                                vec![NodeStatus::unknown(&node_name, &node_addr, &e)],
                            );
                        }
                    };

                    let mut req = Request::new(PullPeersRequest {
                        since: String::new(),
                    });
                    if !cluster_key.is_empty() {
                        if let Ok(val) = cluster_key.parse() {
                            req.metadata_mut().insert("x-cluster-key", val);
                        }
                    }

                    let (items, statuses) = Self::call_node(
                        &node_name,
                        &node_addr,
                        &cache,
                        async {
                            let resp = client
                                .pull_peers(req)
                                .await
                                .map_err(|e| format!("rpc: {e}"))?;
                            Ok(resp.into_inner().peers)
                        },
                        async { cache.get(&node_addr).await.map(|c| c.peers) },
                    )
                    .await;
                    if !items.is_empty() {
                        cache.update_peers(&node_addr, items.clone()).await;
                    }
                    (items, statuses)
                }
            })
            .collect();

        let results = futures::future::join_all(futures).await;
        let mut all = Vec::new();
        let mut statuses = Vec::new();
        for (items, node_statuses) in results {
            all.extend(items);
            statuses.extend(node_statuses);
        }

        AggregatedResult {
            items: all,
            node_statuses: statuses,
        }
    }

    /// Fan-out ListProbeResults to all online nodes in parallel.
    pub async fn fanout_probe_results(
        &self,
        local_addr: &str,
        online_nodes: &[Node],
    ) -> AggregatedResult<ProbeResult> {
        let futures: Vec<_> = online_nodes
            .iter()
            .filter(|n| n.listen_addr != local_addr)
            .map(|node| {
                let node_addr = node.listen_addr.clone();
                let node_name = node.name.clone();
                let cache = self.cache.clone();
                let cluster_key = self.cluster_key.clone();
                async move {
                    let mut client = match Self::connect(&node_addr).await {
                        Ok(c) => c,
                        Err(e) => {
                            let cached = cache.get(&node_addr).await;
                            cache.mark_stale(&node_addr).await;
                            return (
                                cached.map(|c| c.probe_results).unwrap_or_default(),
                                vec![NodeStatus::unknown(&node_name, &node_addr, &e)],
                            );
                        }
                    };

                    let mut req = Request::new(ListProbeResultsRequest {
                        from_node_id: String::new(),
                        to_node_id: String::new(),
                        limit: 0,
                    });
                    if !cluster_key.is_empty() {
                        if let Ok(val) = cluster_key.parse() {
                            req.metadata_mut().insert("x-cluster-key", val);
                        }
                    }

                    let (items, statuses) = Self::call_node(
                        &node_name,
                        &node_addr,
                        &cache,
                        async {
                            let resp = client
                                .list_probe_results(req)
                                .await
                                .map_err(|e| format!("rpc: {e}"))?;
                            Ok(resp.into_inner().results)
                        },
                        async { cache.get(&node_addr).await.map(|c| c.probe_results) },
                    )
                    .await;
                    if !items.is_empty() {
                        cache.update_probe_results(&node_addr, items.clone()).await;
                    }
                    (items, statuses)
                }
            })
            .collect();

        let results = futures::future::join_all(futures).await;
        let mut all = Vec::new();
        let mut statuses = Vec::new();
        for (items, node_statuses) in results {
            all.extend(items);
            statuses.extend(node_statuses);
        }

        AggregatedResult {
            items: all,
            node_statuses: statuses,
        }
    }

    /// Fan-out ListCommunityRules to all online nodes in parallel.
    pub async fn fanout_community_rules(
        &self,
        local_addr: &str,
        online_nodes: &[Node],
    ) -> AggregatedResult<CommunityRule> {
        let futures: Vec<_> = online_nodes
            .iter()
            .filter(|n| n.listen_addr != local_addr)
            .map(|node| {
                let node_addr = node.listen_addr.clone();
                let node_name = node.name.clone();
                let cache = self.cache.clone();
                let cluster_key = self.cluster_key.clone();
                async move {
                    let mut client = match Self::connect(&node_addr).await {
                        Ok(c) => c,
                        Err(e) => {
                            let cached = cache.get(&node_addr).await;
                            cache.mark_stale(&node_addr).await;
                            return (
                                cached.map(|c| c.community_rules).unwrap_or_default(),
                                vec![NodeStatus::unknown(&node_name, &node_addr, &e)],
                            );
                        }
                    };

                    let mut req = Request::new(ListCommunityRulesRequest {});
                    if !cluster_key.is_empty() {
                        if let Ok(val) = cluster_key.parse() {
                            req.metadata_mut().insert("x-cluster-key", val);
                        }
                    }

                    let (items, statuses) = Self::call_node(
                        &node_name,
                        &node_addr,
                        &cache,
                        async {
                            let resp = client
                                .list_community_rules(req)
                                .await
                                .map_err(|e| format!("rpc: {e}"))?;
                            Ok(resp.into_inner().rules)
                        },
                        async { cache.get(&node_addr).await.map(|c| c.community_rules) },
                    )
                    .await;
                    if !items.is_empty() {
                        cache
                            .update_community_rules(&node_addr, items.clone())
                            .await;
                    }
                    (items, statuses)
                }
            })
            .collect();

        let results = futures::future::join_all(futures).await;
        let mut all = Vec::new();
        let mut statuses = Vec::new();
        for (items, node_statuses) in results {
            all.extend(items);
            statuses.extend(node_statuses);
        }

        AggregatedResult {
            items: all,
            node_statuses: statuses,
        }
    }

    /// Call HealthCheck on a single node. Returns true if healthy.
    pub async fn health_check(node_addr: &str, cluster_key: &str) -> bool {
        let mut client = match Self::connect(node_addr).await {
            Ok(c) => c,
            Err(_) => return false,
        };
        let mut req = Request::new(HealthCheckRequest {});
        if !cluster_key.is_empty() {
            if let Ok(val) = cluster_key.parse() {
                req.metadata_mut().insert("x-cluster-key", val);
            }
        }
        timeout(FANOUT_TIMEOUT, client.health_check(req))
            .await
            .map(|r| r.is_ok_and(|resp| resp.into_inner().ok))
            .unwrap_or(false)
    }

    /// Execute a BIRD command on a specific remote node via BirdService RPC.
    pub async fn execute_bird_command(
        &self,
        node_addr: &str,
        command: &str,
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
        self.set_cluster_key(&mut req);

        let response = timeout(FANOUT_TIMEOUT, client.execute_command(req))
            .await
            .map_err(|_| "timeout".to_string())?
            .map_err(|e| format!("rpc: {e}"))?;

        let results = response.into_inner().results;
        results
            .first()
            .filter(|r| r.status_code == 0)
            .map(|r| r.output.clone())
            .ok_or_else(|| {
                results
                    .first()
                    .map(|r| r.error.clone())
                    .unwrap_or_else(|| "no result".to_string())
            })
    }

    /// Exchange node list with a peer. Returns the peer's node list.
    pub async fn exchange_with(
        node_addr: &str,
        cluster_key: &str,
        my_nodes: Vec<NodeInfo>,
    ) -> Result<Vec<NodeInfo>, String> {
        let mut client = Self::connect(node_addr)
            .await
            .map_err(|e| format!("connect: {e}"))?;
        let mut req = Request::new(ExchangeNodesRequest { nodes: my_nodes });
        if !cluster_key.is_empty() {
            if let Ok(val) = cluster_key.parse() {
                req.metadata_mut().insert("x-cluster-key", val);
            }
        }
        let response = timeout(FANOUT_TIMEOUT, client.exchange_nodes(req))
            .await
            .map_err(|_| "timeout".to_string())?
            .map_err(|e| format!("rpc: {e}"))?;
        Ok(response.into_inner().nodes)
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct NodeStatus {
    pub node_name: String,
    pub node_addr: String,
    pub online: bool,
    pub staleness: String,
    pub error: Option<String>,
}

#[allow(dead_code)]
impl NodeStatus {
    pub fn online(name: &str, addr: &str) -> Self {
        Self {
            node_name: name.into(),
            node_addr: addr.into(),
            online: true,
            staleness: "fresh".into(),
            error: None,
        }
    }
    pub fn offline(name: &str, addr: &str, staleness: &str) -> Self {
        Self {
            node_name: name.into(),
            node_addr: addr.into(),
            online: false,
            staleness: staleness.into(),
            error: None,
        }
    }
    pub fn unknown(name: &str, addr: &str, err: &str) -> Self {
        Self {
            node_name: name.into(),
            node_addr: addr.into(),
            online: false,
            staleness: "unknown".into(),
            error: Some(err.into()),
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct AggregatedResult<T> {
    pub items: Vec<T>,
    pub node_statuses: Vec<NodeStatus>,
}
