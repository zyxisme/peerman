use base64::Engine;
use rand::RngCore;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::error::AppError;
use crate::models::peer::Peer;

/// Generate a new WireGuard keypair.
/// Returns (private_key_base64, public_key_base64).
pub fn generate_keypair() -> (String, String) {
    let mut rng = rand::thread_rng();
    let mut private_bytes = [0u8; 32];
    rng.fill_bytes(&mut private_bytes);

    let secret = StaticSecret::from(private_bytes);
    let public = PublicKey::from(&secret);

    let private_b64 = base64::engine::general_purpose::STANDARD.encode(secret.as_bytes());
    let public_b64 = base64::engine::general_purpose::STANDARD.encode(public.as_bytes());

    (private_b64, public_b64)
}

/// Apply WG config using wg syncconf — differential update without down/up.
pub fn apply_syncconf(interface: &str, config_path: &str) -> Result<(), AppError> {
    let status = std::process::Command::new("wg")
        .args(["syncconf", interface, config_path])
        .output()
        .map_err(|e| AppError::Internal(format!("wg syncconf failed: {e}")))?;

    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr);
        return Err(AppError::Internal(format!("wg syncconf error: {stderr}")));
    }
    Ok(())
}

/// Restart a WireGuard interface (down + up via wg-quick).
pub fn restart_interface(iface: &str) -> Result<(), AppError> {
    let _ = std::process::Command::new("wg-quick")
        .args(["down", iface])
        .output(); // Ignore error on down (interface may not be up)

    let output = std::process::Command::new("wg-quick")
        .args(["up", iface])
        .output()
        .map_err(|e| AppError::Internal(format!("wg-quick up failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Internal(
            format!("wg-quick up {iface} failed: {stderr}")
        ));
    }
    Ok(())
}

/// Parse `wg show <interface> dump` output into structured status.
pub fn get_wg_status(interface: &str) -> Result<Vec<crate::grpc::generated::WgInterface>, AppError> {
    let output = std::process::Command::new("wg")
        .args(["show", interface, "dump"])
        .output()
        .map_err(|e| AppError::Internal(format!("wg show failed: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut interfaces: Vec<crate::grpc::generated::WgInterface> = Vec::new();
    let mut current_iface: Option<crate::grpc::generated::WgInterface> = None;

    for line in stdout.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.is_empty() {
            continue;
        }
        match fields[0] {
            // Interface line: private_key, public_key, listen_port, fwmark
            key if fields.len() >= 4 && !key.is_empty() => {
                if let Some(iface) = current_iface.take() {
                    interfaces.push(iface);
                }
                current_iface = Some(crate::grpc::generated::WgInterface {
                    name: interface.to_string(),
                    public_key: fields[1].to_string(),
                    listen_port: fields[2].parse().unwrap_or(0),
                    peers: Vec::new(),
                });
            }
            // Peer line: public_key, preshared_key, endpoint, allowed_ips, latest_handshake, transfer_rx, transfer_tx, keepalive
            peer_key if fields.len() >= 8 => {
                if let Some(ref mut iface) = current_iface {
                    iface.peers.push(crate::grpc::generated::WgPeerStatus {
                        public_key: fields[0].to_string(),
                        endpoint: fields.get(2).map(|s| s.to_string()).unwrap_or_default(),
                        allowed_ips: fields.get(3).map(|s| s.to_string()).unwrap_or_default(),
                        latest_handshake: fields.get(4).map(|s| s.to_string()).unwrap_or_default(),
                        transfer_rx: fields.get(5).map(|s| s.to_string()).unwrap_or_default(),
                        transfer_tx: fields.get(6).map(|s| s.to_string()).unwrap_or_default(),
                    });
                }
            }
            _ => {}
        }
    }
    if let Some(iface) = current_iface {
        interfaces.push(iface);
    }
    Ok(interfaces)
}

/// Generate WG config for the cluster interconnect interface (wg-cluster).
pub fn generate_cluster_wg_config(
    nodes: &[crate::models::node::Node],
    private_key: &str,
    listen_port: u16,
) -> String {
    let mut config = String::new();

    config.push_str("[Interface]\n");
    config.push_str(&format!("PrivateKey = {private_key}\n"));
    config.push_str(&format!("ListenPort = {listen_port}\n"));
    config.push_str("Table = off\n\n");

    for node in nodes {
        if node.wg_pubkey.is_empty() {
            continue;
        }
        config.push_str("[Peer]\n");
        config.push_str(&format!("PublicKey = {}\n", node.wg_pubkey));
        // Extract host from listen_addr (strip port, keep host)
        let host = node.listen_addr.rsplit_once(':')
            .map(|(h, _)| h)
            .unwrap_or(&node.listen_addr);
        config.push_str(&format!("Endpoint = {host}:{listen_port}\n"));
        let mut allowed = format!("{}/32", node.tunnel_ip);
        if !node.tunnel_ipv6.is_empty() {
            allowed.push_str(&format!(", {}/128", node.tunnel_ipv6));
        }
        config.push_str(&format!("AllowedIPs = {allowed}\n"));
        config.push_str("PersistentKeepalive = 25\n\n");
    }

    config
}

/// Generate a complete WireGuard configuration for a single peer.
/// Returns the INI-format config string.
pub fn generate_config(peer: &Peer, settings: &crate::models::settings::Settings) -> String {
    let mut config = String::new();

    // [Interface] section
    config.push_str("[Interface]\n");
    if let Some(ref key) = peer.wg_private_key {
        config.push_str(&format!("PrivateKey = {key}\n"));
    }
    config.push_str(&format!("ListenPort = {}\n", peer.wg_listen_port));
    config.push_str(&format!("Table = {}\n", settings.wg_table));

    // MTU (only if > 0)
    if settings.wg_mtu > 0 {
        config.push_str(&format!("MTU = {}\n", settings.wg_mtu));
    }

    // FwMark (only if > 0)
    if settings.wg_fwmark > 0 {
        config.push_str(&format!("FwMark = {}\n", settings.wg_fwmark));
    }

    // Build Address line from tunnel IPs
    let mut addresses = Vec::new();
    if let Some(ref ipv4) = peer.ipv4_tunnel_local {
        addresses.push(format!("{ipv4}/32"));
    }
    if let Some(ref ipv6) = peer.ipv6_tunnel_local {
        addresses.push(format!("{ipv6}/128"));
    }
    if !addresses.is_empty() {
        config.push_str(&format!("Address = {}\n", addresses.join(", ")));
    }

    // PostUp — auto-generate + user custom
    let mut post_up = String::from(
        "PostUp = ip link set %i up; sysctl -w net.ipv6.conf.%i.autoconf=0"
    );
    if let Some(ref ipv4) = peer.ipv4_tunnel_local {
        post_up.push_str(&format!("; ip addr add {ipv4}/32 dev %i"));
    }
    if let Some(ref ipv6) = peer.ipv6_tunnel_local {
        post_up.push_str(&format!("; ip addr add {ipv6}/128 dev %i"));
    }
    if !settings.wg_post_up.is_empty() {
        post_up.push_str(&format!("; {}", settings.wg_post_up));
    }
    config.push_str(&format!("{post_up}\n"));

    // PostDown — mirror
    let mut post_down = String::from("PostDown = ip link set %i down");
    if !settings.wg_post_down.is_empty() {
        post_down.push_str(&format!("; {}", settings.wg_post_down));
    }
    config.push_str(&format!("{post_down}\n"));

    config.push('\n');

    // [Peer] section
    config.push_str("[Peer]\n");
    if let Some(ref key) = peer.wg_public_key {
        config.push_str(&format!("PublicKey = {key}\n"));
    }
    config.push_str(&format!(
        "Endpoint = {}:{}\n",
        peer.wg_remote_address, peer.wg_remote_port
    ));

    // AllowedIPs — full DN42 prefixes + link-local
    let allowed_ips = format!(
        "{}, {}, fe80::/10",
        settings.dn42_ipv4_prefix, settings.dn42_ipv6_prefix
    );
    config.push_str(&format!("AllowedIPs = {allowed_ips}\n"));

    config.push_str("PersistentKeepalive = 25\n");

    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::settings::Settings;

    fn test_settings() -> Settings {
        Settings {
            local_asn: 4242420000,
            bird_template_name: "test".into(),
            bird_router_id: "1.2.3.4".into(),
            wg_default_listen_port: 42420,
            dn42_ipv4_prefix: "172.20.0.0/14".into(),
            dn42_ipv6_prefix: "fd00::/8".into(),
            wg_table: "off".into(),
            wg_mtu: 1420,
            wg_fwmark: 0,
            wg_post_up: String::new(),
            wg_post_down: String::new(),
            roa_mode: "none".into(),
            roa_static_v4_url: String::new(),
            roa_static_v6_url: String::new(),
            roa_rtr_address: String::new(),
            roa_rtr_port: 323,
            bird_import_limit: 9000,
            bird_export_filter: String::new(),
            bird_import_filter: String::new(),
            enable_community_filters: false,
            enable_bfd: false,
            bfd_interval_ms: 300,
            bfd_multiplier: 3,
            cluster_tunnel_ipv6_range: String::new(),
            enable_confederation: false,
            confederation_local_asn: 0,
        }
    }

    fn test_peer() -> Peer {
        Peer {
            id: "test-id".into(),
            name: "test-peer".into(),
            description: None,
            asn: 4242420001,
            local_asn: 4242420000,
            wg_private_key: Some("privkey".into()),
            wg_public_key: Some("pubkey".into()),
            wg_remote_address: "10.0.0.1".into(),
            wg_remote_port: 42420,
            wg_listen_port: 42420,
            wg_interface_name: "wg0".into(),
            ipv4_tunnel_local: Some("172.20.1.1".into()),
            ipv4_tunnel_remote: Some("172.20.1.2".into()),
            ipv6_tunnel_local: Some("fd00::1".into()),
            ipv6_tunnel_remote: Some("fd00::2".into()),
            multiprotocol: true,
            extended_nexthop: true,
            sessions: 0,
            passive: false,
            import_max_prefix: None,
            export_max_prefix: None,
            enabled: true,
            created_at: "2025-01-01T00:00:00Z".into(),
            updated_at: "2025-01-01T00:00:00Z".into(),
            origin_node_id: None,
        }
    }

    #[test]
    fn test_generate_keypair_format() {
        let (priv_key, pub_key) = generate_keypair();
        assert_eq!(priv_key.len(), 44);
        assert_eq!(pub_key.len(), 44);
        // Should be valid base64
        use base64::Engine;
        assert!(base64::engine::general_purpose::STANDARD.decode(&priv_key).is_ok());
        assert!(base64::engine::general_purpose::STANDARD.decode(&pub_key).is_ok());
    }

    #[test]
    fn test_generate_config_contains_sections() {
        let config = generate_config(&test_peer(), &test_settings());
        assert!(config.contains("[Interface]"));
        assert!(config.contains("[Peer]"));
        assert!(config.contains("PrivateKey = privkey"));
        assert!(config.contains("PublicKey = pubkey"));
        assert!(config.contains("Table = off"));
        assert!(config.contains("MTU = 1420"));
        assert!(config.contains("PostUp ="));
        assert!(config.contains("PostDown ="));
        assert!(config.contains("PersistentKeepalive = 25"));
        assert!(config.contains("fe80::/10"));
    }

    #[test]
    fn test_generate_config_fwmark_omitted_when_zero() {
        let config = generate_config(&test_peer(), &test_settings());
        assert!(!config.contains("FwMark"));
    }

    #[test]
    fn test_generate_config_fwmark_included_when_set() {
        let mut s = test_settings();
        s.wg_fwmark = 51820;
        let config = generate_config(&test_peer(), &s);
        assert!(config.contains("FwMark = 51820"));
    }

    #[test]
    fn test_generate_config_post_up_has_tunnel_ips() {
        let config = generate_config(&test_peer(), &test_settings());
        assert!(config.contains("172.20.1.1/32"));
        assert!(config.contains("fd00::1/128"));
    }

    #[test]
    fn test_generate_config_custom_post_up_appended() {
        let mut s = test_settings();
        s.wg_post_up = "ip route add 10.0.0.0/8 via 172.20.1.2".into();
        let config = generate_config(&test_peer(), &s);
        assert!(config.contains("ip route add 10.0.0.0/8 via 172.20.1.2"));
    }

    #[test]
    fn test_generate_config_without_keys() {
        let mut peer = test_peer();
        peer.wg_private_key = None;
        peer.wg_public_key = None;
        let config = generate_config(&peer, &test_settings());
        assert!(!config.contains("PrivateKey"));
        assert!(!config.contains("PublicKey"));
    }

    #[test]
    fn test_generate_cluster_wg_config_has_interface_and_peers() {
        let nodes = vec![
            crate::models::node::Node {
                id: "n1".into(),
                name: "node-a".into(),
                listen_addr: "1.2.3.4:3000".into(),
                local_asn: 4242420000,
                description: None,
                online: true,
                last_seen_at: String::new(),
                created_at: String::new(),
                updated_at: String::new(),
                wg_pubkey: "pubkey-a".into(),
                tunnel_ip: "10.255.0.1".into(),
                tunnel_ipv6: String::new(),
            },
            crate::models::node::Node {
                id: "n2".into(),
                name: "node-b".into(),
                listen_addr: "5.6.7.8:3000".into(),
                local_asn: 4242420001,
                description: None,
                online: true,
                last_seen_at: String::new(),
                created_at: String::new(),
                updated_at: String::new(),
                wg_pubkey: "pubkey-b".into(),
                tunnel_ip: "10.255.0.2".into(),
                tunnel_ipv6: String::new(),
            },
        ];
        let config = generate_cluster_wg_config(&nodes, "key-a", 51821);
        assert!(config.contains("[Interface]"));
        assert!(config.contains("PrivateKey = key-a"));
        assert!(config.contains("ListenPort = 51821"));
        assert!(config.contains("[Peer]"));
        assert!(config.contains("PublicKey = pubkey-b"));
        assert!(config.contains("Endpoint = 5.6.7.8:51821"));
    }

    #[test]
    fn test_generate_cluster_wg_config_empty_nodes() {
        let nodes: Vec<crate::models::node::Node> = vec![];
        let config = generate_cluster_wg_config(&nodes, "key-a", 51821);
        assert!(config.contains("[Interface]"));
        assert!(!config.contains("[Peer]"));
    }
}
