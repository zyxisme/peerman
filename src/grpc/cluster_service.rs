use tonic::{Request, Response, Status};

use super::generated::{
    cluster_service_server::ClusterService, CommunityRule, DeleteCommunityRuleRequest,
    DeleteCommunityRuleResponse, DeleteNodeRequest, DeleteNodeResponse, ExchangeNodesRequest,
    ExchangeNodesResponse, GetPeerCommunitiesRequest, GetPeerCommunitiesResponse,
    HealthCheckRequest, HealthCheckResponse, ListCommunityRulesRequest, ListCommunityRulesResponse,
    ListNodesRequest, ListNodesResponse, ListProbeResultsRequest, ListProbeResultsResponse, Node,
    NodeInfo, ProbeResult, PullPeersRequest, PullPeersResponse, PushPeerRequest, PushPeerResponse,
    PushProbeResultRequest, PushProbeResultResponse, RegisterNodeRequest, RunProbeRequest,
    RunProbeResponse, SaveCommunityRuleRequest, UpdateNodeRequest,
};

use crate::cluster::auth::check_cluster_key;
use crate::models::community::CommunityRuleRepository;
use crate::models::node::NodeRepository;
use crate::models::peer::PeerRepository;
use crate::models::probe::ProbeResultRepository;
use crate::services;

pub struct ClusterServiceImpl {
    pub node_repo: NodeRepository,
    pub peer_repo: PeerRepository,
    pub probe_repo: ProbeResultRepository,
    pub community_repo: CommunityRuleRepository,
    pub jwt_secret: std::sync::Arc<String>,
    pub cluster_key: std::sync::Arc<String>,
    pub listen_addr: String,
}

fn node_to_proto(n: &crate::models::node::Node) -> Node {
    Node {
        id: n.id.clone(),
        name: n.name.clone(),
        listen_addr: n.listen_addr.clone(),
        local_asn: n.local_asn,
        description: n.description.clone().unwrap_or_default(),
        online: n.online,
        last_seen_at: n.last_seen_at.clone(),
        created_at: n.created_at.clone(),
        updated_at: n.updated_at.clone(),
    }
}

fn probe_result_to_proto(r: &crate::models::probe::ProbeResult) -> ProbeResult {
    ProbeResult {
        id: r.id.clone(),
        from_node_id: r.from_node_id.clone(),
        to_node_id: r.to_node_id.clone(),
        avg_latency_ms: r.avg_latency_ms,
        min_latency_ms: r.min_latency_ms,
        max_latency_ms: r.max_latency_ms,
        packet_loss_pct: r.packet_loss_pct,
        packets_sent: r.packets_sent,
        packets_received: r.packets_received,
        probed_at: r.probed_at.clone(),
    }
}

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

#[tonic::async_trait]
impl ClusterService for ClusterServiceImpl {
    async fn list_nodes(
        &self,
        _request: Request<ListNodesRequest>,
    ) -> Result<Response<ListNodesResponse>, Status> {
        let nodes = self
            .node_repo
            .list_all()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ListNodesResponse {
            nodes: nodes.iter().map(node_to_proto).collect(),
        }))
    }

    async fn register_node(
        &self,
        request: Request<RegisterNodeRequest>,
    ) -> Result<Response<Node>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        let node = self
            .node_repo
            .create(&req.name, &req.listen_addr, req.local_asn, &req.description)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(node_to_proto(&node)))
    }

    async fn update_node(
        &self,
        request: Request<UpdateNodeRequest>,
    ) -> Result<Response<Node>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        let mut node = self
            .node_repo
            .find_by_id(&req.id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;

        node.name = req.name;
        node.listen_addr = req.listen_addr;
        node.local_asn = req.local_asn;
        node.description = if req.description.is_empty() {
            None
        } else {
            Some(req.description)
        };

        let node = self
            .node_repo
            .update(&node)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(node_to_proto(&node)))
    }

    async fn delete_node(
        &self,
        request: Request<DeleteNodeRequest>,
    ) -> Result<Response<DeleteNodeResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        self.node_repo
            .delete(&req.id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;

        Ok(Response::new(DeleteNodeResponse {}))
    }

    async fn push_peer(
        &self,
        request: Request<PushPeerRequest>,
    ) -> Result<Response<PushPeerResponse>, Status> {
        let jwt_ok = crate::auth::check_auth(&request, self.jwt_secret.as_ref()).is_ok();
        let cluster_ok = check_cluster_key(&request, &self.cluster_key).is_ok();
        if !jwt_ok && !cluster_ok {
            return Err(Status::unauthenticated("auth required"));
        }
        let req = request.into_inner();
        let proto_peer = req
            .peer
            .ok_or_else(|| Status::invalid_argument("peer is required"))?;

        // Upsert: insert or update based on updated_at
        let existing = self.peer_repo.find_by_id(&proto_peer.id).await.ok();

        match existing {
            Some(local) if proto_peer.updated_at > local.updated_at => {
                let mut updated = local;
                updated.apply_proto(&proto_peer);
                self.peer_repo
                    .update(&updated)
                    .await
                    .map_err(|e| Status::internal(e.to_string()))?;
            }
            None => {
                let mut peer = self
                    .peer_repo
                    .create(&proto_peer.name)
                    .await
                    .map_err(|e| Status::internal(e.to_string()))?;
                peer.apply_proto(&proto_peer);
                self.peer_repo
                    .update(&peer)
                    .await
                    .map_err(|e| Status::internal(e.to_string()))?;
            }
            _ => {} // remote is older or same — skip
        }

        Ok(Response::new(PushPeerResponse {}))
    }

    async fn pull_peers(
        &self,
        _request: Request<PullPeersRequest>,
    ) -> Result<Response<PullPeersResponse>, Status> {
        let peers = self
            .peer_repo
            .list_all()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(PullPeersResponse {
            peers: peers.iter().map(super::peer_service::peer_to_proto).collect(),
        }))
    }

    async fn push_probe_result(
        &self,
        request: Request<PushProbeResultRequest>,
    ) -> Result<Response<PushProbeResultResponse>, Status> {
        let jwt_ok = crate::auth::check_auth(&request, self.jwt_secret.as_ref()).is_ok();
        let cluster_ok = check_cluster_key(&request, &self.cluster_key).is_ok();
        if !jwt_ok && !cluster_ok {
            return Err(Status::unauthenticated("auth required"));
        }
        let req = request.into_inner();
        let proto = req
            .result
            .ok_or_else(|| Status::invalid_argument("result is required"))?;

        let result = crate::models::probe::ProbeResult {
            id: proto.id,
            from_node_id: proto.from_node_id,
            to_node_id: proto.to_node_id,
            avg_latency_ms: proto.avg_latency_ms,
            min_latency_ms: proto.min_latency_ms,
            max_latency_ms: proto.max_latency_ms,
            packet_loss_pct: proto.packet_loss_pct,
            packets_sent: proto.packets_sent,
            packets_received: proto.packets_received,
            probed_at: proto.probed_at,
        };

        self.probe_repo
            .insert(&result)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(PushProbeResultResponse {}))
    }

    async fn list_probe_results(
        &self,
        request: Request<ListProbeResultsRequest>,
    ) -> Result<Response<ListProbeResultsResponse>, Status> {
        let req = request.into_inner();
        let results = self
            .probe_repo
            .list_by_filters(&req.from_node_id, &req.to_node_id, req.limit)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ListProbeResultsResponse {
            results: results.iter().map(probe_result_to_proto).collect(),
        }))
    }

    async fn run_probe(
        &self,
        request: Request<RunProbeRequest>,
    ) -> Result<Response<RunProbeResponse>, Status> {
        let req = request.into_inner();

        let from_node = self
            .node_repo
            .find_by_id(&req.from_node_id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;

        let to_node = self
            .node_repo
            .find_by_id(&req.to_node_id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;

        let result = services::probe::probe_between(&from_node, &to_node, &self.probe_repo)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(RunProbeResponse {
            result: Some(probe_result_to_proto(&result)),
        }))
    }

    async fn list_community_rules(
        &self,
        _request: Request<ListCommunityRulesRequest>,
    ) -> Result<Response<ListCommunityRulesResponse>, Status> {
        let rules = self
            .community_repo
            .list_all()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ListCommunityRulesResponse {
            rules: rules.iter().map(community_rule_to_proto).collect(),
        }))
    }

    async fn save_community_rule(
        &self,
        request: Request<SaveCommunityRuleRequest>,
    ) -> Result<Response<CommunityRule>, Status> {
        let jwt_ok = crate::auth::check_auth(&request, self.jwt_secret.as_ref()).is_ok();
        let cluster_ok = check_cluster_key(&request, &self.cluster_key).is_ok();
        if !jwt_ok && !cluster_ok {
            return Err(Status::unauthenticated("auth required"));
        }
        let req = request.into_inner();
        let proto = req
            .rule
            .ok_or_else(|| Status::invalid_argument("rule is required"))?;

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
        };

        let saved = self
            .community_repo
            .save(&rule)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(community_rule_to_proto(&saved)))
    }

    async fn delete_community_rule(
        &self,
        request: Request<DeleteCommunityRuleRequest>,
    ) -> Result<Response<DeleteCommunityRuleResponse>, Status> {
        let jwt_ok = crate::auth::check_auth(&request, self.jwt_secret.as_ref()).is_ok();
        let cluster_ok = check_cluster_key(&request, &self.cluster_key).is_ok();
        if !jwt_ok && !cluster_ok {
            return Err(Status::unauthenticated("auth required"));
        }
        let req = request.into_inner();
        self.community_repo
            .delete(&req.id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;

        Ok(Response::new(DeleteCommunityRuleResponse {}))
    }

    async fn get_peer_communities(
        &self,
        request: Request<GetPeerCommunitiesRequest>,
    ) -> Result<Response<GetPeerCommunitiesResponse>, Status> {
        let req = request.into_inner();
        let peer = self
            .peer_repo
            .find_by_id(&req.peer_id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;

        let local_node_id = self
            .node_repo
            .list_all()
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .first()
            .map(|n| n.id.clone())
            .unwrap_or_default();

        let (v4, v6) = services::community_mapper::CommunityMapper::compute_communities(
            &peer,
            &local_node_id,
            &self.probe_repo,
            &self.community_repo,
        )
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(GetPeerCommunitiesResponse {
            community_ipv4: v4,
            community_ipv6: v6,
        }))
    }

    async fn exchange_nodes(
        &self,
        request: Request<ExchangeNodesRequest>,
    ) -> Result<Response<ExchangeNodesResponse>, Status> {
        check_cluster_key(&request, &self.cluster_key)?;
        let req = request.into_inner();

        // Upsert nodes received from peer
        for ni in &req.nodes {
            if let Err(e) = self
                .node_repo
                .upsert_by_name(&ni.name, &ni.listen_addr, ni.local_asn, &ni.description)
                .await
            {
                tracing::warn!("Failed to upsert node {} from exchange: {}", ni.name, e);
            }
        }

        // Return our known nodes
        let nodes = self
            .node_repo
            .list_all()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let node_infos: Vec<NodeInfo> = nodes
            .iter()
            .map(|n| NodeInfo {
                name: n.name.clone(),
                listen_addr: n.listen_addr.clone(),
                local_asn: n.local_asn,
                description: n.description.clone().unwrap_or_default(),
                last_seen_at: n.last_seen_at.clone(),
                wg_public_key: String::new(),
                tunnel_ip: String::new(),
            })
            .collect();

        Ok(Response::new(ExchangeNodesResponse {
            nodes: node_infos,
        }))
    }

    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        Ok(Response::new(HealthCheckResponse { ok: true }))
    }
}
