use std::net::{Ipv4Addr, Ipv6Addr};
use std::os::unix::fs::PermissionsExt;
use std::str::FromStr;

use crate::error::AppError;
use crate::models::node::NodeRepository;
use crate::services;

const CLUSTER_WG_INTERFACE: &str = "wg-cluster";
const CLUSTER_WG_PORT: u16 = 51821;

/// Ensure this node has a WG keypair and a tunnel IP assigned.
pub async fn init_local_node(
    node_repo: &NodeRepository,
    node_id: &str,
    tunnel_ip_range: &str,
    tunnel_ipv6_range: &str,
) -> Result<(String, String, String, String), AppError> {
    let node = node_repo.find_by_id(node_id).await?;

    // Only generate keypair if node doesn't have one persisted
    let (priv_key, pub_key) = if node.wg_pubkey.is_empty() || node.wg_private_key.is_empty() {
        let (pk, pubk) = services::wireguard::generate_keypair();
        node_repo
            .update_cluster_fields(node_id, &pubk, &node.tunnel_ip)
            .await?;
        node_repo.update_wg_private_key(node_id, &pk).await?;
        (pk, pubk)
    } else {
        (node.wg_private_key.clone(), node.wg_pubkey.clone())
    };

    // Assign tunnel IP if missing
    let tunnel_ip = if node.tunnel_ip.is_empty() {
        let ip = assign_tunnel_ip(node_repo, tunnel_ip_range).await?;
        node_repo
            .update_cluster_fields(node_id, &pub_key, &ip)
            .await?;
        ip
    } else {
        node.tunnel_ip.clone()
    };

    // Assign IPv6 tunnel IP if missing and range is configured
    let tunnel_ipv6 = if node.tunnel_ipv6.is_empty() && !tunnel_ipv6_range.is_empty() {
        let ip = assign_tunnel_ipv6(node_repo, tunnel_ipv6_range).await?;
        node_repo.update_tunnel_ipv6(node_id, &ip).await?;
        ip
    } else {
        node.tunnel_ipv6.clone()
    };

    Ok((priv_key, pub_key, tunnel_ip, tunnel_ipv6))
}

/// Assign the first unused IP from the given range (e.g. "10.255.0.0/24").
async fn assign_tunnel_ip(node_repo: &NodeRepository, range: &str) -> Result<String, AppError> {
    let (base, prefix_len) = range
        .split_once('/')
        .ok_or_else(|| AppError::Internal("invalid tunnel_ip_range format".into()))?;

    let base_ip = Ipv4Addr::from_str(base)
        .map_err(|e| AppError::Internal(format!("invalid tunnel_ip_range base: {e}")))?;
    let prefix_len: u8 = prefix_len
        .parse()
        .map_err(|_| AppError::Internal("invalid tunnel_ip_range prefix".into()))?;

    let base_u32 = u32::from(base_ip);
    let mask = !((1u32 << (32 - prefix_len)) - 1);
    let network = base_u32 & mask;
    let broadcast = network | !mask;

    let all_nodes = node_repo.list_all().await?;
    let used_ips: std::collections::HashSet<String> = all_nodes
        .iter()
        .filter_map(|n| {
            if n.tunnel_ip.is_empty() {
                None
            } else {
                Some(n.tunnel_ip.clone())
            }
        })
        .collect();

    for offset in 1..(broadcast - network) {
        let candidate = Ipv4Addr::from(network + offset);
        let candidate_str = candidate.to_string();
        if !used_ips.contains(&candidate_str) {
            return Ok(candidate_str);
        }
    }

    Err(AppError::Internal(
        "no available IP in tunnel_ip_range".into(),
    ))
}

/// Assign the first unused IPv6 address from the given range (e.g. "fd00:cluster::/64").
async fn assign_tunnel_ipv6(node_repo: &NodeRepository, range: &str) -> Result<String, AppError> {
    let (base, prefix_len) = range
        .split_once('/')
        .ok_or_else(|| AppError::Internal("invalid tunnel_ipv6_range format".into()))?;

    let base_ip = Ipv6Addr::from_str(base)
        .map_err(|e| AppError::Internal(format!("invalid tunnel_ipv6_range base: {e}")))?;
    let prefix_len: u8 = prefix_len
        .parse()
        .map_err(|_| AppError::Internal("invalid tunnel_ipv6_range prefix".into()))?;

    let base_u128 = u128::from(base_ip);
    let mask = !((1u128 << (128 - prefix_len)) - 1);
    let network = base_u128 & mask;
    let broadcast = network | !mask;

    let all_nodes = node_repo.list_all().await?;
    let used_ips: std::collections::HashSet<String> = all_nodes
        .iter()
        .filter_map(|n| {
            if n.tunnel_ipv6.is_empty() {
                None
            } else {
                Some(n.tunnel_ipv6.clone())
            }
        })
        .collect();

    // Skip network address (offset 0), start from offset 1
    // Limit iteration to avoid huge ranges
    let max_hosts = std::cmp::min(broadcast - network, 65536);
    for offset in 1..max_hosts {
        let candidate = Ipv6Addr::from(network + offset);
        let candidate_str = candidate.to_string();
        if !used_ips.contains(&candidate_str) {
            return Ok(candidate_str);
        }
    }

    Err(AppError::Internal(
        "no available IPv6 in tunnel_ipv6_range".into(),
    ))
}

/// Rebuild wg-cluster config and write it to /etc/wireguard/.
pub async fn sync_cluster_wg(
    node_repo: &NodeRepository,
    my_wg_private_key: &str,
) -> Result<(), AppError> {
    let nodes = node_repo.list_all().await?;

    let config =
        services::wireguard::generate_cluster_wg_config(&nodes, my_wg_private_key, CLUSTER_WG_PORT);

    let config_path = format!("/etc/wireguard/{CLUSTER_WG_INTERFACE}.conf");
    let tmp_path = format!("{config_path}.tmp");

    std::fs::write(&tmp_path, config)
        .map_err(|e| AppError::Internal(format!("Cannot write wg-cluster config: {e}")))?;
    std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600)).map_err(|e| {
        AppError::Internal(format!("Cannot set permissions on wg-cluster config: {e}"))
    })?;
    std::fs::rename(&tmp_path, &config_path)
        .map_err(|e| AppError::Internal(format!("Cannot rename wg-cluster config: {e}")))?;

    // Apply via wg syncconf
    services::wireguard::apply_syncconf(CLUSTER_WG_INTERFACE, &config_path).await
}

/// Rebuild bird.conf (full config with iBGP) and apply it.
pub async fn sync_cluster_bird(
    peer_repo: &crate::models::peer::PeerRepository,
    settings: &crate::models::settings::Settings,
    node_repo: &NodeRepository,
    my_tunnel_ip: &str,
) -> Result<(), AppError> {
    let peers = peer_repo.list_all().await?;
    let nodes = node_repo.list_all().await?;

    let mut config = services::bird::generate_full_config(
        &peers,
        settings,
        "",
        &std::collections::HashMap::new(),
    );

    let ibgp = services::bird::generate_ibgp_blocks(&nodes, settings, my_tunnel_ip);
    config.push_str(&ibgp);

    services::bird::apply_config(&config)
}
