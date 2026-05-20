use base64::Engine;
use rand::RngCore;
use x25519_dalek::{PublicKey, StaticSecret};

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

    // AllowedIPs — full DN42 prefixes plus link-local
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
            wg_table: "auto".into(),
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
}
