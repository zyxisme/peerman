mod app_state;
mod auth;
mod cluster;
mod config;
mod db;
mod error;
mod grpc;
mod models;
mod services;
mod static_files;

use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::http::header::SET_COOKIE;
use axum::routing::{get, post};
use axum::Json;
use axum::Router;
use clap::Parser;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

/// Global config (set once at startup, read by HTTP handlers).
static APP_CONFIG: OnceLock<Arc<config::Config>> = OnceLock::new();
fn app_config() -> Arc<config::Config> {
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
// Auth HTTP handlers (not gRPC)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<UserInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct UserInfo {
    username: String,
}

#[derive(Serialize)]
struct MeResponse {
    authenticated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
}

async fn handle_login(Json(req): Json<LoginRequest>) -> axum::response::Response {
    let cfg = app_config();
    if req.username != cfg.auth.username {
        return json_response(
            axum::http::StatusCode::UNAUTHORIZED,
            &LoginResponse {
                success: false,
                user: None,
                error: Some("Invalid credentials".into()),
            },
            None,
        );
    }

    let password_ok = if cfg.auth.password_hash.is_empty() {
        tracing::warn!("Using plaintext password comparison — set password_hash in config");
        req.password == cfg.auth.password
    } else {
        auth::password::verify_password(&req.password, &cfg.auth.password_hash)
            .unwrap_or(false)
    };

    if !password_ok {
        return json_response(
            axum::http::StatusCode::UNAUTHORIZED,
            &LoginResponse {
                success: false,
                user: None,
                error: Some("Invalid credentials".into()),
            },
            None,
        );
    }

    let secret = if cfg.auth.jwt_secret.is_empty() {
        ""
    } else {
        &cfg.auth.jwt_secret
    };

    match auth::create_token(&req.username, secret) {
        Ok(token) => {
            let cookie = format!(
                "jwt={}; HttpOnly; SameSite=Strict; Path=/; Max-Age=3600",
                token
            );
            json_response(
                axum::http::StatusCode::OK,
                &LoginResponse {
                    success: true,
                    user: Some(UserInfo {
                        username: req.username,
                    }),
                    error: None,
                },
                Some(&cookie),
            )
        }
        Err(e) => json_response(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            &LoginResponse {
                success: false,
                user: None,
                error: Some(format!("Token creation failed: {e}")),
            },
            None,
        ),
    }
}

async fn handle_logout() -> axum::response::Response {
    json_response(
        axum::http::StatusCode::OK,
        &serde_json::json!({"success": true}),
        Some("jwt=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0"),
    )
}

async fn handle_me(headers: axum::http::HeaderMap) -> Json<MeResponse> {
    let cfg = app_config();
    let cookie_header = headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    match auth::parse_cookie(cookie_header, "jwt") {
        Some(token) => match auth::verify_token(token, &cfg.auth.jwt_secret) {
            Ok(claims) => Json(MeResponse {
                authenticated: true,
                username: Some(claims.sub),
            }),
            Err(_) => Json(MeResponse {
                authenticated: false,
                username: None,
            }),
        },
        None => Json(MeResponse {
            authenticated: false,
            username: None,
        }),
    }
}

fn json_response(
    status: axum::http::StatusCode,
    body: &impl Serialize,
    cookie: Option<&str>,
) -> axum::response::Response {
    let json = serde_json::to_string(body).unwrap_or_default();
    let mut builder = axum::response::Response::builder()
        .status(status)
        .header("content-type", "application/json");
    if let Some(c) = cookie {
        builder = builder.header(SET_COOKIE, c);
    }
    builder
        .body(axum::body::Body::from(json))
        .expect("body is infallible")
}

// ---------------------------------------------------------------------------

#[tokio::main]
#[allow(deprecated)] // tonic 0.12 into_router() — into_axum_router not available on Server::Router
async fn main() -> anyhow::Result<()> {
    let cli = config::Cli::parse();
    let mut cfg = config::Config::load(&cli.config)?;

    // Generate JWT secret if not configured
    if cfg.auth.jwt_secret.is_empty() {
        cfg.auth.jwt_secret = auth::generate_jwt_secret();
        tracing::info!("Auto-generated JWT secret (tokens will expire on restart)");
    }
    // Auto-hash plaintext password if password_hash is not set
    if cfg.auth.password_hash.is_empty() && !cfg.auth.password.is_empty() {
        cfg.auth.password_hash = auth::password::hash_password(&cfg.auth.password)
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {e}"))?;
        tracing::info!("Auto-hashed plaintext password. Consider replacing 'password' with 'password_hash' in config.toml");
    } else if cfg.auth.password_hash.is_empty() && cfg.auth.password.is_empty() {
        anyhow::bail!(
            "auth.password or auth.password_hash must be set in config.toml"
        );
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
    let peer_svc = PeerServiceImpl {
        peer_repo: state.peer_repo.clone(),
        settings_repo: state.settings_repo.clone(),
        jwt_secret: jwt_secret.clone(),
        node_repo: state.node_repo.clone(),
        cluster_key: Arc::new(cluster_key.clone()),
        listen_addr: listen_addr.clone(),
        pool: pool.clone(),
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
        .route("/api/auth/login", post(handle_login))
        .route("/api/auth/logout", post(handle_logout))
        .route("/api/auth/me", get(handle_me))
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

        // Spawn periodic stale-node cleanup task
        let stale_state = state.clone();
        let stale_interval = sync_interval;
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

        // Spawn flap-suppressed health check + ICMP probe task
        if probe_interval > 0 {
            let probe_ct = shutdown.clone();
            let probe_interval_dur = Duration::from_secs(probe_interval);
            let node_repo_probe = state.node_repo.clone();
            let probe_repo_probe = state.probe_repo.clone();
            let node_name_probe = node_name.clone();
            let cluster_key_probe = cluster_key.clone();
            let cluster_cache = state.cluster_cache.clone();

            tokio::spawn(async move {
                let mut fail_streaks: std::collections::HashMap<String, u32> =
                    std::collections::HashMap::new();
                let mut interval = tokio::time::interval(probe_interval_dur);
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                loop {
                    tokio::select! {
                        _ = probe_ct.cancelled() => break,
                        _ = interval.tick() => {}
                    }

                    let nodes = match node_repo_probe.list_all().await {
                        Ok(n) => n,
                        Err(e) => {
                            tracing::warn!("Failed to list nodes for health check: {e}");
                            continue;
                        }
                    };

                    let local = nodes.iter().find(|n| n.name == node_name_probe);

                    for node in &nodes {
                        if node.name == node_name_probe {
                            continue;
                        }

                        let healthy = crate::cluster::aggregator::ClusterAggregator::health_check(
                            &node.listen_addr,
                            &cluster_key_probe,
                        )
                        .await;

                        let prev_fails = fail_streaks.get(&node.listen_addr).copied().unwrap_or(0);

                        if healthy {
                            if prev_fails >= 2 {
                                let _ = node_repo_probe.mark_online(&node.id).await;
                                cluster_cache.invalidate(&node.listen_addr).await;
                                tracing::info!(
                                    "Node {} ({}) is back online",
                                    node.name,
                                    node.listen_addr
                                );
                            }
                            fail_streaks.insert(node.listen_addr.clone(), 0);

                            // Also run ICMP probe for latency data
                            if let Some(local_node) = local {
                                let _ = crate::services::probe::probe_between(
                                    local_node,
                                    node,
                                    &probe_repo_probe,
                                )
                                .await;
                            }
                        } else {
                            let new_fails = prev_fails + 1;
                            fail_streaks.insert(node.listen_addr.clone(), new_fails);

                            if new_fails >= 2 && prev_fails < 2 {
                                let _ = node_repo_probe.mark_stale_node(&node.id).await;
                                cluster_cache.mark_stale(&node.listen_addr).await;
                                tracing::warn!(
                                    "Node {} ({}) went offline after {} consecutive failures",
                                    node.name,
                                    node.listen_addr,
                                    new_fails
                                );
                            }
                        }
                    }
                }
            });
        }

        // Spawn periodic anti-entropy node exchange task
        let sync_ct = shutdown.clone();
        let sync_interval_dur = Duration::from_secs(sync_interval);
        let node_repo_sync = state.node_repo.clone();
        let cluster_key_sync = cluster_key.clone();
        let listen_addr_sync = listen_addr.clone();
        let tunnel_ip_range_sync = tunnel_ip_range.clone();
        let settings_repo_sync = state.settings_repo.clone();
        let peer_repo_sync = state.peer_repo.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(sync_interval_dur);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = sync_ct.cancelled() => break,
                    _ = interval.tick() => {}
                }

                let nodes = match node_repo_sync.list_all().await {
                    Ok(n) => n,
                    Err(_) => continue,
                };

                let online_peers: Vec<_> = nodes
                    .iter()
                    .filter(|n| n.online && n.listen_addr != listen_addr_sync)
                    .collect();

                if online_peers.is_empty() {
                    continue;
                }

                // Pick a random peer
                let idx = {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default();
                    (now.subsec_nanos() as usize) % online_peers.len()
                };
                let peer = online_peers[idx];

                let my_info: Vec<crate::grpc::generated::NodeInfo> = nodes
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

                match crate::cluster::aggregator::ClusterAggregator::exchange_with(
                    &peer.listen_addr,
                    &cluster_key_sync,
                    my_info,
                )
                .await
                {
                    Ok(remote_nodes) => {
                        for info in &remote_nodes {
                            if info.listen_addr == listen_addr_sync {
                                continue;
                            }
                            if let Ok(Some(_)) =
                                node_repo_sync.find_by_listen_addr(&info.listen_addr).await
                            {
                                continue;
                            }
                            let _ = node_repo_sync
                                .create(
                                    &info.name,
                                    &info.listen_addr,
                                    info.local_asn,
                                    &info.description,
                                )
                                .await;
                        }

                        // After discovering new nodes, sync cluster configs
                        if !tunnel_ip_range_sync.is_empty() {
                            let nodes = match node_repo_sync.list_all().await {
                                Ok(n) => n,
                                Err(_) => continue,
                            };
                            let my_tunnel_ip = nodes
                                .iter()
                                .find(|n| n.listen_addr == listen_addr_sync)
                                .and_then(|n| {
                                    if n.tunnel_ip.is_empty() {
                                        None
                                    } else {
                                        Some(n.tunnel_ip.clone())
                                    }
                                })
                                .unwrap_or_default();
                            if !my_tunnel_ip.is_empty() {
                                if let Ok(settings) = settings_repo_sync.load().await {
                                    let _ = crate::cluster::tunnel::sync_cluster_bird(
                                        &peer_repo_sync,
                                        &settings,
                                        &node_repo_sync,
                                        &my_tunnel_ip,
                                    )
                                    .await;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::debug!(
                            "Periodic ExchangeNodes with {} failed: {}",
                            peer.listen_addr,
                            e
                        );
                    }
                }
            }
        });

        // Spawn BGP flap detector
        let node_id = node.id.clone();
        let flap_node_name = node_name.clone();
        let flap_repo = state.flap_event_repo.clone();
        let flap_token = shutdown.clone();

        tokio::spawn(async move {
            tracing::info!("Starting BGP flap detector for node '{flap_node_name}' ({node_id})");

            let (tx, rx) =
                tokio::sync::mpsc::channel::<crate::services::bgp_listener::PathChange>(1024);

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

                    let mut detector = crate::services::flap_detector::FlapDetector::new(
                        node_id.clone(),
                        flap_repo,
                    );
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

    let addr: SocketAddr = listen_addr.parse()?;
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
