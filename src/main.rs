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
use tokio_util::sync::CancellationToken;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::grpc::generated::bird_service_server::BirdServiceServer;
use crate::grpc::generated::cluster_service_server::ClusterServiceServer;
use crate::grpc::generated::flap_service_server::FlapServiceServer;
use crate::grpc::generated::peer_service_server::PeerServiceServer;
use crate::grpc::generated::settings_service_server::SettingsServiceServer;
use crate::grpc::bird_service::BirdServiceImpl;
use crate::grpc::cluster_service::ClusterServiceImpl;
use crate::grpc::flap_service::FlapServiceImpl;
use crate::grpc::peer_service::PeerServiceImpl;
use crate::grpc::settings_service::SettingsServiceImpl;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = config::Cli::parse();
    let cfg = config::Config::load(&cli.config)?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&cfg.logging.level))
        .init();

    tracing::info!("Starting peerman, listening on {}", cfg.server.listen_addr);

    // Database
    let pool = db::create_pool(&cfg.storage.db_path).await?;
    let state = app_state::AppState::new(pool.clone());

    // Verify
    let peers = state.peer_repo.list_all().await?;
    tracing::info!("Loaded {} peers from database", peers.len());

    // Seed community rules
    let rules_count = state.community_repo.list_all().await?.len() as i64;
    state.community_repo.seed_defaults(rules_count).await?;

    // Global cancellation token for graceful shutdown
    let shutdown = CancellationToken::new();

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
    let bird_svc = BirdServiceImpl {
        node_name: cfg.cluster.node_name.clone(),
        node_repo: state.node_repo.clone(),
    };
    let flap_svc = FlapServiceImpl {
        flap_repo: state.flap_event_repo.clone(),
    };

    // Build tonic gRPC router with tonic-web wrapper
    let grpc_router = tonic::transport::Server::builder()
        .accept_http1(true)
        .layer(tonic_web::GrpcWebLayer::new())
        .add_service(PeerServiceServer::new(peer_svc))
        .add_service(SettingsServiceServer::new(settings_svc))
        .add_service(ClusterServiceServer::new(cluster_svc))
        .add_service(BirdServiceServer::new(bird_svc))
        .add_service(FlapServiceServer::new(flap_svc))
        .into_router();

    // Build axum router: static files + gRPC
    let app = Router::new()
        .nest("/api", grpc_router)
        .fallback(static_files::serve_static)
        .layer(TraceLayer::new_for_http());

    // Self-register if cluster mode is enabled
    if !cfg.cluster.node_name.is_empty() {
        let local_asn = state.settings_repo.load().await?.local_asn;
        let node = state
            .node_repo
            .upsert_self(&cfg.cluster.node_name, &cfg.server.listen_addr, local_asn)
            .await?;
        tracing::info!(
            "Self-registered as node '{}' (id={}, asn={})",
            cfg.cluster.node_name,
            node.id,
            local_asn
        );

        // Mark known bootstrap nodes (add them if not already present)
        for addr in &cfg.cluster.bootstrap_nodes {
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

        // Spawn periodic stale-node cleanup task
        let stale_state = state.clone();
        let stale_interval = cfg.cluster.sync_interval_secs;
        let stale_token = shutdown.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = stale_token.cancelled() => {
                        tracing::info!("Stale-node cleanup task shutting down");
                        return;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(stale_interval)) => {}
                }
                if let Err(e) = stale_state.node_repo.mark_stale(120).await {
                    tracing::warn!("Failed to mark stale nodes: {}", e);
                }
            }
        });

        // Spawn periodic probe task
        if cfg.cluster.probe_interval_secs > 0 {
            let probe_state = state.clone();
            let probe_node_name = cfg.cluster.node_name.clone();
            let probe_interval = cfg.cluster.probe_interval_secs;
            let probe_token = shutdown.clone();

            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = probe_token.cancelled() => {
                            tracing::info!("Probe task shutting down");
                            return;
                        }
                        _ = tokio::time::sleep(Duration::from_secs(probe_interval)) => {}
                    }

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

        // Spawn BGP flap detector
        let node_id = node.id.clone();
        let node_name = cfg.cluster.node_name.clone();
        let flap_repo = state.flap_event_repo.clone();
        let flap_token = shutdown.clone();

        tokio::spawn(async move {
            tracing::info!("Starting BGP flap detector for node '{node_name}' ({node_id})");

            let (tx, rx) = tokio::sync::mpsc::channel::<
                crate::services::bgp_listener::PathChange,
            >(1024);

            match crate::services::bgp_listener::BgpListener::bind(node_id.clone()).await {
                Ok(listener) => {
                    tracing::info!("iBGP listener active on [::1]:1790");
                    let bgp_tx = tx.clone();
                    let bgp_token = flap_token.clone();
                    tokio::spawn(async move {
                        tokio::select! {
                            _ = bgp_token.cancelled() => {
                                tracing::info!("iBGP listener shutting down");
                            }
                            _ = listener.run(bgp_tx) => {}
                        }
                    });

                    let mut detector =
                        crate::services::flap_detector::FlapDetector::new(node_id.clone(), flap_repo);
                    detector.run(rx, flap_token).await;
                }
                Err(e) => {
                    tracing::warn!("iBGP listener unavailable ({e}), flap detection will use socket polling fallback");
                    let mut detector =
                        crate::services::flap_detector::FlapDetector::new(node_id, flap_repo);
                    detector.run(rx, flap_token).await;
                }
            }
        });
    }

    let addr: SocketAddr = cfg.server.listen_addr.parse()?;
    tracing::info!("peerman ready at http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;

    tokio::select! {
        result = axum::serve(listener, app) => {
            if let Err(e) = result {
                tracing::error!("Server error: {e}");
            }
        }
        _ = shutdown.cancelled() => {
            tracing::info!("Shutdown signal received");
        }
    }

    tracing::info!("Waiting for background tasks to complete...");
    // Give tasks a grace period to finish
    tokio::time::sleep(Duration::from_secs(2)).await;
    tracing::info!("peerman stopped");

    Ok(())
}
