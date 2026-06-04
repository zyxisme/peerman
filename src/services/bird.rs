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
            block.push_str(
                &crate::services::community_mapper::CommunityMapper::to_bird_filter_lines(
                    communities_v4,
                    communities_v6,
                ),
            );
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
                block.push_str(&format!("    neighbor {v4} as {};\n", peer.asn));
                if peer.passive {
                    block.push_str("    passive on;\n");
                }
                if !communities_v4.is_empty() {
                    let v4_filter =
                        crate::services::community_mapper::CommunityMapper::to_bird_filter_lines(
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
                    peer.wg_interface_name, peer.asn
                ));
                if peer.passive {
                    block.push_str("    passive on;\n");
                }
                if !communities_v6.is_empty() {
                    let v6_filter =
                        crate::services::community_mapper::CommunityMapper::to_bird_filter_lines(
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

fn generate_bfd_section(settings: &Settings) -> String {
    if !settings.enable_bfd {
        return String::new();
    }
    let interval = if settings.bfd_interval_ms > 0 {
        settings.bfd_interval_ms
    } else {
        300
    };
    let multiplier = if settings.bfd_multiplier > 0 {
        settings.bfd_multiplier
    } else {
        3
    };
    format!(
        "protocol bfd {{\n\
         \x20   interface \"wg*\" {{\n\
         \x20       interval {interval}ms;\n\
         \x20       multiplier {multiplier};\n\
         \x20   }};\n\
         }}\n\n"
    )
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

fn generate_community_functions() -> String {
    "\
function update_latency(int link_latency) {\n\
    bgp_community.add((64511, link_latency));\n\
}\n\n\
function update_bandwidth(int link_bandwidth) {\n\
    bgp_community.add((64511, 10 + link_bandwidth));\n\
}\n\n\
function update_crypto(int link_crypto) {\n\
    bgp_community.add((64511, 30 + link_crypto));\n\
}\n\n\
function update_flags(int link_latency; int link_bandwidth; int link_crypto) {\n\
    update_latency(link_latency);\n\
    update_bandwidth(link_bandwidth);\n\
    update_crypto(link_crypto);\n\
}\n\n\
function dn42_import_filter(int link_latency; int link_bandwidth; int link_crypto) {\n\
    if is_valid_network() && !is_self_net() then {\n\
        if (roa_check(dn42_roa, net, bgp_path.last) != ROA_VALID) then {\n\
            print \"[dn42] ROA check failed for \", net, \" ASN \", bgp_path.last;\n\
            reject;\n\
        }\n\
        update_flags(link_latency, link_bandwidth, link_crypto);\n\
        if (bgp_path.len = 1) then {\n\
            bgp_local_pref = bgp_local_pref + 500;\n\
        }\n\
        accept;\n\
    } else reject;\n\
}\n\n\
function dn42_export_filter(int link_latency; int link_bandwidth; int link_crypto) {\n\
    if is_valid_network() || is_valid_network_v6() then {\n\
        update_flags(link_latency, link_bandwidth, link_crypto);\n\
        bgp_med = bgp_med + 4 * link_crypto;\n\
        bgp_med = bgp_med + 9 * link_bandwidth;\n\
        bgp_med = bgp_med + link_latency;\n\
        accept;\n\
    } else reject;\n\
}\n\n\
"
    .to_string()
}

/// Generate a complete BIRD2 configuration with template, filters, and all peer blocks.
/// `peer_communities` maps peer ID to (v4_communities, v6_communities).
pub fn generate_full_config(
    peers: &[Peer],
    settings: &Settings,
    template_body: &str,
    peer_communities: &std::collections::HashMap<String, (Vec<String>, Vec<String>)>,
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

    // Community filter functions (DN42 standard AS 64511)
    if settings.enable_community_filters {
        config.push_str(&generate_community_functions());
    }

    // BFD
    config.push_str(&generate_bfd_section(settings));

    // BGP template
    config.push_str(&format!(
        "template bgp {tpl} {{\n",
        tpl = settings.bird_template_name
    ));
    if !template_body.is_empty() {
        config.push_str(template_body);
    } else {
        config.push_str("    local as OWNAS;\n");
        config.push_str("    path metric 1;\n");

        let import_body = if settings.bird_import_filter.is_empty() {
            "if is_valid_network() && !is_self_net() then {\n\
                 \x20         if (roa_check(dn42_roa, net, bgp_path.last) != ROA_VALID) then {\n\
                 \x20           print \"[dn42] ROA check failed for \", net, \" ASN \", bgp_path.last;\n\
                 \x20           reject;\n\
                 \x20         } else accept;\n\
                 \x20       } else reject;".to_string()
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
        let (v4, v6) = peer_communities
            .get(&peer.id)
            .map(|(a, b)| (a.as_slice(), b.as_slice()))
            .unwrap_or((&[], &[]));
        config.push_str(&generate_peer_block_with_communities(
            peer, settings, v4, v6,
        ));
    }

    config
}

/// Write bird.conf to /etc/bird/ and hot-reload via birdc configure.
pub fn apply_config(config: &str) -> Result<(), crate::error::AppError> {
    use std::io::Write;

    let config_path = "/etc/bird/bird.conf";
    let tmp_path = "/etc/bird/bird.conf.tmp";

    // Atomic write: tmp then rename
    {
        let mut f = std::fs::File::create(tmp_path).map_err(|e| {
            crate::error::AppError::Internal(format!("Cannot create bird.conf.tmp: {e}"))
        })?;
        f.write_all(config.as_bytes()).map_err(|e| {
            crate::error::AppError::Internal(format!("Cannot write bird.conf.tmp: {e}"))
        })?;
    }
    std::fs::rename(tmp_path, config_path)
        .map_err(|e| crate::error::AppError::Internal(format!("Cannot rename bird.conf: {e}")))?;

    // Hot reload
    let output = std::process::Command::new("birdc")
        .arg("configure")
        .output()
        .map_err(|e| crate::error::AppError::Internal(format!("birdc not found: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        return Err(crate::error::AppError::Internal(format!(
            "birdc configure failed: {stdout} {stderr}"
        )));
    }
    Ok(())
}

/// Parse `birdc show protocols` output into structured status.
pub fn get_bird_status() -> Result<Vec<crate::grpc::generated::BirdProtocol>, crate::error::AppError>
{
    let output = std::process::Command::new("birdc")
        .args(["show", "protocols"])
        .output()
        .map_err(|e| crate::error::AppError::Internal(format!("birdc failed: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut protocols: Vec<crate::grpc::generated::BirdProtocol> = Vec::new();

    for line in stdout.lines().skip(2) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            protocols.push(crate::grpc::generated::BirdProtocol {
                name: parts[0].to_string(),
                proto: parts[1].to_string(),
                table: parts[2].to_string(),
                state: parts[3].to_string(),
                since: parts[4].to_string(),
                info: parts.get(5..).map(|s| s.join(" ")).unwrap_or_default(),
            });
        }
    }

    Ok(protocols)
}

/// Generate iBGP full-mesh protocol blocks for cluster nodes.
/// `my_tunnel_ip` is the local node's tunnel IP, used to skip self.
///
/// When `settings.enable_confederation` is true and `confederation_local_asn > 0`,
/// generates BGP Confederation blocks instead of standard iBGP:
/// - Uses `confederation_local_asn` as the local AS (private ASN per node)
/// - Uses `settings.local_asn` as the confederation identifier (DN42 ASN)
/// - Adds `confederation member yes` and `neighbor <ip> external`
pub fn generate_ibgp_blocks(
    nodes: &[crate::models::node::Node],
    settings: &Settings,
    my_tunnel_ip: &str,
) -> String {
    let mut blocks = String::new();
    let use_confederation = settings.enable_confederation && settings.confederation_local_asn > 0;

    for node in nodes {
        if node.tunnel_ip == my_tunnel_ip || node.tunnel_ip.is_empty() {
            continue;
        }

        let name = sanitize_name(&node.name);

        if use_confederation {
            // BGP Confederation mode
            blocks.push_str(&format!(
                "protocol bgp node_{name} from {tpl} {{\n",
                tpl = settings.bird_template_name
            ));
            blocks.push_str(&format!(
                "    local as {};\n",
                settings.confederation_local_asn
            ));
            blocks.push_str(&format!("    confederation {};\n", settings.local_asn));
            blocks.push_str("    confederation member yes;\n");
            blocks.push_str(&format!("    neighbor {} external;\n", node.tunnel_ip));
            blocks.push_str("    direct;\n");
            blocks.push_str("    ipv4 {\n");
            blocks.push_str("        next hop self yes;\n");
            blocks.push_str(
                "        import where source = RTS_BGP && is_valid_network() && !is_self_net();\n",
            );
            blocks.push_str(
                "        export where source = RTS_BGP && is_valid_network() && !is_self_net();\n",
            );
            blocks.push_str("    };\n");
            blocks.push_str("    ipv6 {\n");
            blocks.push_str("        next hop self yes;\n");
            blocks.push_str("        import where source = RTS_BGP && is_valid_network_v6() && !is_self_net();\n");
            blocks.push_str("        export where source = RTS_BGP && is_valid_network_v6() && !is_self_net();\n");
            blocks.push_str("    };\n");
            blocks.push_str("}\n\n");

            // Add separate IPv6 confederation neighbor if tunnel_ipv6 is assigned
            if !node.tunnel_ipv6.is_empty() {
                blocks.push_str(&format!(
                    "protocol bgp node_{name}_v6 from {tpl} {{\n",
                    tpl = settings.bird_template_name
                ));
                blocks.push_str(&format!(
                    "    local as {};\n",
                    settings.confederation_local_asn
                ));
                blocks.push_str(&format!("    confederation {};\n", settings.local_asn));
                blocks.push_str("    confederation member yes;\n");
                blocks.push_str(&format!("    neighbor {} external;\n", node.tunnel_ipv6));
                blocks.push_str("    direct;\n");
                blocks.push_str("    ipv6 {\n");
                blocks.push_str("        next hop self yes;\n");
                blocks.push_str("        import where source = RTS_BGP && is_valid_network_v6() && !is_self_net();\n");
                blocks.push_str("        export where source = RTS_BGP && is_valid_network_v6() && !is_self_net();\n");
                blocks.push_str("    };\n");
                blocks.push_str("}\n\n");
            }
        } else {
            // Standard iBGP full mesh
            blocks.push_str(&format!(
                "protocol bgp node_{name} from {tpl} {{\n",
                tpl = settings.bird_template_name
            ));
            blocks.push_str(&format!(
                "    neighbor {tunnel_ip} as {local_asn};\n",
                tunnel_ip = node.tunnel_ip,
                local_asn = settings.local_asn
            ));
            blocks.push_str("    direct;\n");
            blocks.push_str("    ipv4 {\n");
            blocks.push_str("        next hop self yes;\n");
            blocks.push_str(
                "        import where source = RTS_BGP && is_valid_network() && !is_self_net();\n",
            );
            blocks.push_str(
                "        export where source = RTS_BGP && is_valid_network() && !is_self_net();\n",
            );
            blocks.push_str("    };\n");
            blocks.push_str("    ipv6 {\n");
            blocks.push_str("        next hop self yes;\n");
            blocks.push_str("        import where source = RTS_BGP && is_valid_network_v6() && !is_self_net();\n");
            blocks.push_str("        export where source = RTS_BGP && is_valid_network_v6() && !is_self_net();\n");
            blocks.push_str("    };\n");
            blocks.push_str("}\n\n");

            // Add separate IPv6 iBGP neighbor if tunnel_ipv6 is assigned
            if !node.tunnel_ipv6.is_empty() {
                blocks.push_str(&format!(
                    "protocol bgp node_{name}_v6 from {tpl} {{\n",
                    tpl = settings.bird_template_name
                ));
                blocks.push_str(&format!(
                    "    neighbor {tunnel_ipv6} as {local_asn};\n",
                    tunnel_ipv6 = node.tunnel_ipv6,
                    local_asn = settings.local_asn
                ));
                blocks.push_str("    direct;\n");
                blocks.push_str("    ipv6 {\n");
                blocks.push_str("        next hop self yes;\n");
                blocks.push_str("        import where source = RTS_BGP && is_valid_network_v6() && !is_self_net();\n");
                blocks.push_str("        export where source = RTS_BGP && is_valid_network_v6() && !is_self_net();\n");
                blocks.push_str("    };\n");
                blocks.push_str("}\n\n");
            }
        }
    }

    blocks
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
        let config = generate_full_config(&[], &s, "", &std::collections::HashMap::new());
        assert!(config.contains("protocol rpki roa_dn42"));
        assert!(config.contains("rpki.dn42.example"));
    }

    #[test]
    fn test_generate_full_config_has_static_roa() {
        let mut s = test_settings();
        s.roa_mode = "static_file".into();
        s.roa_static_v4_url = "https://example.com/roa_v4.conf".into();
        let config = generate_full_config(&[], &s, "", &std::collections::HashMap::new());
        assert!(config.contains("include \"/etc/bird/roa_dn42_v4.conf\""));
    }

    #[test]
    fn test_generate_full_config_has_filter_functions() {
        let config =
            generate_full_config(&[], &test_settings(), "", &std::collections::HashMap::new());
        assert!(config.contains("function is_valid_network()"));
        assert!(config.contains("function is_valid_network_v6()"));
        assert!(config.contains("function is_self_net()"));
    }

    #[test]
    fn test_generate_full_config_has_import_limit() {
        let config =
            generate_full_config(&[], &test_settings(), "", &std::collections::HashMap::new());
        assert!(config.contains("import limit 9000 action block"));
    }

    #[test]
    fn test_generate_full_config_has_roa_check() {
        let config =
            generate_full_config(&[], &test_settings(), "", &std::collections::HashMap::new());
        assert!(config.contains("roa_check(dn42_roa, net, bgp_path.last)"));
    }

    #[test]
    fn test_generate_full_config_custom_export_filter() {
        let mut s = test_settings();
        s.bird_export_filter = "accept;".into();
        let config = generate_full_config(&[], &s, "", &std::collections::HashMap::new());
        assert!(config.contains("export filter { accept; }"));
    }

    #[test]
    fn test_generate_ibgp_blocks_creates_protocol_blocks() {
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
                wg_pubkey: "pk-a".into(),
                tunnel_ip: "10.255.0.1".into(),
                tunnel_ipv6: String::new(),
                wg_private_key: String::new(),
            },
            crate::models::node::Node {
                id: "n2".into(),
                name: "node-b".into(),
                listen_addr: "5.6.7.8:3000".into(),
                local_asn: 4242420000,
                description: None,
                online: true,
                last_seen_at: String::new(),
                created_at: String::new(),
                updated_at: String::new(),
                wg_pubkey: "pk-b".into(),
                tunnel_ip: "10.255.0.2".into(),
                tunnel_ipv6: String::new(),
                wg_private_key: String::new(),
            },
        ];
        let settings = test_settings();
        let blocks = generate_ibgp_blocks(&nodes, &settings, "10.255.0.1");
        assert!(blocks.contains("protocol bgp node_node-b from"));
        assert!(blocks.contains("neighbor 10.255.0.2 as 4242420000"));
        assert!(blocks.contains("next hop self yes"));
    }

    #[test]
    fn test_generate_ibgp_blocks_skips_self_and_no_tunnel_ip() {
        let nodes = vec![
            crate::models::node::Node {
                id: "n1".into(),
                name: "self-node".into(),
                listen_addr: "1.2.3.4:3000".into(),
                local_asn: 4242420000,
                description: None,
                online: true,
                last_seen_at: String::new(),
                created_at: String::new(),
                updated_at: String::new(),
                wg_pubkey: "pk-a".into(),
                tunnel_ip: "10.255.0.1".into(),
                tunnel_ipv6: String::new(),
                wg_private_key: String::new(),
            },
            crate::models::node::Node {
                id: "n2".into(),
                name: "no-tunnel".into(),
                listen_addr: "5.6.7.8:3000".into(),
                local_asn: 4242420000,
                description: None,
                online: false,
                last_seen_at: String::new(),
                created_at: String::new(),
                updated_at: String::new(),
                wg_pubkey: String::new(),
                tunnel_ip: String::new(),
                tunnel_ipv6: String::new(),
                wg_private_key: String::new(),
            },
        ];
        let settings = test_settings();
        let blocks = generate_ibgp_blocks(&nodes, &settings, "10.255.0.1");
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_generate_peer_block_with_community_tiers() {
        let block = generate_peer_block_with_communities(
            &test_peer(),
            &test_settings(),
            &["4242420000,10".into()],
            &["4242420000,610".into()],
        );
        // Should contain community add calls in export filter
        assert!(block.contains("bgp_community.add"));
    }

    #[test]
    fn test_generate_full_config_has_community_functions() {
        let mut s = test_settings();
        s.enable_community_filters = true;
        let config = generate_full_config(&[], &s, "", &std::collections::HashMap::new());
        assert!(config.contains("function update_latency"));
        assert!(config.contains("function update_bandwidth"));
        assert!(config.contains("function update_crypto"));
        assert!(config.contains("function update_flags"));
        assert!(config.contains("function dn42_import_filter"));
        assert!(config.contains("function dn42_export_filter"));
    }

    #[test]
    fn test_generate_full_config_community_functions_use_64511() {
        let mut s = test_settings();
        s.enable_community_filters = true;
        let config = generate_full_config(&[], &s, "", &std::collections::HashMap::new());
        assert!(config.contains("bgp_community.add((64511,"));
    }

    #[test]
    fn test_generate_full_config_no_community_functions_when_disabled() {
        let mut s = test_settings();
        s.enable_community_filters = false;
        let config = generate_full_config(&[], &s, "", &std::collections::HashMap::new());
        assert!(!config.contains("function update_latency"));
    }

    #[test]
    fn test_generate_full_config_has_community_functions_when_enabled() {
        let mut s = test_settings();
        s.enable_community_filters = true;
        let config = generate_full_config(&[], &s, "", &std::collections::HashMap::new());
        assert!(config.contains("function update_latency"));
    }

    #[test]
    fn test_generate_full_config_has_bfd_when_enabled() {
        let mut s = test_settings();
        s.enable_bfd = true;
        s.bfd_interval_ms = 300;
        s.bfd_multiplier = 3;
        let config = generate_full_config(&[], &s, "", &std::collections::HashMap::new());
        assert!(config.contains("protocol bfd"));
        assert!(config.contains("interval 300ms"));
        assert!(config.contains("multiplier 3"));
    }

    #[test]
    fn test_generate_full_config_no_bfd_when_disabled() {
        let mut s = test_settings();
        s.enable_bfd = false;
        let config = generate_full_config(&[], &s, "", &std::collections::HashMap::new());
        assert!(!config.contains("protocol bfd"));
    }

    #[test]
    fn test_generate_ibgp_blocks_confederation() {
        let mut s = test_settings();
        s.enable_confederation = true;
        s.confederation_local_asn = 65000;
        s.local_asn = 4242420000;
        let nodes = vec![crate::models::node::Node {
            id: "n1".into(),
            name: "node-a".into(),
            listen_addr: "1.2.3.4:3000".into(),
            local_asn: 4242420000,
            description: None,
            online: true,
            last_seen_at: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            wg_pubkey: "pk-a".into(),
            tunnel_ip: "10.255.0.1".into(),
            tunnel_ipv6: String::new(),
            wg_private_key: String::new(),
        }];
        let blocks = generate_ibgp_blocks(&nodes, &s, "10.255.0.2");
        assert!(blocks.contains("confederation 4242420000"));
        assert!(blocks.contains("confederation member yes"));
        assert!(blocks.contains("local as 65000"));
        assert!(blocks.contains("neighbor 10.255.0.1 external"));
    }

    #[test]
    fn test_generate_ibgp_blocks_no_confederation_when_disabled() {
        let mut s = test_settings();
        s.enable_confederation = false;
        let nodes = vec![crate::models::node::Node {
            id: "n1".into(),
            name: "node-a".into(),
            listen_addr: "1.2.3.4:3000".into(),
            local_asn: 4242420000,
            description: None,
            online: true,
            last_seen_at: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            wg_pubkey: "pk-a".into(),
            tunnel_ip: "10.255.0.1".into(),
            tunnel_ipv6: String::new(),
            wg_private_key: String::new(),
        }];
        let blocks = generate_ibgp_blocks(&nodes, &s, "10.255.0.2");
        assert!(!blocks.contains("confederation"));
        assert!(blocks.contains("neighbor 10.255.0.1 as 4242420000"));
    }

    #[test]
    fn test_generate_ibgp_blocks_confederation_with_ipv6() {
        let mut s = test_settings();
        s.enable_confederation = true;
        s.confederation_local_asn = 65000;
        s.local_asn = 4242420000;
        let nodes = vec![crate::models::node::Node {
            id: "n1".into(),
            name: "node-a".into(),
            listen_addr: "1.2.3.4:3000".into(),
            local_asn: 4242420000,
            description: None,
            online: true,
            last_seen_at: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
            wg_pubkey: "pk-a".into(),
            tunnel_ip: "10.255.0.1".into(),
            tunnel_ipv6: "fd00:255::1".into(),
            wg_private_key: String::new(),
        }];
        let blocks = generate_ibgp_blocks(&nodes, &s, "10.255.0.2");
        // IPv4 block should have confederation directives
        assert!(blocks.contains("confederation 4242420000"));
        assert!(blocks.contains("neighbor 10.255.0.1 external"));
        // IPv6 block should also have confederation directives
        assert!(blocks.contains("neighbor fd00:255::1 external"));
        // Should have two separate protocol blocks (v4 + v6)
        assert_eq!(blocks.matches("confederation member yes").count(), 2);
    }
}
