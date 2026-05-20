mod app_state;
mod config;
mod db;
mod error;
mod grpc;
mod models;
mod services;
mod static_files;

use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use clap::Parser;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::grpc::generated::cluster_service_server::ClusterServiceServer;
use crate::grpc::generated::peer_service_server::PeerServiceServer;
use crate::grpc::generated::settings_service_server::SettingsServiceServer;
use crate::grpc::cluster_service::ClusterServiceImpl;
use crate::grpc::peer_service::PeerServiceImpl;
use crate::grpc::settings_service::SettingsServiceImpl;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = config::Config::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&cfg.log_level))
        .init();

    tracing::info!("Starting peerman, listening on {}", cfg.listen_addr);

    // Database
    let pool = db::create_pool(&cfg.db_path).await?;
    let state = app_state::AppState::new(pool.clone());

    // Verify
    let peers = state.peer_repo.list_all().await?;
    tracing::info!("Loaded {} peers from database", peers.len());

    // Seed community rules
    let rules_count = state.community_repo.list_all().await?.len() as i64;
    state.community_repo.seed_defaults(rules_count).await?;

    // Build gRPC services
    let peer_svc = PeerServiceImpl {
        peer_repo: state.peer_repo.clone(),
        settings_repo: state.settings_repo.clone(),
    };
    let settings_svc = SettingsServiceImpl {
        settings_repo: state.settings_repo.clone(),
    };
    let cluster_svc = ClusterServiceImpl {
        node_repo: state.node_repo.clone(),
        peer_repo: state.peer_repo.clone(),
        probe_repo: state.probe_repo.clone(),
        community_repo: state.community_repo.clone(),
    };

    // Build tonic gRPC router with tonic-web wrapper
    let grpc_router = tonic::transport::Server::builder()
        .accept_http1(true)
        .layer(tonic_web::GrpcWebLayer::new())
        .add_service(PeerServiceServer::new(peer_svc))
        .add_service(SettingsServiceServer::new(settings_svc))
        .add_service(ClusterServiceServer::new(cluster_svc))
        .into_router();

    // Build axum router: static files + gRPC
    let app = Router::new()
        .nest("/api", grpc_router)
        .fallback(static_files::serve_static)
        .layer(TraceLayer::new_for_http());

    // Self-register if cluster mode is enabled
    if !cfg.node_name.is_empty() {
        let local_asn = state.settings_repo.load().await?.local_asn;
        let node = state
            .node_repo
            .upsert_self(&cfg.node_name, &cfg.listen_addr, local_asn)
            .await?;
        tracing::info!(
            "Self-registered as node '{}' (id={}, asn={})",
            cfg.node_name,
            node.id,
            local_asn
        );

        // Mark known bootstrap nodes (add them if not already present)
        if !cfg.cluster_nodes.is_empty() {
            for addr in cfg.cluster_nodes.split(',') {
                let addr = addr.trim();
                if addr.is_empty() {
                    continue;
                }
                if state.node_repo.find_by_listen_addr(addr).await?.is_none() {
                    let name = format!("node-{}", addr.replace([':', '.'], "-"));
                    match state
                        .node_repo
                        .create(&name, addr, 0, "bootstrap node")
                        .await
                    {
                        Ok(n) => tracing::info!("Added bootstrap node: {} ({})", name, n.id),
                        Err(e) => tracing::warn!("Failed to add bootstrap node {}: {}", addr, e),
                    }
                }
            }
        }

        // Spawn periodic stale-node cleanup task
        let stale_state = state.clone();
        let stale_interval = cfg.sync_interval_secs;

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(stale_interval)).await;
                if let Err(e) = stale_state.node_repo.mark_stale(120).await {
                    tracing::warn!("Failed to mark stale nodes: {}", e);
                }
            }
        });

        // Spawn periodic probe task
        if cfg.probe_interval_secs > 0 {
            let probe_state = state.clone();
            let probe_node_name = cfg.node_name.clone();
            let probe_interval = cfg.probe_interval_secs;

            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(probe_interval)).await;

                    let nodes = match probe_state.node_repo.list_all().await {
                        Ok(n) => n,
                        Err(e) => {
                            tracing::warn!("Failed to list nodes for probe: {}", e);
                            continue;
                        }
                    };

                    let local_node = nodes.iter().find(|n| n.name == probe_node_name);
                    let local = match local_node {
                        Some(n) => n.clone(),
                        None => continue,
                    };

                    let results =
                        crate::services::probe::probe_all(&local, &nodes, &probe_state.probe_repo)
                            .await;

                    if !results.is_empty() {
                        tracing::info!("Completed {} probes", results.len());
                    }
                }
            });
        }
    }

    let addr: SocketAddr = cfg.listen_addr.parse()?;
    tracing::info!("peerman ready at http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
