mod app_state;
mod auth;
mod cluster;
mod config;
mod db;
mod error;
mod grpc;
mod http;
mod models;
mod services;
mod static_files;
mod tasks;

use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::Router;
use axum::routing::{get, post};
use clap::Parser;
use tokio_util::sync::CancellationToken;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

/// Global config (set once at startup, read by HTTP handlers).
pub(crate) static APP_CONFIG: OnceLock<Arc<config::Config>> = OnceLock::new();
pub(crate) fn app_config() -> Arc<config::Config> {
    APP_CONFIG
        .get()
        .expect("APP_CONFIG not initialized")
        .clone()
}

use crate::grpc::bird_service::BirdServiceImpl;
use crate::grpc::cluster_service::ClusterServiceImpl;
use crate::grpc::flap_service::FlapServiceImpl;
use crate::grpc::generated::bird_service_server::BirdServiceServer;
use crate::grpc::generated::cluster_service_server::ClusterServiceServer;
use crate::grpc::generated::flap_service_server::FlapServiceServer;
use crate::grpc::generated::management_service_server::ManagementServiceServer;
use crate::grpc::generated::peer_service_server::PeerServiceServer;
use crate::grpc::generated::settings_service_server::SettingsServiceServer;
use crate::grpc::management_service::ManagementServiceImpl;
use crate::grpc::peer_service::PeerServiceImpl;
use crate::grpc::settings_service::SettingsServiceImpl;

// ---------------------------------------------------------------------------

#[tokio::main]
#[allow(deprecated)] // tonic 0.12 into_router() — into_axum_router not available on Server::Router
async fn main() -> anyhow::Result<()> {
    let cli = config::Cli::parse();
    let mut cfg = config::Config::load(&cli.config)?;
    cfg.validate()?;

    // Generate JWT secret if not configured
    if cfg.auth.jwt_secret.is_empty() {
        cfg.auth.jwt_secret = auth::generate_jwt_secret();
        tracing::info!("Auto-generated JWT secret (tokens will expire on restart)");
    }
    // Auto-hash plaintext password if password_hash is not set
    if cfg.auth.password_hash.is_empty() && !cfg.auth.password.is_empty() {
        cfg.auth.password_hash = auth::password::hash_password(&cfg.auth.password)
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {e}"))?;
        tracing::info!(
            "Auto-hashed plaintext password. Consider replacing 'password' with 'password_hash' in config.toml"
        );
    } else if cfg.auth.password_hash.is_empty() && cfg.auth.password.is_empty() {
        anyhow::bail!("auth.password or auth.password_hash must be set in config.toml");
    }

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&cfg.logging.level))
        .init();

    tracing::info!("Starting peerman, listening on {}", cfg.server.listen_addr);

    let jwt_secret = Arc::new(cfg.auth.jwt_secret.clone());

    // Clone config values needed after cfg moves into Arc
    let listen_addr = cfg.server.listen_addr.clone();
    let db_path = cfg.storage.db_path.clone();
    let node_name = cfg.cluster.node_name.clone();
    let sync_interval = cfg.cluster.sync_interval_secs;
    let probe_interval = cfg.cluster.probe_interval_secs;
    let peer_nodes = cfg.cluster.peer_nodes.clone();
    let cluster_key = cfg.cluster.cluster_key.clone();
    let tunnel_ip_range = cfg.cluster.tunnel_ip_range.clone();
    let tunnel_ipv6_range = cfg.cluster.tunnel_ipv6_range.clone();
    let cfg_arc = Arc::new(cfg);
    APP_CONFIG
        .set(cfg_arc.clone())
        .map_err(|_| anyhow::anyhow!("APP_CONFIG already set"))?;

    // Database
    let pool = db::create_pool(&db_path).await?;
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
    let config_dirty = Arc::new(AtomicBool::new(false));
    let peer_svc = PeerServiceImpl {
        state: state.peer_state(),
        jwt_secret: jwt_secret.clone(),
        cluster_key: Arc::new(cluster_key.clone()),
        listen_addr: listen_addr.clone(),
        config_dirty: config_dirty.clone(),
    };
    let settings_svc = SettingsServiceImpl {
        settings_repo: state.settings_repo.clone(),
        jwt_secret: jwt_secret.clone(),
    };
    let cluster_svc = ClusterServiceImpl {
        node_repo: state.node_repo.clone(),
        peer_repo: state.peer_repo.clone(),
        probe_repo: state.probe_repo.clone(),
        community_repo: state.community_repo.clone(),
        settings_repo: state.settings_repo.clone(),
        jwt_secret: jwt_secret.clone(),
        cluster_key: Arc::new(cluster_key.clone()),
        listen_addr: listen_addr.clone(),
    };
    let bird_svc = BirdServiceImpl {
        node_name: node_name.clone(),
        jwt_secret: jwt_secret.clone(),
        cluster_key: Arc::new(cluster_key.clone()),
        node_repo: state.node_repo.clone(),
        cache: state.cluster_cache.clone(),
    };
    let flap_svc = FlapServiceImpl {
        flap_repo: state.flap_event_repo.clone(),
        jwt_secret: jwt_secret.clone(),
    };
    let mgmt_svc = ManagementServiceImpl {
        jwt_secret: jwt_secret.clone(),
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
        .add_service(ManagementServiceServer::new(mgmt_svc))
        .into_router();

    // Build axum router: auth endpoints + gRPC + static files
    let app = Router::new()
        .route("/health", get(crate::http::handlers::handle_health))
        .route("/api/auth/login", post(crate::http::handlers::handle_login))
        .route(
            "/api/auth/logout",
            post(crate::http::handlers::handle_logout),
        )
        .route("/api/auth/me", get(crate::http::handlers::handle_me))
        .nest("/api", grpc_router)
        .fallback(static_files::serve_static)
        .layer(TraceLayer::new_for_http());

    // Self-register if cluster mode is enabled
    if !node_name.is_empty() {
        let local_asn = state.settings_repo.load().await?.local_asn;
        let node = state
            .node_repo
            .upsert_self(&node_name, &listen_addr, local_asn)
            .await?;
        tracing::info!(
            "Self-registered as node '{}' (id={}, asn={})",
            node_name,
            node.id,
            local_asn
        );

        // Seed bootstrap peers into local nodes table
        for addr in &peer_nodes {
            let addr = addr.trim();
            if addr.is_empty() {
                continue;
            }
            if state.node_repo.find_by_listen_addr(addr).await?.is_none() {
                let name = format!("node-{}", addr.replace([':', '.'], "-"));
                let _ = state.node_repo.create(&name, addr, 0, "bootstrap").await;
            }
        }

        // Exchange node lists with bootstrap peers to discover the full cluster
        if !peer_nodes.is_empty() {
            let local_nodes = state.node_repo.list_all().await.unwrap_or_default();
            let my_info: Vec<crate::grpc::generated::NodeInfo> = local_nodes
                .iter()
                .map(|n| crate::grpc::generated::NodeInfo {
                    name: n.name.clone(),
                    listen_addr: n.listen_addr.clone(),
                    local_asn: n.local_asn,
                    description: n.description.clone().unwrap_or_default(),
                    last_seen_at: n.last_seen_at.clone(),
                    wg_public_key: String::new(),
                    tunnel_ip: String::new(),
                    tunnel_ipv6: String::new(),
                })
                .collect();

            for addr in &peer_nodes {
                let addr = addr.trim();
                if addr.is_empty() || addr == listen_addr.as_str() {
                    continue;
                }
                match crate::cluster::aggregator::ClusterAggregator::exchange_with(
                    addr,
                    &cluster_key,
                    my_info.clone(),
                )
                .await
                {
                    Ok(remote_nodes) => {
                        for info in &remote_nodes {
                            if info.listen_addr == listen_addr {
                                continue;
                            }
                            if state
                                .node_repo
                                .find_by_listen_addr(&info.listen_addr)
                                .await?
                                .is_some()
                            {
                                continue;
                            }
                            let _ = state
                                .node_repo
                                .create(
                                    &info.name,
                                    &info.listen_addr,
                                    info.local_asn,
                                    &info.description,
                                )
                                .await;
                        }

                        // Sync cluster configs after discovering new nodes
                        if !tunnel_ip_range.is_empty() {
                            let nodes = state.node_repo.list_all().await?;
                            let my_tunnel_ip = nodes
                                .iter()
                                .find(|n| n.listen_addr == listen_addr)
                                .and_then(|n| {
                                    if n.tunnel_ip.is_empty() {
                                        None
                                    } else {
                                        Some(n.tunnel_ip.clone())
                                    }
                                })
                                .unwrap_or_default();
                            if !my_tunnel_ip.is_empty() {
                                let _ =
                                    crate::cluster::tunnel::sync_cluster_wg(&state.node_repo, "")
                                        .await;
                                let settings = state.settings_repo.load().await?;
                                let _ = crate::cluster::tunnel::sync_cluster_bird(
                                    &state.peer_repo,
                                    &settings,
                                    &state.node_repo,
                                    &my_tunnel_ip,
                                )
                                .await;
                            }
                        }

                        tracing::info!(
                            "Discovered {} nodes from bootstrap peer {}",
                            remote_nodes.len(),
                            addr
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to exchange nodes with bootstrap peer {}: {}",
                            addr,
                            e
                        );
                    }
                }
            }
        }

        // Init cluster WG tunnel
        let _wg_private_key = if !tunnel_ip_range.is_empty() {
            match crate::cluster::tunnel::init_local_node(
                &state.node_repo,
                &node.id,
                &tunnel_ip_range,
                &tunnel_ipv6_range,
            )
            .await
            {
                Ok((priv_key, pub_key, tunnel_ip, tunnel_ipv6)) => {
                    if !tunnel_ipv6.is_empty() {
                        tracing::info!(
                            "Cluster tunnel initialized: key={}, ip={}, ipv6={}",
                            pub_key,
                            tunnel_ip,
                            tunnel_ipv6
                        );
                    } else {
                        tracing::info!(
                            "Cluster tunnel initialized: key={}, ip={}",
                            pub_key,
                            tunnel_ip
                        );
                    }

                    // Apply initial wg-cluster config
                    if let Err(e) =
                        crate::cluster::tunnel::sync_cluster_wg(&state.node_repo, &priv_key).await
                    {
                        tracing::warn!("Failed to apply initial wg-cluster config: {e}");
                    }

                    // Apply initial bird config with iBGP
                    let settings = state.settings_repo.load().await?;
                    if let Err(e) = crate::cluster::tunnel::sync_cluster_bird(
                        &state.peer_repo,
                        &settings,
                        &state.node_repo,
                        &tunnel_ip,
                    )
                    .await
                    {
                        tracing::warn!("Failed to apply initial cluster bird config: {e}");
                    }

                    priv_key
                }
                Err(e) => {
                    tracing::warn!("Cluster tunnel init failed: {e}");
                    String::new()
                }
            }
        } else {
            tracing::debug!("No tunnel_ip_range configured, skipping cluster WG tunnels");
            String::new()
        };

        // Spawn cluster background tasks
        tasks::cluster::ClusterTasks {
            node_name: node_name.clone(),
            node_id: node.id.clone(),
            listen_addr: listen_addr.clone(),
            cluster_key: cluster_key.clone(),
            sync_interval,
            probe_interval,
            tunnel_ip_range: tunnel_ip_range.clone(),
            state: state.clone(),
            shutdown: shutdown.clone(),
        }
        .spawn_all();

        // Spawn data retention cleanup task
        tasks::retention::spawn_retention_cleanup(pool.clone(), shutdown.clone());
    }

    // Spawn debounced WG+BIRD apply task (dirty-flag + 5s periodic check)
    tasks::apply::spawn_config_apply(
        config_dirty.clone(),
        state.peer_state(),
        listen_addr.clone(),
        pool.clone(),
        shutdown.clone(),
    );

    // Wire SIGINT to graceful shutdown
    let shutdown_signal = shutdown.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("Received shutdown signal");
        shutdown_signal.cancel();
    });

    let addr: SocketAddr = listen_addr.parse()?;
    tracing::info!("peerman ready at http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;

    tokio::select! {
        result = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()) => {
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
