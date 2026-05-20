use crate::models::community::CommunityRuleRepository;
use crate::models::peer::Peer;
use crate::models::probe::ProbeResultRepository;

pub struct CommunityMapper;

impl CommunityMapper {
    /// Compute which community tags match a peer based on the latest probe
    /// result between the local node and the peer's origin node.
    pub async fn compute_communities(
        peer: &Peer,
        local_node_id: &str,
        probe_repo: &ProbeResultRepository,
        rule_repo: &CommunityRuleRepository,
    ) -> Result<(Vec<String>, Vec<String>), crate::error::AppError> {
        let rules = rule_repo.list_enabled().await?;

        let origin_node_id = peer.origin_node_id.as_deref().unwrap_or(local_node_id);

        let (latency, loss_pct) = if origin_node_id == local_node_id {
            (0.0, 0.0)
        } else {
            match probe_repo
                .latest_between(local_node_id, origin_node_id)
                .await?
            {
                Some(probe) => (probe.avg_latency_ms, probe.packet_loss_pct),
                None => return Ok((Vec::new(), Vec::new())),
            }
        };

        let mut v4 = Vec::new();
        let mut v6 = Vec::new();

        for rule in &rules {
            if !rule.enabled {
                continue;
            }
            // Rule matches if latency <= max_latency AND loss <= max_loss
            // max_latency of 0 means infinity (matches anything)
            let lat_ok = rule.max_latency_ms <= 0.0 || latency <= rule.max_latency_ms;
            let loss_ok = loss_pct <= rule.max_packet_loss_pct;

            if lat_ok && loss_ok {
                v4.push(rule.community_ipv4.clone());
                v6.push(rule.community_ipv6.clone());
            }
        }

        Ok((v4, v6))
    }

    /// Generate BIRD export filter lines for community tags.
    pub fn to_bird_filter_lines(communities_v4: &[String], communities_v6: &[String]) -> String {
        let mut lines = String::new();

        if !communities_v4.is_empty() {
            lines.push_str("    ipv4 {\n        export filter {\n");
            for c in communities_v4 {
                lines.push_str(&format!(
                    "            bgp_community.add(({}));\n",
                    c
                ));
            }
            lines.push_str("            accept;\n        };\n    };\n");
        }

        if !communities_v6.is_empty() {
            lines.push_str("    ipv6 {\n        export filter {\n");
            for c in communities_v6 {
                lines.push_str(&format!(
                    "            bgp_community.add(({}));\n",
                    c
                ));
            }
            lines.push_str("            accept;\n        };\n    };\n");
        }

        lines
    }
}
