use tonic::{Request, Response, Status};

use super::generated::{
    bird_service_server::BirdService, ExecuteCommandRequest, ExecuteCommandResponse,
    NodeBirdResult, NodeTracerouteResult, RunTracerouteRequest, RunTracerouteResponse,
};

use crate::services::bird_socket::BirdSocket;

pub struct BirdServiceImpl {
    pub node_name: String,
    pub jwt_secret: std::sync::Arc<String>,
    pub cluster_key: std::sync::Arc<String>,
    pub node_repo: crate::models::node::NodeRepository,
    pub cache: crate::cluster::cache::ClusterCache,
}

#[tonic::async_trait]
impl BirdService for BirdServiceImpl {
    async fn execute_command(
        &self,
        request: Request<ExecuteCommandRequest>,
    ) -> Result<Response<ExecuteCommandResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();

        let results = if req.target_node_id.is_empty() || req.target_node_id == self.node_name {
            // Local execution
            let output = execute_local(&req.command).await?;
            vec![NodeBirdResult {
                node_id: self.node_name.clone(),
                node_name: self.node_name.clone(),
                output,
                status_code: 0,
                error: String::new(),
            }]
        } else {
            // Remote node — forward via cluster
            let aggregator = crate::cluster::aggregator::ClusterAggregator::new(
                self.cache.clone(),
                self.cluster_key.as_ref().clone(),
            );

            // Look up target node address
            let nodes = self
                .node_repo
                .list_all()
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            let target_node = nodes
                .iter()
                .find(|n| n.name == req.target_node_id || n.id == req.target_node_id)
                .ok_or_else(|| {
                    Status::not_found(format!("node {} not found", req.target_node_id))
                })?;

            match aggregator
                .execute_bird_command(&target_node.listen_addr, &req.command)
                .await
            {
                Ok(output) => vec![NodeBirdResult {
                    node_id: req.target_node_id.clone(),
                    node_name: target_node.name.clone(),
                    output,
                    status_code: 0,
                    error: String::new(),
                }],
                Err(e) => vec![NodeBirdResult {
                    node_id: req.target_node_id.clone(),
                    node_name: target_node.name.clone(),
                    output: String::new(),
                    status_code: 1,
                    error: e,
                }],
            }
        };

        Ok(Response::new(ExecuteCommandResponse { results }))
    }

    async fn run_traceroute(
        &self,
        request: Request<RunTracerouteRequest>,
    ) -> Result<Response<RunTracerouteResponse>, Status> {
        crate::auth::check_auth(&request, self.jwt_secret.as_ref())?;
        let req = request.into_inner();
        let target = req.target.trim().to_string();

        if target.is_empty() {
            return Err(Status::invalid_argument("target is required"));
        }

        // Run traceroute subprocess
        let output = match tokio::process::Command::new("traceroute")
            .arg("-n")
            .arg(&target)
            .output()
            .await
        {
            Ok(out) => {
                if out.status.success() {
                    String::from_utf8_lossy(&out.stdout).to_string()
                } else {
                    String::from_utf8_lossy(&out.stderr).to_string()
                }
            }
            Err(e) => format!("traceroute failed: {e}"),
        };

        Ok(Response::new(RunTracerouteResponse {
            results: vec![NodeTracerouteResult {
                node_id: self.node_name.clone(),
                node_name: self.node_name.clone(),
                output,
            }],
        }))
    }
}

async fn execute_local(command: &str) -> Result<String, Status> {
    let mut socket = BirdSocket::connect()
        .await
        .map_err(|e| Status::unavailable(format!("Cannot connect to BIRD socket: {e}")))?;

    let response = socket
        .execute(command)
        .await
        .map_err(|e| Status::internal(format!("BIRD command failed: {e}")))?;

    Ok(response.lines.join("\n"))
}
