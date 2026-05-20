use crate::models::peer::Peer;
use crate::models::settings::Settings;

/// Generate a single BIRD2 protocol block for a peer (for inclusion in existing config).
pub fn generate_peer_block(peer: &Peer, settings: &Settings) -> String {
    let name = sanitize_name(&peer.name);
    let mut block = String::new();

    let has_ipv4 = peer.ipv4_tunnel_remote.is_some();
    let has_ipv6 = peer.ipv6_tunnel_remote.is_some();

    let use_ipv4_session = peer.sessions == 0 || peer.sessions == 2;
    let use_ipv6_session = peer.sessions == 1 || peer.sessions == 2;

    // Mode 1 or 2: Multiprotocol — single protocol block
    if peer.multiprotocol {
        let neighbor = if peer.extended_nexthop && has_ipv6 {
            format!(
                "{}%{}",
                peer.ipv6_tunnel_remote.as_ref().unwrap(),
                peer.wg_interface_name
            )
        } else if has_ipv4 {
            peer.ipv4_tunnel_remote.as_ref().unwrap().clone()
        } else {
            String::new()
        };

        block.push_str(&format!(
            "protocol bgp peer_{name} from {tpl} {{\n",
            tpl = settings.bird_template_name
        ));

        if !neighbor.is_empty() {
            block.push_str(&format!(
                "    neighbor {neighbor} as {};\n",
                peer.asn
            ));
        }

        if peer.extended_nexthop && has_ipv6 {
            block.push_str("    ipv4 {\n        extended next hop on;\n    };\n");
        }

        if peer.passive {
            block.push_str("    passive on;\n");
        }

        if let Some(limit) = peer.import_max_prefix {
            block.push_str(&format!("    ipv4 import limit {limit};\n"));
        }

        block.push_str("}\n\n");
    } else {
        // Mode 3: Separate IPv4 and IPv6 sessions
        if use_ipv4_session && has_ipv4 {
            block.push_str(&format!(
                "protocol bgp peer_{name}_v4 from {tpl} {{\n",
                tpl = settings.bird_template_name
            ));
            block.push_str(&format!(
                "    neighbor {} as {};\n",
                peer.ipv4_tunnel_remote.as_ref().unwrap(),
                peer.asn
            ));
            if peer.passive {
                block.push_str("    passive on;\n");
            }
            block.push_str("}\n\n");
        }

        if use_ipv6_session && has_ipv6 {
            block.push_str(&format!(
                "protocol bgp peer_{name}_v6 from {tpl} {{\n",
                tpl = settings.bird_template_name
            ));
            block.push_str(&format!(
                "    neighbor {}%{} as {};\n",
                peer.ipv6_tunnel_remote.as_ref().unwrap(),
                peer.wg_interface_name,
                peer.asn
            ));
            if peer.passive {
                block.push_str("    passive on;\n");
            }
            block.push_str("}\n\n");
        }
    }

    block
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
