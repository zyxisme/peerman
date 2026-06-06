use tonic::{Request, Response, Status};

use super::generated::{
    ApplyConfigNowRequest, ApplyConfigNowResponse, BirdStatusResponse, GetApplyStatusRequest,
    GetApplyStatusResponse, GetBirdStatusRequest, GetWgStatusRequest, WgStatusResponse,
    management_service_server::ManagementService,
};

pub struct ManagementServiceImpl {
    pub jwt_secret: std::sync::Arc<String>,
    pub apply_status: crate::app_state::ApplyStatus,
    pub peer_state: crate::app_state::PeerState,
    pub listen_addr: String,
    pub pool: sqlx::SqlitePool,
}

#[tonic::async_trait]
impl ManagementService for ManagementServiceImpl {
    async fn get_wire_guard_status(
        &self,
        request: Request<GetWgStatusRequest>,
    ) -> Result<Response<WgStatusResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        let iface = if req.interface.is_empty() {
            "all"
        } else {
            &req.interface
        };

        let interfaces = crate::services::wireguard::get_wg_status(iface)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(WgStatusResponse { interfaces }))
    }

    async fn get_bird_status(
        &self,
        request: Request<GetBirdStatusRequest>,
    ) -> Result<Response<BirdStatusResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let protocols = crate::services::bird::get_bird_status()
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(BirdStatusResponse { protocols }))
    }

    async fn get_apply_status(
        &self,
        request: Request<GetApplyStatusRequest>,
    ) -> Result<Response<GetApplyStatusResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;

        let last_apply_at = self
            .apply_status
            .last_apply_at
            .lock()
            .await
            .clone()
            .unwrap_or_default();
        let last_error = self
            .apply_status
            .last_apply_error
            .lock()
            .await
            .clone()
            .unwrap_or_default();
        let pending = self
            .apply_status
            .pending
            .load(std::sync::atomic::Ordering::Relaxed);
        let interfaces = self.apply_status.managed_interfaces.lock().await.clone();

        Ok(Response::new(GetApplyStatusResponse {
            status: Some(super::generated::ApplyStatus {
                last_apply_at,
                pending,
                last_error,
                managed_interfaces: interfaces,
            }),
        }))
    }

    async fn apply_config_now(
        &self,
        request: Request<ApplyConfigNowRequest>,
    ) -> Result<Response<ApplyConfigNowResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;

        crate::grpc::peer_service::apply_wg_bird(
            &self.peer_state,
            &self.listen_addr,
            &self.pool,
            &self.apply_status,
        )
        .await?;

        Ok(Response::new(ApplyConfigNowResponse {}))
    }
}
