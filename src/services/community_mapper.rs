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

    /// Map latency (ms) to DN42 community tier (1-5).
    pub fn latency_to_tier(latency_ms: f64) -> i32 {
        if latency_ms <= 5.0 { 1 }
        else if latency_ms <= 20.0 { 2 }
        else if latency_ms <= 50.0 { 3 }
        else if latency_ms <= 150.0 { 4 }
        else { 5 }
    }

    /// Extract the numeric tier from a community string like "4242420000,10".
    pub fn parse_community_tier(community: &str) -> i32 {
        community
            .split(',')
            .nth(1)
            .and_then(|s| s.parse::<i32>().ok())
            .unwrap_or(0)
    }

    /// Extract the best (lowest) latency tier from a list of community strings.
    pub fn best_latency_tier(communities: &[String]) -> i32 {
        communities.iter()
            .map(|c| Self::parse_community_tier(c))
            .min()
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_to_tier_metro() {
        assert_eq!(CommunityMapper::latency_to_tier(3.0), 1);
    }

    #[test]
    fn test_latency_to_tier_regional() {
        assert_eq!(CommunityMapper::latency_to_tier(15.0), 2);
    }

    #[test]
    fn test_latency_to_tier_continental() {
        assert_eq!(CommunityMapper::latency_to_tier(35.0), 3);
    }

    #[test]
    fn test_latency_to_tier_intercontinental() {
        assert_eq!(CommunityMapper::latency_to_tier(100.0), 4);
    }

    #[test]
    fn test_latency_to_tier_high() {
        assert_eq!(CommunityMapper::latency_to_tier(200.0), 5);
    }

    #[test]
    fn test_parse_community_tier() {
        assert_eq!(CommunityMapper::parse_community_tier("4242420000,10"), 10);
        assert_eq!(CommunityMapper::parse_community_tier("4242420000,620"), 620);
    }

    #[test]
    fn test_parse_community_tier_invalid() {
        assert_eq!(CommunityMapper::parse_community_tier("invalid"), 0);
    }
}
