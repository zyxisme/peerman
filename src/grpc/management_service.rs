use tonic::{Request, Response, Status};

use super::generated::{
    management_service_server::ManagementService,
    GetWgStatusRequest, WgStatusResponse,
    GetBirdStatusRequest, BirdStatusResponse,
};

pub struct ManagementServiceImpl;

#[tonic::async_trait]
impl ManagementService for ManagementServiceImpl {
    async fn get_wire_guard_status(
        &self,
        request: Request<GetWgStatusRequest>,
    ) -> Result<Response<WgStatusResponse>, Status> {
        let req = request.into_inner();
        let iface = if req.interface.is_empty() { "all" } else { &req.interface };

        let interfaces = crate::services::wireguard::get_wg_status(iface)
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(WgStatusResponse { interfaces }))
    }

    async fn get_bird_status(
        &self,
        _request: Request<GetBirdStatusRequest>,
    ) -> Result<Response<BirdStatusResponse>, Status> {
        let protocols = crate::services::bird::get_bird_status()
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(BirdStatusResponse { protocols }))
    }
}
