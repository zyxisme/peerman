use tonic::{Request, Response, Status};

use super::generated::{
    ApplyConfigNowRequest, ApplyConfigNowResponse, BirdStatusResponse, GetApplyStatusRequest,
    GetApplyStatusResponse, GetBirdStatusRequest, GetWgStatusRequest, WgStatusResponse,
    management_service_server::ManagementService,
};

pub struct ManagementServiceImpl {
    pub jwt_secret: std::sync::Arc<String>,
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
        // TODO: implement in Task 7
        Err(Status::unimplemented("GetApplyStatus not yet implemented"))
    }

    async fn apply_config_now(
        &self,
        request: Request<ApplyConfigNowRequest>,
    ) -> Result<Response<ApplyConfigNowResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        // TODO: implement in Task 7
        Err(Status::unimplemented("ApplyConfigNow not yet implemented"))
    }
}
