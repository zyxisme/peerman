use tonic::{Request, Response, Status};

use super::generated::{
    flap_service_server::FlapService, FlapEvent, GetFlapStatsRequest, GetFlapStatsResponse,
    ListFlapEventsRequest, ListFlapEventsResponse,
};

use crate::models::flap_event::FlapEventRepository;

pub struct FlapServiceImpl {
    pub flap_repo: FlapEventRepository,
    pub jwt_secret: std::sync::Arc<String>,
}

fn flap_event_to_proto(e: &crate::models::flap_event::FlapEvent) -> FlapEvent {
    FlapEvent {
        id: e.id.clone(),
        prefix: e.prefix.clone(),
        prefix_type: e.prefix_type.clone(),
        node_id: e.node_id.clone(),
        change_count: e.change_count,
        window_start: e.window_start.clone(),
        window_end: e.window_end.clone(),
        source: e.source.clone(),
        active: e.active,
        detected_at: e.detected_at.clone(),
        resolved_at: e.resolved_at.clone().unwrap_or_default(),
    }
}

#[tonic::async_trait]
impl FlapService for FlapServiceImpl {
    async fn list_flap_events(
        &self,
        request: Request<ListFlapEventsRequest>,
    ) -> Result<Response<ListFlapEventsResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        let events = if req.active_only {
            self.flap_repo
                .list_active()
                .await
                .map_err(|e| Status::internal(e.to_string()))?
        } else {
            self.flap_repo
                .list_recent(req.limit)
                .await
                .map_err(|e| Status::internal(e.to_string()))?
        };

        Ok(Response::new(ListFlapEventsResponse {
            events: events.iter().map(flap_event_to_proto).collect(),
        }))
    }

    async fn get_flap_stats(
        &self,
        request: Request<GetFlapStatsRequest>,
    ) -> Result<Response<GetFlapStatsResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let stats = self
            .flap_repo
            .stats()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(GetFlapStatsResponse {
            active_count: stats.active_count,
            total_today: stats.total_today,
            avg_changes_per_hour: stats.avg_changes_per_hour,
        }))
    }
}
