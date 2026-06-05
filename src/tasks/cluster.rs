use std::collections::HashMap;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::app_state::AppState;

/// Holds all dependencies needed to spawn cluster background tasks.
pub struct ClusterTasks {
    pub node_name: String,
    pub node_id: String,
    pub listen_addr: String,
    pub cluster_key: String,
    pub sync_interval: u64,
    pub probe_interval: u64,
    pub tunnel_ip_range: String,
    pub state: AppState,
    pub shutdown: CancellationToken,
}

impl ClusterTasks {
    /// Spawn all cluster background tasks.
    pub fn spawn_all(self) {
        self.spawn_stale_cleanup();
        self.spawn_health_check();
        self.spawn_anti_entropy();
        self.spawn_flap_detector();
    }

    /// Periodic task that marks nodes as stale after 120s without contact.
    fn spawn_stale_cleanup(&self) {
        let state = self.state.clone();
        let interval = self.sync_interval;
        let token = self.shutdown.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        tracing::info!("Stale-node cleanup task shutting down");
                        return;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(interval)) => {}
                }
                if let Err(e) = state.node_repo.mark_stale(120).await {
                    tracing::warn!("Failed to mark stale nodes: {}", e);
                }
            }
        });
    }

    /// Flap-suppressed health check with ICMP probe for latency data.
    fn spawn_health_check(&self) {
        if self.probe_interval == 0 {
            return;
        }

        let token = self.shutdown.clone();
        let interval_dur = Duration::from_secs(self.probe_interval);
        let node_repo = self.state.node_repo.clone();
        let probe_repo = self.state.probe_repo.clone();
        let node_name = self.node_name.clone();
        let cluster_key = self.cluster_key.clone();
        let cluster_cache = self.state.cluster_cache.clone();

        tokio::spawn(async move {
            let mut fail_streaks: HashMap<String, u32> = HashMap::new();
            let mut interval = tokio::time::interval(interval_dur);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    _ = interval.tick() => {}
                }

                let nodes = match node_repo.list_all().await {
                    Ok(n) => n,
                    Err(e) => {
                        tracing::warn!("Failed to list nodes for health check: {e}");
                        continue;
                    }
                };

                let local = nodes.iter().find(|n| n.name == node_name);

                for node in &nodes {
                    if node.name == node_name {
                        continue;
                    }

                    let healthy = crate::cluster::aggregator::ClusterAggregator::health_check(
                        &node.listen_addr,
                        &cluster_key,
                    )
                    .await;

                    let prev_fails = fail_streaks.get(&node.listen_addr).copied().unwrap_or(0);

                    if healthy {
                        if prev_fails >= 2 {
                            let _ = node_repo.mark_online(&node.id).await;
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
                                &probe_repo,
                            )
                            .await;
                        }
                    } else {
                        let new_fails = prev_fails + 1;
                        fail_streaks.insert(node.listen_addr.clone(), new_fails);

                        if new_fails >= 2 && prev_fails < 2 {
                            let _ = node_repo.mark_stale_node(&node.id).await;
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

    /// Periodic anti-entropy task that exchanges node lists with a random online peer.
    fn spawn_anti_entropy(&self) {
        let token = self.shutdown.clone();
        let interval_dur = Duration::from_secs(self.sync_interval);
        let node_repo = self.state.node_repo.clone();
        let cluster_key = self.cluster_key.clone();
        let listen_addr = self.listen_addr.clone();
        let tunnel_ip_range = self.tunnel_ip_range.clone();
        let settings_repo = self.state.settings_repo.clone();
        let peer_repo = self.state.peer_repo.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval_dur);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    _ = interval.tick() => {}
                }

                let nodes = match node_repo.list_all().await {
                    Ok(n) => n,
                    Err(_) => continue,
                };

                let online_peers: Vec<_> = nodes
                    .iter()
                    .filter(|n| n.online && n.listen_addr != listen_addr)
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
                    &cluster_key,
                    my_info,
                )
                .await
                {
                    Ok(remote_nodes) => {
                        for info in &remote_nodes {
                            if info.listen_addr == listen_addr {
                                continue;
                            }
                            if let Ok(Some(_)) =
                                node_repo.find_by_listen_addr(&info.listen_addr).await
                            {
                                continue;
                            }
                            let _ = node_repo
                                .create(
                                    &info.name,
                                    &info.listen_addr,
                                    info.local_asn,
                                    &info.description,
                                )
                                .await;
                        }

                        // After discovering new nodes, sync cluster configs
                        if !tunnel_ip_range.is_empty() {
                            let nodes = match node_repo.list_all().await {
                                Ok(n) => n,
                                Err(_) => continue,
                            };
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
                            if !my_tunnel_ip.is_empty()
                                && let Ok(settings) = settings_repo.load().await
                            {
                                let _ = crate::cluster::tunnel::sync_cluster_bird(
                                    &peer_repo,
                                    &settings,
                                    &node_repo,
                                    &my_tunnel_ip,
                                )
                                .await;
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
    }

    /// BGP flap detector using iBGP listener with socket polling fallback.
    fn spawn_flap_detector(&self) {
        let node_id = self.node_id.clone();
        let flap_node_name = self.node_name.clone();
        let flap_repo = self.state.flap_event_repo.clone();
        let token = self.shutdown.clone();

        tokio::spawn(async move {
            tracing::info!("Starting BGP flap detector for node '{flap_node_name}' ({node_id})");

            let (tx, rx) =
                tokio::sync::mpsc::channel::<crate::services::bgp_listener::PathChange>(1024);

            match crate::services::bgp_listener::BgpListener::bind(node_id.clone()).await {
                Ok(listener) => {
                    tracing::info!("iBGP listener active on [::1]:1790");
                    let bgp_tx = tx.clone();
                    let bgp_token = token.clone();
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
                    detector.run(rx, token).await;
                }
                Err(e) => {
                    tracing::warn!(
                        "iBGP listener unavailable ({e}), flap detection will use socket polling fallback"
                    );
                    let _keep_tx = tx; // Keep channel alive so rx doesn't close
                    let mut detector =
                        crate::services::flap_detector::FlapDetector::new(node_id, flap_repo);
                    detector.run(rx, token).await;
                }
            }
        });
    }
}
