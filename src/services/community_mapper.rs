use crate::models::community::CommunityRuleRepository;
use crate::models::peer::Peer;
use crate::models::probe::ProbeResultRepository;

pub struct CommunityMapper;

impl CommunityMapper {
    /// Compute which community tags match a peer based on the latest probe
    /// result between the local node and the peer's origin node.
    ///
    /// Matching is 3-dimensional: latency, bandwidth, crypto weight.
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

        let crypto_weight: i32 = if peer.wg_private_key.is_some() { 1 } else { 0 };

        let mut v4 = Vec::new();
        let mut v6 = Vec::new();

        for rule in &rules {
            if !rule.enabled {
                continue;
            }
            let lat_ok = rule.max_latency_ms <= 0.0 || latency <= rule.max_latency_ms;
            let loss_ok = loss_pct <= rule.max_packet_loss_pct;
            let bw_ok = rule.min_bandwidth_mbps <= 0.0;
            let crypto_ok = rule.crypto_weight == 0 || crypto_weight >= rule.crypto_weight;

            if lat_ok && loss_ok && bw_ok && crypto_ok {
                v4.push(rule.community_ipv4.clone());
                v6.push(rule.community_ipv6.clone());
            }
        }

        Ok((v4, v6))
    }

    /// Compute BGP MED value for a peer based on matched community rules.
    pub async fn compute_med(
        peer: &Peer,
        local_node_id: &str,
        probe_repo: &ProbeResultRepository,
        rule_repo: &CommunityRuleRepository,
    ) -> Result<i32, crate::error::AppError> {
        let rules = rule_repo.list_enabled().await?;

        let origin_node_id = peer.origin_node_id.as_deref().unwrap_or(local_node_id);

        let latency = if origin_node_id == local_node_id {
            0.0
        } else {
            match probe_repo
                .latest_between(local_node_id, origin_node_id)
                .await?
            {
                Some(probe) => probe.avg_latency_ms,
                None => return Ok(1000),
            }
        };

        let crypto_weight: i32 = if peer.wg_private_key.is_some() { 1 } else { 0 };

        let mut med: i32 = 0;

        for rule in &rules {
            if !rule.enabled {
                continue;
            }
            let lat_ok = rule.max_latency_ms <= 0.0 || latency <= rule.max_latency_ms;
            let crypto_ok = rule.crypto_weight == 0 || crypto_weight >= rule.crypto_weight;

            if lat_ok && crypto_ok {
                med += rule.med_penalty;
            }
        }

        Ok(med)
    }

    /// Generate BIRD export filter lines for community tags.
    pub fn to_bird_filter_lines(communities_v4: &[String], communities_v6: &[String]) -> String {
        let mut lines = String::new();

        if !communities_v4.is_empty() {
            lines.push_str("    ipv4 {\n        export filter {\n");
            for c in communities_v4 {
                lines.push_str(&format!("            bgp_community.add(({}));\n", c));
            }
            lines.push_str("            accept;\n        };\n    };\n");
        }

        if !communities_v6.is_empty() {
            lines.push_str("    ipv6 {\n        export filter {\n");
            for c in communities_v6 {
                lines.push_str(&format!("            bgp_community.add(({}));\n", c));
            }
            lines.push_str("            accept;\n        };\n    };\n");
        }

        lines
    }
}
