use crate::models::peer::Peer;
use crate::models::settings::Settings;

/// Generate a single BIRD2 protocol block for a peer (for inclusion in existing config).
/// Optional communities: IPv4 and IPv6 community strings for export filters.
pub fn generate_peer_block_with_communities(
    peer: &Peer,
    settings: &Settings,
    communities_v4: &[String],
    communities_v6: &[String],
) -> String {
    let name = sanitize_name(&peer.name);
    let mut block = String::new();

    let has_communities = !communities_v4.is_empty() || !communities_v6.is_empty();

    // Mode 1 or 2: Multiprotocol — single protocol block
    if peer.multiprotocol {
        let neighbor = if peer.extended_nexthop {
            if let Some(ref v6) = peer.ipv6_tunnel_remote {
                format!("{v6}%{}", peer.wg_interface_name)
            } else {
                String::new()
            }
        } else if let Some(ref v4) = peer.ipv4_tunnel_remote {
            v4.clone()
        } else {
            String::new()
        };

        block.push_str(&format!(
            "protocol bgp peer_{name} from {tpl} {{\n",
            tpl = settings.bird_template_name
        ));

        if !neighbor.is_empty() {
            block.push_str(&format!("    neighbor {neighbor} as {};\n", peer.asn));
        }

        if peer.extended_nexthop && peer.ipv6_tunnel_remote.is_some() {
            block.push_str("    ipv4 {\n        extended next hop on;\n    };\n");
        }

        if peer.passive {
            block.push_str("    passive on;\n");
        }

        if let Some(limit) = peer.import_max_prefix {
            block.push_str(&format!("    ipv4 import limit {limit};\n"));
        }

        if has_communities {
            block.push_str(&crate::services::community_mapper::CommunityMapper::to_bird_filter_lines(
                communities_v4,
                communities_v6,
            ));
        }

        block.push_str("}\n\n");
    } else {
        // Mode 3: Separate IPv4 and IPv6 sessions
        let use_ipv4_session = peer.sessions == 0 || peer.sessions == 2;
        let use_ipv6_session = peer.sessions == 1 || peer.sessions == 2;

        if use_ipv4_session {
            if let Some(ref v4) = peer.ipv4_tunnel_remote {
                block.push_str(&format!(
                    "protocol bgp peer_{name}_v4 from {tpl} {{\n",
                    tpl = settings.bird_template_name
                ));
                block.push_str(&format!(
                    "    neighbor {v4} as {};\n",
                    peer.asn
                ));
                if peer.passive {
                    block.push_str("    passive on;\n");
                }
                if !communities_v4.is_empty() {
                    let v4_filter = crate::services::community_mapper::CommunityMapper::to_bird_filter_lines(
                        communities_v4,
                        &[],
                    );
                    block.push_str(&v4_filter);
                }
                block.push_str("}\n\n");
            }
        }

        if use_ipv6_session {
            if let Some(ref v6) = peer.ipv6_tunnel_remote {
                block.push_str(&format!(
                    "protocol bgp peer_{name}_v6 from {tpl} {{\n",
                    tpl = settings.bird_template_name
                ));
                block.push_str(&format!(
                    "    neighbor {v6}%{} as {};\n",
                    peer.wg_interface_name,
                    peer.asn
                ));
                if peer.passive {
                    block.push_str("    passive on;\n");
                }
                if !communities_v6.is_empty() {
                    let v6_filter = crate::services::community_mapper::CommunityMapper::to_bird_filter_lines(
                        &[],
                        communities_v6,
                    );
                    block.push_str(&v6_filter);
                }
                block.push_str("}\n\n");
            }
        }
    }

    block
}

/// Generate a single BIRD2 protocol block for a peer (for inclusion in existing config).
pub fn generate_peer_block(peer: &Peer, settings: &Settings) -> String {
    generate_peer_block_with_communities(peer, settings, &[], &[])
}

/// Generate a complete BIRD2 configuration with template, filters, and all peer blocks.
pub fn generate_full_config(
    peers: &[Peer],
    settings: &Settings,
    template_body: &str,
) -> String {
    let mut config = String::new();

    // Router definition
    config.push_str(&format!(
        "router id {};\n\n",
        settings.bird_router_id
    ));

    // ASN and routing tables
    config.push_str(&format!(
        "define OWNAS = {};\n",
        settings.local_asn
    ));
    config.push_str(&format!(
        "define OWNNETSET = [{}+, {}];\n\n",
        settings.dn42_ipv4_prefix, settings.dn42_ipv6_prefix
    ));

    // BGP template
    config.push_str(&format!(
        "template bgp {tpl} {{\n",
        tpl = settings.bird_template_name
    ));
    if !template_body.is_empty() {
        config.push_str(template_body);
    } else {
        config.push_str("    local as OWNAS;\n");
        config.push_str("    ipv4 {\n        import all;\n        export all;\n    };\n");
        config.push_str("    ipv6 {\n        import all;\n        export all;\n    };\n");
    }
    config.push_str("}\n\n");

    // Peer blocks
    for peer in peers.iter().filter(|p| p.enabled) {
        config.push_str(&generate_peer_block(peer, settings));
    }

    config
}

/// Generate only the protocol blocks (snippets) for all enabled peers.
pub fn generate_snippets(peers: &[Peer], settings: &Settings) -> String {
    peers
        .iter()
        .filter(|p| p.enabled)
        .map(|p| generate_peer_block(p, settings))
        .collect::<Vec<_>>()
        .join("")
}

fn sanitize_name(name: &str) -> String {
    name.replace(|c: char| !c.is_ascii_alphanumeric() && c != '-', "_")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::settings::Settings;

    fn test_settings() -> Settings {
        Settings {
            local_asn: 4242420000,
            bird_template_name: "dnpeers".into(),
            bird_router_id: "172.20.0.1".into(),
            wg_default_listen_port: 42420,
            dn42_ipv4_prefix: "172.20.0.0/14".into(),
            dn42_ipv6_prefix: "fd00::/8".into(),
            wg_table: "off".into(),
        }
    }

    fn test_peer() -> Peer {
        Peer {
            id: "test-id".into(),
            name: "Test Peer".into(),
            description: None,
            asn: 4242420001,
            local_asn: 4242420000,
            wg_private_key: None,
            wg_public_key: None,
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
    fn test_sanitize_name_converts_spaces() {
        assert_eq!(sanitize_name("Test Peer"), "test_peer");
    }

    #[test]
    fn test_sanitize_name_preserves_hyphens() {
        assert_eq!(sanitize_name("my-peer"), "my-peer");
    }

    #[test]
    fn test_sanitize_name_lowercases() {
        assert_eq!(sanitize_name("UPPERCASE"), "uppercase");
    }

    #[test]
    fn test_generate_peer_block_multiprotocol() {
        let block = generate_peer_block(&test_peer(), &test_settings());
        assert!(block.contains("protocol bgp peer_test_peer from dnpeers"));
        assert!(block.contains("neighbor fd00::2%wg0 as 4242420001"));
    }

    #[test]
    fn test_generate_peer_block_no_tunnel_ips() {
        let mut peer = test_peer();
        peer.ipv4_tunnel_remote = None;
        peer.ipv6_tunnel_remote = None;
        let block = generate_peer_block(&peer, &test_settings());
        assert!(block.contains("bgp peer_test_peer"));
    }
}
