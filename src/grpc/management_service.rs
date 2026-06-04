use tonic::{Request, Response, Status};

use super::generated::{
    management_service_server::ManagementService, BirdStatusResponse, GetBirdStatusRequest,
    GetWgStatusRequest, WgStatusResponse,
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
}
