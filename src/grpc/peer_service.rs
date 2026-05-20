use tonic::{Request, Response, Status};

use super::generated::{
    peer_service_server::PeerService, ConfigResponse, CreatePeerRequest, DeletePeerRequest,
    DeletePeerResponse, ExportAllRequest, GenerateKeypairRequest, GenerateKeypairResponse,
    GetConfigRequest, GetPeerRequest, ListPeersRequest, ListPeersResponse, Peer,
    TogglePeerRequest, UpdatePeerRequest,
};

use crate::models::peer::PeerRepository;
use crate::models::settings::SettingsRepository;
use crate::services;

pub struct PeerServiceImpl {
    pub peer_repo: PeerRepository,
    pub settings_repo: SettingsRepository,
}

pub fn peer_to_proto(p: &crate::models::peer::Peer) -> Peer {
    Peer {
        id: p.id.clone(),
        name: p.name.clone(),
        description: p.description.clone().unwrap_or_default(),
        asn: p.asn,
        local_asn: p.local_asn,
        wg_private_key: p.wg_private_key.clone().unwrap_or_default(),
        wg_public_key: p.wg_public_key.clone().unwrap_or_default(),
        wg_remote_address: p.wg_remote_address.clone(),
        wg_remote_port: p.wg_remote_port as u32,
        wg_listen_port: p.wg_listen_port as u32,
        wg_interface_name: p.wg_interface_name.clone(),
        ipv4_tunnel_local: p.ipv4_tunnel_local.clone().unwrap_or_default(),
        ipv4_tunnel_remote: p.ipv4_tunnel_remote.clone().unwrap_or_default(),
        ipv6_tunnel_local: p.ipv6_tunnel_local.clone().unwrap_or_default(),
        ipv6_tunnel_remote: p.ipv6_tunnel_remote.clone().unwrap_or_default(),
        multiprotocol: p.multiprotocol,
        extended_nexthop: p.extended_nexthop,
        sessions: p.sessions,
        passive: p.passive,
        import_max_prefix: p.import_max_prefix.unwrap_or(0) as i32,
        export_max_prefix: p.export_max_prefix.unwrap_or(0) as i32,
        enabled: p.enabled,
        created_at: p.created_at.clone(),
        updated_at: p.updated_at.clone(),
        origin_node_id: p.origin_node_id.clone().unwrap_or_default(),
    }
}

fn apply_create_fields(peer: &mut crate::models::peer::Peer, req: &CreatePeerRequest) {
    peer.name = req.name.clone();
    peer.description = if req.description.is_empty() { None } else { Some(req.description.clone()) };
    peer.asn = req.asn;
    peer.local_asn = req.local_asn;
    peer.wg_private_key = if req.wg_private_key.is_empty() { None } else { Some(req.wg_private_key.clone()) };
    peer.wg_public_key = if req.wg_public_key.is_empty() { None } else { Some(req.wg_public_key.clone()) };
    peer.wg_remote_address = req.wg_remote_address.clone();
    peer.wg_remote_port = req.wg_remote_port as i64;
    peer.wg_listen_port = req.wg_listen_port as i64;
    peer.wg_interface_name = req.wg_interface_name.clone();
    peer.ipv4_tunnel_local = if req.ipv4_tunnel_local.is_empty() { None } else { Some(req.ipv4_tunnel_local.clone()) };
    peer.ipv4_tunnel_remote = if req.ipv4_tunnel_remote.is_empty() { None } else { Some(req.ipv4_tunnel_remote.clone()) };
    peer.ipv6_tunnel_local = if req.ipv6_tunnel_local.is_empty() { None } else { Some(req.ipv6_tunnel_local.clone()) };
    peer.ipv6_tunnel_remote = if req.ipv6_tunnel_remote.is_empty() { None } else { Some(req.ipv6_tunnel_remote.clone()) };
    peer.multiprotocol = req.multiprotocol;
    peer.extended_nexthop = req.extended_nexthop;
    peer.sessions = req.sessions;
    peer.passive = req.passive;
    peer.import_max_prefix = if req.import_max_prefix == 0 { None } else { Some(req.import_max_prefix as i64) };
    peer.export_max_prefix = if req.export_max_prefix == 0 { None } else { Some(req.export_max_prefix as i64) };
    peer.origin_node_id = if req.origin_node_id.is_empty() { None } else { Some(req.origin_node_id.clone()) };
}

#[tonic::async_trait]
impl PeerService for PeerServiceImpl {
    async fn list_peers(
        &self,
        _request: Request<ListPeersRequest>,
    ) -> Result<Response<ListPeersResponse>, Status> {
        let peers = self
            .peer_repo
            .list_all()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ListPeersResponse {
            peers: peers.iter().map(peer_to_proto).collect(),
        }))
    }

    async fn get_peer(
        &self,
        request: Request<GetPeerRequest>,
    ) -> Result<Response<Peer>, Status> {
        let req = request.into_inner();
        let peer = self
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
        let req = request.into_inner();

        services::validation::validate_peer_name(&req.name)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let mut peer = self
            .peer_repo
            .create(&req.name)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        apply_create_fields(&mut peer, &req);

        let peer = self
            .peer_repo
            .update(&peer)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(peer_to_proto(&peer)))
    }

    async fn update_peer(
        &self,
        request: Request<UpdatePeerRequest>,
    ) -> Result<Response<Peer>, Status> {
        let req = request.into_inner();

        let mut peer = self
            .peer_repo
            .find_by_id(&req.id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;

        peer.name = req.name.clone();
        peer.description = if req.description.is_empty() { None } else { Some(req.description.clone()) };
        peer.asn = req.asn;
        peer.local_asn = req.local_asn;
        peer.wg_private_key = if req.wg_private_key.is_empty() { None } else { Some(req.wg_private_key.clone()) };
        peer.wg_public_key = if req.wg_public_key.is_empty() { None } else { Some(req.wg_public_key.clone()) };
        peer.wg_remote_address = req.wg_remote_address.clone();
        peer.wg_remote_port = req.wg_remote_port as i64;
        peer.wg_listen_port = req.wg_listen_port as i64;
        peer.wg_interface_name = req.wg_interface_name.clone();
        peer.ipv4_tunnel_local = if req.ipv4_tunnel_local.is_empty() { None } else { Some(req.ipv4_tunnel_local.clone()) };
        peer.ipv4_tunnel_remote = if req.ipv4_tunnel_remote.is_empty() { None } else { Some(req.ipv4_tunnel_remote.clone()) };
        peer.ipv6_tunnel_local = if req.ipv6_tunnel_local.is_empty() { None } else { Some(req.ipv6_tunnel_local.clone()) };
        peer.ipv6_tunnel_remote = if req.ipv6_tunnel_remote.is_empty() { None } else { Some(req.ipv6_tunnel_remote.clone()) };
        peer.multiprotocol = req.multiprotocol;
        peer.extended_nexthop = req.extended_nexthop;
        peer.sessions = req.sessions;
        peer.passive = req.passive;
        peer.import_max_prefix = if req.import_max_prefix == 0 { None } else { Some(req.import_max_prefix as i64) };
        peer.export_max_prefix = if req.export_max_prefix == 0 { None } else { Some(req.export_max_prefix as i64) };
        peer.origin_node_id = if req.origin_node_id.is_empty() { None } else { Some(req.origin_node_id.clone()) };

        let peer = self
            .peer_repo
            .update(&peer)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(peer_to_proto(&peer)))
    }

    async fn delete_peer(
        &self,
        request: Request<DeletePeerRequest>,
    ) -> Result<Response<DeletePeerResponse>, Status> {
        let req = request.into_inner();
        self.peer_repo
            .delete(&req.id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;

        Ok(Response::new(DeletePeerResponse {}))
    }

    async fn toggle_peer(
        &self,
        request: Request<TogglePeerRequest>,
    ) -> Result<Response<Peer>, Status> {
        let req = request.into_inner();
        let peer = self
            .peer_repo
            .toggle_enabled(&req.id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;

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
        let req = request.into_inner();
        let peer = self
            .peer_repo
            .find_by_id(&req.id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;
        let settings = self
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
        let req = request.into_inner();
        let peer = self
            .peer_repo
            .find_by_id(&req.id)
            .await
            .map_err(|e| Status::not_found(e.to_string()))?;
        let settings = self
            .settings_repo
            .load()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let content = services::bird::generate_peer_block(&peer, &settings);
        Ok(Response::new(ConfigResponse { content }))
    }

    async fn export_all_wire_guard(
        &self,
        _request: Request<ExportAllRequest>,
    ) -> Result<Response<ConfigResponse>, Status> {
        let peers = self
            .peer_repo
            .list_all()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let settings = self
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
        _request: Request<ExportAllRequest>,
    ) -> Result<Response<ConfigResponse>, Status> {
        let peers = self
            .peer_repo
            .list_all()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let settings = self
            .settings_repo
            .load()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let content = services::bird::generate_full_config(&peers, &settings, "");
        Ok(Response::new(ConfigResponse { content }))
    }
}
