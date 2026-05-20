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

fn generate_roa_section(settings: &Settings) -> String {
    match settings.roa_mode.as_str() {
        "static_file" => {
            let mut s = format!(
                "# ROA data (static) — regenerate via cron every 15 min:\n\
                 #   curl -sfSL -o /etc/bird/roa_dn42_v4.conf {}\n\
                 #   curl -sfSL -o /etc/bird/roa_dn42_v6.conf {}\n",
                settings.roa_static_v4_url, settings.roa_static_v6_url
            );
            s.push_str("include \"/etc/bird/roa_dn42_v4.conf\";\n");
            s.push_str("include \"/etc/bird/roa_dn42_v6.conf\";\n\n");
            s
        }
        "rtr" => {
            format!(
                "protocol rpki roa_dn42 {{\n\
                 \x20   roa4 {{ table dn42_roa; }};\n\
                 \x20   roa6 {{ table dn42_roa_v6; }};\n\
                 \x20   remote \"{addr}\";\n\
                 \x20   port {port};\n\
                 \x20   refresh 600;\n\
                 \x20   retry 300;\n\
                 \x20   expire 7200;\n\
                 }}\n\n",
                addr = settings.roa_rtr_address,
                port = settings.roa_rtr_port
            )
        }
        _ => String::new(),
    }
}

fn generate_filter_functions(settings: &Settings) -> String {
    format!(
        "function is_valid_network() -> bool {{\n\
         \x20 return net ~ [\n\
         \x20   {ipv4_prefix}{{21,29}},    # dn42\n\
         \x20   {ipv4_prefix}{{28,32}},    # dn42 Anycast\n\
         \x20   172.21.0.0/24{{28,32}},    # dn42 Anycast\n\
         \x20   172.22.0.0/24{{28,32}},    # dn42 Anycast\n\
         \x20   172.23.0.0/24{{28,32}},    # dn42 Anycast\n\
         \x20   172.31.0.0/16+,           # ChaosVPN\n\
         \x20   10.100.0.0/14+,           # ChaosVPN\n\
         \x20   10.127.0.0/16+,           # neonetwork\n\
         \x20   10.0.0.0/8{{15,24}}        # Freifunk.net\n\
         \x20 ];\n\
         }}\n\n\
         function is_valid_network_v6() -> bool {{\n\
         \x20 return net ~ [ {ipv6_prefix}{{44,64}} ];\n\
         }}\n\n\
         function is_self_net() -> bool {{\n\
         \x20 return net ~ OWNNETSET;\n\
         }}\n\n",
        ipv4_prefix = settings.dn42_ipv4_prefix,
        ipv6_prefix = settings.dn42_ipv6_prefix,
    )
}

/// Generate a complete BIRD2 configuration with template, filters, and all peer blocks.
pub fn generate_full_config(
    peers: &[Peer],
    settings: &Settings,
    template_body: &str,
) -> String {
    let mut config = String::new();

    config.push_str(&format!("router id {};\n\n", settings.bird_router_id));

    config.push_str(&format!("define OWNAS = {};\n", settings.local_asn));
    config.push_str(&format!(
        "define OWNNETSET = [{}+, {}];\n\n",
        settings.dn42_ipv4_prefix, settings.dn42_ipv6_prefix
    ));

    // ROA tables
    if settings.roa_mode != "none" {
        config.push_str("roa4 table dn42_roa;\nroa6 table dn42_roa_v6;\n\n");
        config.push_str(&generate_roa_section(settings));
    }

    // Filter functions
    config.push_str(&generate_filter_functions(settings));

    // BGP template
    config.push_str(&format!("template bgp {tpl} {{\n", tpl = settings.bird_template_name));
    if !template_body.is_empty() {
        config.push_str(template_body);
    } else {
        config.push_str("    local as OWNAS;\n");
        config.push_str("    path metric 1;\n");

        let import_body = if settings.bird_import_filter.is_empty() {
            format!(
                "if is_valid_network() && !is_self_net() then {{\n\
                 \x20         if (roa_check(dn42_roa, net, bgp_path.last) != ROA_VALID) then {{\n\
                 \x20           print \"[dn42] ROA check failed for \", net, \" ASN \", bgp_path.last;\n\
                 \x20           reject;\n\
                 \x20         }} else accept;\n\
                 \x20       }} else reject;"
            )
        } else {
            settings.bird_import_filter.clone()
        };

        let export_body = if settings.bird_export_filter.is_empty() {
            "if is_valid_network() && source ~ [RTS_STATIC, RTS_BGP] then accept; else reject;"
                .to_string()
        } else {
            settings.bird_export_filter.clone()
        };

        config.push_str(&format!(
            "    ipv4 {{\n\
             \x20       import filter {{\n\
             \x20         {import_body}\n\
             \x20       }};\n\
             \x20       export filter {{ {export_body} }};\n\
             \x20       import limit {} action block;\n\
             \x20     }};\n",
            settings.bird_import_limit
        ));

        config.push_str(&format!(
            "    ipv6 {{\n\
             \x20       import filter {{\n\
             \x20         if is_valid_network_v6() && !is_self_net() then {{\n\
             \x20           if (roa_check(dn42_roa_v6, net, bgp_path.last) != ROA_VALID) then {{\n\
             \x20             print \"[dn42] ROA check failed for \", net, \" ASN \", bgp_path.last;\n\
             \x20             reject;\n\
             \x20           }} else accept;\n\
             \x20         }} else reject;\n\
             \x20       }};\n\
             \x20       export filter {{ if is_valid_network_v6() && source ~ [RTS_STATIC, RTS_BGP] then accept; else reject; }};\n\
             \x20       import limit {} action block;\n\
             \x20     }};\n",
            settings.bird_import_limit
        ));
        config.push_str("    import table;\n");
    }
    config.push_str("}\n\n");

    // Peer blocks
    for peer in peers.iter().filter(|p| p.enabled) {
        config.push_str(&generate_peer_block(peer, settings));
    }

    config
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

    #[test]
    fn test_generate_full_config_has_roa_when_rtr() {
        let mut s = test_settings();
        s.roa_mode = "rtr".into();
        s.roa_rtr_address = "rpki.dn42.example".into();
        let config = generate_full_config(&[], &s, "");
        assert!(config.contains("protocol rpki roa_dn42"));
        assert!(config.contains("rpki.dn42.example"));
    }

    #[test]
    fn test_generate_full_config_has_static_roa() {
        let mut s = test_settings();
        s.roa_mode = "static_file".into();
        s.roa_static_v4_url = "https://example.com/roa_v4.conf".into();
        let config = generate_full_config(&[], &s, "");
        assert!(config.contains("include \"/etc/bird/roa_dn42_v4.conf\""));
    }

    #[test]
    fn test_generate_full_config_has_filter_functions() {
        let config = generate_full_config(&[], &test_settings(), "");
        assert!(config.contains("function is_valid_network()"));
        assert!(config.contains("function is_valid_network_v6()"));
        assert!(config.contains("function is_self_net()"));
    }

    #[test]
    fn test_generate_full_config_has_import_limit() {
        let config = generate_full_config(&[], &test_settings(), "");
        assert!(config.contains("import limit 9000 action block"));
    }

    #[test]
    fn test_generate_full_config_has_roa_check() {
        let config = generate_full_config(&[], &test_settings(), "");
        assert!(config.contains("roa_check(dn42_roa, net, bgp_path.last)"));
    }

    #[test]
    fn test_generate_full_config_custom_export_filter() {
        let mut s = test_settings();
        s.bird_export_filter = "accept;".into();
        let config = generate_full_config(&[], &s, "");
        assert!(config.contains("export filter { accept; }"));
    }
}
