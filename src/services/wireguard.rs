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
