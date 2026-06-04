use std::time::Duration;

use tokio::time::timeout;
use tonic::transport::Endpoint;
use tonic::Request;

use crate::cluster::cache::ClusterCache;
use crate::models::node::Node;

use crate::grpc::generated::{
    cluster_service_client::ClusterServiceClient,
    CommunityRule, ExchangeNodesRequest, HealthCheckRequest, HealthCheckResponse,
    ListCommunityRulesRequest, ListProbeResultsRequest, NodeInfo, Peer,
    ProbeResult, PullPeersRequest,
};

const FANOUT_TIMEOUT: Duration = Duration::from_secs(2);

pub struct ClusterAggregator {
    pub cache: ClusterCache,
    pub cluster_key: String,
}

impl ClusterAggregator {
    pub fn new(cache: ClusterCache, cluster_key: String) -> Self {
        Self { cache, cluster_key }
    }

    async fn connect(addr: &str) -> Result<ClusterServiceClient<tonic::transport::Channel>, String> {
        let uri = format!("http://{}", addr);
        let channel = Endpoint::from_shared(uri)
            .map_err(|e| format!("invalid uri: {e}"))?
            .connect()
            .await
            .map_err(|e| format!("connect failed: {e}"))?;
        Ok(ClusterServiceClient::new(channel))
    }

    fn set_cluster_key<T>(&self, req: &mut Request<T>) {
        if !self.cluster_key.is_empty() {
            if let Ok(val) = self.cluster_key.parse() {
                req.metadata_mut().insert("x-cluster-key", val);
            }
        }
    }

    /// Fan-out PullPeers to all online nodes, return merged peers + per-node status.
    /// Updates cache for successful responses; falls back to cache for failed nodes.
    pub async fn fanout_peers(
        &self,
        local_addr: &str,
        online_nodes: &[Node],
    ) -> AggregatedResult<Peer> {
        let mut all: Vec<Peer> = Vec::new();
        let mut statuses: Vec<NodeStatus> = Vec::new();

        for node in online_nodes {
            if node.listen_addr == local_addr {
                continue;
            }
            let node_addr = node.listen_addr.clone();
            let node_name = node.name.clone();

            let mut client = match Self::connect(&node_addr).await {
                Ok(c) => c,
                Err(e) => {
                    if let Some(entry) = self.cache.get(&node_addr).await {
                        all.extend(entry.peers);
                        statuses.push(NodeStatus::offline(&node_name, &node_addr, "stale"));
                    } else {
                        statuses.push(NodeStatus::unknown(&node_name, &node_addr, &e));
                    }
                    self.cache.mark_stale(&node_addr).await;
                    continue;
                }
            };

            let mut req = Request::new(PullPeersRequest {
                since: String::new(),
            });
            self.set_cluster_key(&mut req);

            match timeout(FANOUT_TIMEOUT, client.pull_peers(req)).await {
                Ok(Ok(response)) => {
                    let peers: Vec<Peer> = response.into_inner().peers;
                    self.cache.update_peers(&node_addr, peers.clone()).await;
                    all.extend(peers);
                    statuses.push(NodeStatus::online(&node_name, &node_addr));
                }
                _ => {
                    if let Some(entry) = self.cache.get(&node_addr).await {
                        all.extend(entry.peers);
                        statuses.push(NodeStatus::offline(&node_name, &node_addr, "stale"));
                    } else {
                        statuses.push(NodeStatus::unknown(
                            &node_name,
                            &node_addr,
                            "fanout timeout",
                        ));
                    }
                    self.cache.mark_stale(&node_addr).await;
                }
            }
        }

        AggregatedResult {
            items: all,
            node_statuses: statuses,
        }
    }

    /// Fan-out ListProbeResults to all online nodes.
    pub async fn fanout_probe_results(
        &self,
        local_addr: &str,
        online_nodes: &[Node],
    ) -> AggregatedResult<ProbeResult> {
        let mut all: Vec<ProbeResult> = Vec::new();
        let mut statuses: Vec<NodeStatus> = Vec::new();

        for node in online_nodes {
            if node.listen_addr == local_addr {
                continue;
            }
            let node_addr = node.listen_addr.clone();
            let node_name = node.name.clone();

            let mut client = match Self::connect(&node_addr).await {
                Ok(c) => c,
                Err(e) => {
                    if let Some(entry) = self.cache.get(&node_addr).await {
                        all.extend(entry.probe_results);
                        statuses.push(NodeStatus::offline(&node_name, &node_addr, "stale"));
                    } else {
                        statuses.push(NodeStatus::unknown(&node_name, &node_addr, &e));
                    }
                    self.cache.mark_stale(&node_addr).await;
                    continue;
                }
            };

            let mut req = Request::new(ListProbeResultsRequest {
                from_node_id: String::new(),
                to_node_id: String::new(),
                limit: 0,
            });
            self.set_cluster_key(&mut req);

            match timeout(FANOUT_TIMEOUT, client.list_probe_results(req)).await {
                Ok(Ok(response)) => {
                    let results: Vec<ProbeResult> = response.into_inner().results;
                    self.cache.update_probe_results(&node_addr, results.clone()).await;
                    all.extend(results);
                    statuses.push(NodeStatus::online(&node_name, &node_addr));
                }
                _ => {
                    let mut served = false;
                    if let Some(entry) = self.cache.get(&node_addr).await {
                        all.extend(entry.probe_results);
                        served = true;
                    }
                    statuses.push(if served {
                        NodeStatus::offline(&node_name, &node_addr, "stale")
                    } else {
                        NodeStatus::unknown(&node_name, &node_addr, "fanout timeout")
                    });
                    self.cache.mark_stale(&node_addr).await;
                }
            }
        }

        AggregatedResult {
            items: all,
            node_statuses: statuses,
        }
    }

    /// Fan-out ListCommunityRules to all online nodes.
    pub async fn fanout_community_rules(
        &self,
        local_addr: &str,
        online_nodes: &[Node],
    ) -> AggregatedResult<CommunityRule> {
        let mut all: Vec<CommunityRule> = Vec::new();
        let mut statuses: Vec<NodeStatus> = Vec::new();

        for node in online_nodes {
            if node.listen_addr == local_addr {
                continue;
            }
            let node_addr = node.listen_addr.clone();
            let node_name = node.name.clone();

            let mut client = match Self::connect(&node_addr).await {
                Ok(c) => c,
                Err(e) => {
                    if let Some(entry) = self.cache.get(&node_addr).await {
                        all.extend(entry.community_rules);
                        statuses.push(NodeStatus::offline(&node_name, &node_addr, "stale"));
                    } else {
                        statuses.push(NodeStatus::unknown(&node_name, &node_addr, &e));
                    }
                    self.cache.mark_stale(&node_addr).await;
                    continue;
                }
            };

            let mut req = Request::new(ListCommunityRulesRequest {});
            self.set_cluster_key(&mut req);

            match timeout(FANOUT_TIMEOUT, client.list_community_rules(req)).await {
                Ok(Ok(response)) => {
                    let rules: Vec<CommunityRule> = response.into_inner().rules;
                    self.cache.update_community_rules(&node_addr, rules.clone()).await;
                    all.extend(rules);
                    statuses.push(NodeStatus::online(&node_name, &node_addr));
                }
                _ => {
                    let mut served = false;
                    if let Some(entry) = self.cache.get(&node_addr).await {
                        all.extend(entry.community_rules);
                        served = true;
                    }
                    statuses.push(if served {
                        NodeStatus::offline(&node_name, &node_addr, "stale")
                    } else {
                        NodeStatus::unknown(&node_name, &node_addr, "fanout timeout")
                    });
                    self.cache.mark_stale(&node_addr).await;
                }
            }
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
        let mut client =
            Self::connect(node_addr).await.map_err(|e| format!("connect: {e}"))?;
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
pub struct NodeStatus {
    pub node_name: String,
    pub node_addr: String,
    pub online: bool,
    pub staleness: String,
    pub error: Option<String>,
}

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
pub struct AggregatedResult<T> {
    pub items: Vec<T>,
    pub node_statuses: Vec<NodeStatus>,
}
