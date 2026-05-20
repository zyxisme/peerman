use regex::Regex;
use std::sync::OnceLock;
use tokio::process::Command;

use crate::error::AppError;
use crate::models::node::Node;
use crate::models::probe::{ProbeResult, ProbeResultRepository};
use uuid::Uuid;

fn rtt_regex() -> &'static Regex {
    static RTT_RE: OnceLock<Regex> = OnceLock::new();
    RTT_RE.get_or_init(|| {
        Regex::new(r"rtt min/avg/max/mdev = ([\d.]+)/([\d.]+)/([\d.]+)/[\d.]+ ms").unwrap()
    })
}

fn stats_regex() -> &'static Regex {
    static STATS_RE: OnceLock<Regex> = OnceLock::new();
    STATS_RE.get_or_init(|| {
        Regex::new(r"(\d+) packets transmitted, (\d+) received, ([\d.]+)% packet loss").unwrap()
    })
}

pub struct PingOutput {
    pub avg_ms: f64,
    pub min_ms: f64,
    pub max_ms: f64,
    pub loss_pct: f64,
    pub sent: i32,
    pub received: i32,
}

/// Run `ping -c <count> -i <interval> <target>` and parse output.
pub async fn ping(target_ip: &str, count: u32, interval_secs: f64) -> Result<PingOutput, AppError> {
    let output = Command::new("ping")
        .args([
            "-c",
            &count.to_string(),
            "-i",
            &format!("{:.1}", interval_secs),
            "-W",
            "2",
            target_ip,
        ])
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("ping command failed: {e}")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    let (min_ms, avg_ms, max_ms) = rtt_regex()
        .captures(&stdout)
        .map(|caps| {
            (
                caps[1].parse::<f64>().unwrap_or(0.0),
                caps[2].parse::<f64>().unwrap_or(0.0),
                caps[3].parse::<f64>().unwrap_or(0.0),
            )
        })
        .unwrap_or((0.0, 0.0, 0.0));

    let (sent, received, loss_pct) = stats_regex()
        .captures(&stdout)
        .map(|caps| {
            (
                caps[1].parse::<i32>().unwrap_or(count as i32),
                caps[2].parse::<i32>().unwrap_or(0),
                caps[3].parse::<f64>().unwrap_or(100.0),
            )
        })
        .unwrap_or((count as i32, 0, 100.0));

    Ok(PingOutput {
        avg_ms,
        min_ms,
        max_ms,
        loss_pct,
        sent,
        received,
    })
}

/// Resolve a target IP for probing a node.
/// Prefers the first tunnel IP found in peers hosted on that node,
/// falls back to the listen_addr host part.
pub fn resolve_target_ip(node: &Node) -> String {
    // Extract host from listen_addr (e.g., "172.20.1.1:3000" → "172.20.1.1")
    node.listen_addr
        .split(':')
        .next()
        .unwrap_or("127.0.0.1")
        .to_string()
}

/// Probe between two nodes and store the result.
pub async fn probe_between(
    from_node: &Node,
    to_node: &Node,
    repo: &ProbeResultRepository,
) -> Result<ProbeResult, AppError> {
    let target_ip = resolve_target_ip(to_node);
    let ping_out = ping(&target_ip, 5, 0.2).await?;

    let now = chrono::Utc::now().to_rfc3339();
    let result = ProbeResult {
        id: Uuid::new_v4().to_string(),
        from_node_id: from_node.id.clone(),
        to_node_id: to_node.id.clone(),
        avg_latency_ms: ping_out.avg_ms,
        min_latency_ms: ping_out.min_ms,
        max_latency_ms: ping_out.max_ms,
        packet_loss_pct: ping_out.loss_pct,
        packets_sent: ping_out.sent,
        packets_received: ping_out.received,
        probed_at: now,
    };

    repo.insert(&result).await?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_target_ip_extracts_host() {
        let node = Node {
            id: "n1".into(),
            name: "test".into(),
            listen_addr: "192.168.1.1:3000".into(),
            local_asn: 0,
            description: None,
            online: true,
            last_seen_at: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        assert_eq!(resolve_target_ip(&node), "192.168.1.1");
    }

    #[test]
    fn test_resolve_target_ip_no_port() {
        let node = Node {
            id: "n1".into(),
            name: "test".into(),
            listen_addr: "10.0.0.1".into(),
            local_asn: 0,
            description: None,
            online: true,
            last_seen_at: String::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        assert_eq!(resolve_target_ip(&node), "10.0.0.1");
    }
}

/// Run probes from this node to all other nodes. Returns probe results.
pub async fn probe_all(
    local_node: &Node,
    all_nodes: &[Node],
    repo: &ProbeResultRepository,
) -> Vec<ProbeResult> {
    let mut results = Vec::new();

    for node in all_nodes {
        if node.id == local_node.id {
            continue;
        }
        match probe_between(local_node, node, repo).await {
            Ok(r) => results.push(r),
            Err(e) => {
                tracing::warn!("Probe failed from {} to {}: {}", local_node.name, node.name, e);
            }
        }
    }

    results
}
