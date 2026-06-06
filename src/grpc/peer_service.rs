use std::os::unix::fs::PermissionsExt;
use tonic::{Request, Response, Status};

use super::generated::{
    ConfigResponse, CreatePeerRequest, DeletePeerRequest, DeletePeerResponse, ExportAllRequest,
    GenerateKeypairRequest, GenerateKeypairResponse, GetConfigRequest, GetPeerRequest,
    ListPeersRequest, ListPeersResponse, Peer, PushPeerRequest, RestartWireGuardRequest,
    RestartWireGuardResponse, TogglePeerRequest, UpdatePeerRequest,
    peer_service_server::PeerService,
};

use crate::app_state::PeerState;
use crate::models::community::CommunityRuleRepository;
use crate::models::probe::ProbeResultRepository;
use crate::services;
use crate::services::community_mapper::CommunityMapper;

pub struct PeerServiceImpl {
    pub state: PeerState,
    pub jwt_secret: std::sync::Arc<String>,
    pub cluster_key: std::sync::Arc<String>,
    pub listen_addr: String,
    pub config_dirty: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// Apply WireGuard and BIRD configs from current DB state. Extracted from
/// `PeerServiceImpl::auto_apply_wg_bird` so it can be called from the
/// background debounce task without holding a reference to the service.
pub async fn apply_wg_bird(
    state: &PeerState,
    listen_addr: &str,
    pool: &sqlx::SqlitePool,
    apply_status: &crate::app_state::ApplyStatus,
) -> Result<(), Status> {
    // Prevent concurrent apply
    static APPLY_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();
    let _guard = APPLY_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;

    apply_status.pending.store(true, std::sync::atomic::Ordering::Relaxed);

    let peers = state
        .peer_repo
        .list_all()
        .await
        .map_err(|e| Status::internal(e.to_string()))?;
    let settings = state
        .settings_repo
        .load()
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    // Track managed interfaces for status reporting
    let interfaces: Vec<String> = peers
        .iter()
        .filter(|p| p.enabled)
        .map(|p| {
            if p.wg_interface_name.is_empty() {
                "wg0".to_string()
            } else {
                p.wg_interface_name.clone()
            }
        })
        .collect();
    *apply_status.managed_interfaces.lock().await = interfaces;

    // 1. WireGuard: per-peer interface apply
    for peer in peers.iter().filter(|p| p.enabled) {
        let iface = if peer.wg_interface_name.is_empty() {
            "wg0".to_string()
        } else {
            peer.wg_interface_name.clone()
        };
        let conf_path = format!("/etc/wireguard/{iface}.conf");
        let tmp_path = format!("{conf_path}.tmp");
        let config = crate::services::wireguard::generate_config(peer, &settings);

        std::fs::write(&tmp_path, &config)
            .map_err(|e| Status::internal(format!("Cannot write {conf_path}: {e}")))?;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| Status::internal(format!("Cannot set permissions on {conf_path}: {e}")))?;
        std::fs::rename(&tmp_path, &conf_path)
            .map_err(|e| Status::internal(format!("Cannot rename {conf_path}: {e}")))?;

        if !crate::services::wireguard::interface_exists(&iface).await {
            crate::services::wireguard::create_wg_interface(&iface, &conf_path)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
        } else {
            crate::services::wireguard::apply_syncconf(&iface, &conf_path)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
        }
    }

    // 2. BIRD: full regenerate bird.conf + apply
    let peer_communities = if settings.enable_community_filters {
        let probe_repo = ProbeResultRepository::new(pool.clone());
        let rule_repo = CommunityRuleRepository::new(pool.clone());
        let rules = rule_repo.list_enabled().await.unwrap_or_default();
        let mut map = std::collections::HashMap::new();
        for peer in &peers {
            if !peer.enabled {
                continue;
            }
            match CommunityMapper::compute_communities_with_rules(
                peer,
                listen_addr,
                &probe_repo,
                &rules,
            )
            .await
            {
                Ok((v4, v6)) if !v4.is_empty() || !v6.is_empty() => {
                    map.insert(peer.id.clone(), (v4, v6));
                }
                _ => {}
            }
        }
        map
    } else {
        std::collections::HashMap::new()
    };

    let mut bird_config =
        crate::services::bird::generate_full_config(&peers, &settings, "", &peer_communities);

    // Append cluster iBGP blocks if any nodes exist (cluster mode)
    if let Ok(nodes) = state.node_repo.list_all().await {
        let my_tunnel_ip = nodes
            .iter()
            .find(|n| n.listen_addr == listen_addr)
            .map(|n| n.tunnel_ip.clone())
            .unwrap_or_default();
        if !my_tunnel_ip.is_empty() {
            bird_config.push_str(&crate::services::bird::generate_ibgp_blocks(
                &nodes,
                &settings,
                &my_tunnel_ip,
            ));
        }
    }

    if let Err(e) = crate::services::bird::apply_config(&bird_config) {
        apply_status.pending.store(false, std::sync::atomic::Ordering::Relaxed);
        *apply_status.last_apply_error.lock().await = Some(e.to_string());
        return Err(Status::internal(e.to_string()));
    }

    apply_status.pending.store(false, std::sync::atomic::Ordering::Relaxed);
    *apply_status.last_apply_at.lock().await = Some(chrono::Utc::now().to_rfc3339());
    *apply_status.last_apply_error.lock().await = None;

    Ok(())
}

impl PeerServiceImpl {
    async fn proxy_push_peer(&self, target_addr: &str, peer: Peer) -> Result<Peer, Status> {
        use crate::grpc::generated::cluster_service_client::ClusterServiceClient;

        let mut client: ClusterServiceClient<tonic::transport::Channel> =
            crate::cluster::aggregator::ClusterAggregator::connect(target_addr)
                .await
                .map_err(|e| Status::internal(format!("connect failed: {e}")))?;

        let mut req = Request::new(PushPeerRequest {
            peer: Some(peer.clone()),
            origin_node_id: peer.origin_node_id.clone(),
        });

        if !self.cluster_key.is_empty()
            && let Ok(v) = self.cluster_key.parse()
        {
            req.metadata_mut().insert("x-cluster-key", v);
        }

        client
            .push_peer(req)
            .await
            .map_err(|e| Status::internal(format!("proxy push failed: {e}")))?;

        // PushPeerResponse has no peer field — return the peer we sent
        Ok(peer)
    }
}

pub fn peer_to_proto(p: &crate::models::peer::Peer) -> Peer {
    let mut proto: Peer = p.into();
    proto.wg_private_key = String::new();
    proto
}

#[tonic::async_trait]
impl PeerService for PeerServiceImpl {
    async fn list_peers(
        &self,
        request: Request<ListPeersRequest>,
    ) -> Result<Response<ListPeersResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let peers = self
            .state
            .peer_repo
            .list_all()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ListPeersResponse {
            peers: peers.iter().map(peer_to_proto).collect(),
        }))
    }

    async fn get_peer(&self, request: Request<GetPeerRequest>) -> Result<Response<Peer>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        let peer = self
            .state
            .peer_repo
            .find_by_id(&req.id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;

        Ok(Response::new(peer_to_proto(&peer)))
    }

    async fn create_peer(
        &self,
        request: Request<CreatePeerRequest>,
    ) -> Result<Response<Peer>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        let origin = req.origin_node_id.clone();

        // If targeting a remote node, proxy the write
        if !origin.is_empty() && origin != self.listen_addr {
            let target_node = self
                .state
                .node_repo
                .find_by_id(&origin)
                .await
                .map_err(|_| Status::not_found("target node not found"))?;

            let proto = create_request_to_proto(&req);
            let proxied = self
                .proxy_push_peer(&target_node.listen_addr, proto)
                .await?;
            return Ok(Response::new(proxied));
        }

        validate_peer_fields(
            &req.name,
            req.asn,
            &req.wg_public_key,
            &req.ipv4_tunnel_local,
            &req.ipv4_tunnel_remote,
            &req.ipv6_tunnel_local,
            &req.ipv6_tunnel_remote,
        )
        .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let proto = create_request_to_proto(&req);
        let peer = self
            .state
            .peer_repo
            .create_full(&proto.into())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        self.config_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);

        Ok(Response::new(peer_to_proto(&peer)))
    }

    async fn update_peer(
        &self,
        request: Request<UpdatePeerRequest>,
    ) -> Result<Response<Peer>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        let origin = req.origin_node_id.clone();

        // If targeting a remote node, proxy the write
        if !origin.is_empty() && origin != self.listen_addr {
            let target_node = self
                .state
                .node_repo
                .find_by_id(&origin)
                .await
                .map_err(|_| Status::not_found("target node not found"))?;

            let proto = update_request_to_proto(&req);
            let proxied = self
                .proxy_push_peer(&target_node.listen_addr, proto)
                .await?;
            return Ok(Response::new(proxied));
        }

        validate_peer_fields(
            &req.name,
            req.asn,
            &req.wg_public_key,
            &req.ipv4_tunnel_local,
            &req.ipv4_tunnel_remote,
            &req.ipv6_tunnel_local,
            &req.ipv6_tunnel_remote,
        )
        .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let mut peer = self
            .state
            .peer_repo
            .find_by_id(&req.id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;

        peer.apply_proto(&update_request_to_proto(&req));

        let peer = self
            .state
            .peer_repo
            .update(&peer)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        self.config_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);

        Ok(Response::new(peer_to_proto(&peer)))
    }

    async fn delete_peer(
        &self,
        request: Request<DeletePeerRequest>,
    ) -> Result<Response<DeletePeerResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        self.state
            .peer_repo
            .delete(&req.id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;

        self.config_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);

        Ok(Response::new(DeletePeerResponse {}))
    }

    async fn toggle_peer(
        &self,
        request: Request<TogglePeerRequest>,
    ) -> Result<Response<Peer>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        let peer = self
            .state
            .peer_repo
            .toggle_enabled(&req.id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;

        self.config_dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);

        Ok(Response::new(peer_to_proto(&peer)))
    }

    async fn generate_keypair(
        &self,
        _request: Request<GenerateKeypairRequest>,
    ) -> Result<Response<GenerateKeypairResponse>, Status> {
        let (private_key, public_key) = services::wireguard::generate_keypair();
        Ok(Response::new(GenerateKeypairResponse {
            private_key,
            public_key,
        }))
    }

    async fn get_wire_guard_config(
        &self,
        request: Request<GetConfigRequest>,
    ) -> Result<Response<ConfigResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        let peer = self
            .state
            .peer_repo
            .find_by_id(&req.id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;
        let settings = self
            .state
            .settings_repo
            .load()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let content = services::wireguard::generate_config(&peer, &settings);
        Ok(Response::new(ConfigResponse { content }))
    }

    async fn get_bird_config(
        &self,
        request: Request<GetConfigRequest>,
    ) -> Result<Response<ConfigResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        let peer = self
            .state
            .peer_repo
            .find_by_id(&req.id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;
        let settings = self
            .state
            .settings_repo
            .load()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let content = services::bird::generate_peer_block(&peer, &settings);
        Ok(Response::new(ConfigResponse { content }))
    }

    async fn export_all_wire_guard(
        &self,
        request: Request<ExportAllRequest>,
    ) -> Result<Response<ConfigResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let peers = self
            .state
            .peer_repo
            .list_all()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let settings = self
            .state
            .settings_repo
            .load()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let content: String = peers
            .iter()
            .filter(|p| p.enabled)
            .map(|p| services::wireguard::generate_config(p, &settings))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(Response::new(ConfigResponse { content }))
    }

    async fn export_all_bird(
        &self,
        request: Request<ExportAllRequest>,
    ) -> Result<Response<ConfigResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let peers = self
            .state
            .peer_repo
            .list_all()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let settings = self
            .state
            .settings_repo
            .load()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let content = services::bird::generate_full_config(
            &peers,
            &settings,
            "",
            &std::collections::HashMap::new(),
        );
        Ok(Response::new(ConfigResponse { content }))
    }

    async fn restart_wire_guard(
        &self,
        request: Request<RestartWireGuardRequest>,
    ) -> Result<Response<RestartWireGuardResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        let iface = if req.interface_name.is_empty() {
            "wg0"
        } else {
            &req.interface_name
        };
        crate::services::wireguard::restart_interface(iface)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(RestartWireGuardResponse {}))
    }
}

fn create_request_to_proto(req: &CreatePeerRequest) -> Peer {
    Peer {
        id: String::new(),
        name: req.name.clone(),
        description: req.description.clone(),
        asn: req.asn,
        local_asn: req.local_asn,
        wg_private_key: req.wg_private_key.clone(),
        wg_public_key: req.wg_public_key.clone(),
        wg_remote_address: req.wg_remote_address.clone(),
        wg_remote_port: req.wg_remote_port,
        wg_listen_port: req.wg_listen_port,
        wg_interface_name: req.wg_interface_name.clone(),
        ipv4_tunnel_local: req.ipv4_tunnel_local.clone(),
        ipv4_tunnel_remote: req.ipv4_tunnel_remote.clone(),
        ipv6_tunnel_local: req.ipv6_tunnel_local.clone(),
        ipv6_tunnel_remote: req.ipv6_tunnel_remote.clone(),
        multiprotocol: req.multiprotocol,
        extended_nexthop: req.extended_nexthop,
        sessions: req.sessions,
        passive: req.passive,
        import_max_prefix: req.import_max_prefix,
        export_max_prefix: req.export_max_prefix,
        enabled: true,
        created_at: String::new(),
        updated_at: String::new(),
        origin_node_id: req.origin_node_id.clone(),
    }
}

fn update_request_to_proto(req: &UpdatePeerRequest) -> Peer {
    Peer {
        id: req.id.clone(),
        name: req.name.clone(),
        description: req.description.clone(),
        asn: req.asn,
        local_asn: req.local_asn,
        wg_private_key: req.wg_private_key.clone(),
        wg_public_key: req.wg_public_key.clone(),
        wg_remote_address: req.wg_remote_address.clone(),
        wg_remote_port: req.wg_remote_port,
        wg_listen_port: req.wg_listen_port,
        wg_interface_name: req.wg_interface_name.clone(),
        ipv4_tunnel_local: req.ipv4_tunnel_local.clone(),
        ipv4_tunnel_remote: req.ipv4_tunnel_remote.clone(),
        ipv6_tunnel_local: req.ipv6_tunnel_local.clone(),
        ipv6_tunnel_remote: req.ipv6_tunnel_remote.clone(),
        multiprotocol: req.multiprotocol,
        extended_nexthop: req.extended_nexthop,
        sessions: req.sessions,
        passive: req.passive,
        import_max_prefix: req.import_max_prefix,
        export_max_prefix: req.export_max_prefix,
        enabled: true,
        created_at: String::new(),
        updated_at: String::new(),
        origin_node_id: req.origin_node_id.clone(),
    }
}

fn validate_peer_fields(
    name: &str,
    asn: i64,
    wg_public_key: &str,
    ipv4_local: &str,
    ipv4_remote: &str,
    ipv6_local: &str,
    ipv6_remote: &str,
) -> Result<(), crate::error::AppError> {
    use crate::services::validation;

    validation::validate_peer_name(name)?;
    if asn != 0 {
        validation::validate_asn(asn)?;
    }
    if !wg_public_key.is_empty() {
        validation::validate_wg_public_key(wg_public_key)?;
    }
    if !ipv4_local.is_empty() {
        validation::validate_ipv4(ipv4_local)?;
    }
    if !ipv4_remote.is_empty() {
        validation::validate_ipv4(ipv4_remote)?;
    }
    if !ipv6_local.is_empty() {
        validation::validate_ipv6(ipv6_local)?;
    }
    if !ipv6_remote.is_empty() {
        validation::validate_ipv6(ipv6_remote)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_peer() -> crate::models::peer::Peer {
        crate::models::peer::Peer {
            id: "test".into(),
            name: "test".into(),
            description: None,
            asn: 4242420001,
            local_asn: 4242420000,
            wg_private_key: Some("super-secret-key".into()),
            wg_public_key: Some("pubkey".into()),
            wg_remote_address: "10.0.0.1".into(),
            wg_remote_port: 42420,
            wg_listen_port: 42420,
            wg_interface_name: "wg0".into(),
            ipv4_tunnel_local: Some("172.20.1.1".into()),
            ipv4_tunnel_remote: Some("172.20.1.2".into()),
            ipv6_tunnel_local: None,
            ipv6_tunnel_remote: None,
            multiprotocol: false,
            extended_nexthop: false,
            sessions: 0,
            passive: false,
            import_max_prefix: None,
            export_max_prefix: None,
            enabled: true,
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:00:00Z".into(),
            origin_node_id: None,
        }
    }

    #[test]
    fn test_peer_to_proto_redacts_private_key() {
        let peer = make_test_peer();
        let proto = peer_to_proto(&peer);
        assert!(
            proto.wg_private_key.is_empty(),
            "Private key should be redacted"
        );
    }
}
