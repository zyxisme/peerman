mod app_state;
mod config;
mod db;
mod error;
mod grpc;
mod models;
mod services;
mod static_files;

use std::net::SocketAddr;

use axum::Router;
use clap::Parser;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::grpc::generated::peer_service_server::PeerServiceServer;
use crate::grpc::generated::settings_service_server::SettingsServiceServer;
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

    // Build gRPC services
    let peer_svc = PeerServiceImpl {
        peer_repo: state.peer_repo.clone(),
        settings_repo: state.settings_repo.clone(),
    };
    let settings_svc = SettingsServiceImpl {
        settings_repo: state.settings_repo.clone(),
    };

    // Build tonic gRPC router with tonic-web wrapper
    let grpc_router = tonic::transport::Server::builder()
        .accept_http1(true)
        .layer(tonic_web::GrpcWebLayer::new())
        .add_service(PeerServiceServer::new(peer_svc))
        .add_service(SettingsServiceServer::new(settings_svc))
        .into_router();

    // Build axum router: static files + gRPC
    let app = Router::new()
        .nest("/api", grpc_router)
        .fallback(static_files::serve_static)
        .layer(TraceLayer::new_for_http());

    let addr: SocketAddr = cfg.listen_addr.parse()?;
    tracing::info!("peerman ready at http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
