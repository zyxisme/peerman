use crate::grpc::generated::Peer;
use crate::models::probe::ProbeResult;

/// Apply proto Peer fields to a local Peer model.
pub fn apply_proto_to_model(model: &mut crate::models::peer::Peer, proto: &Peer) {
    model.name = proto.name.clone();
    model.description = if proto.description.is_empty() {
        None
    } else {
        Some(proto.description.clone())
    };
    model.asn = proto.asn;
    model.local_asn = proto.local_asn;
    model.wg_private_key = if proto.wg_private_key.is_empty() {
        None
    } else {
        Some(proto.wg_private_key.clone())
    };
    model.wg_public_key = if proto.wg_public_key.is_empty() {
        None
    } else {
        Some(proto.wg_public_key.clone())
    };
    model.wg_remote_address = proto.wg_remote_address.clone();
    model.wg_remote_port = proto.wg_remote_port as i64;
    model.wg_listen_port = proto.wg_listen_port as i64;
    model.wg_interface_name = proto.wg_interface_name.clone();
    model.ipv4_tunnel_local = if proto.ipv4_tunnel_local.is_empty() {
        None
    } else {
        Some(proto.ipv4_tunnel_local.clone())
    };
    model.ipv4_tunnel_remote = if proto.ipv4_tunnel_remote.is_empty() {
        None
    } else {
        Some(proto.ipv4_tunnel_remote.clone())
    };
    model.ipv6_tunnel_local = if proto.ipv6_tunnel_local.is_empty() {
        None
    } else {
        Some(proto.ipv6_tunnel_local.clone())
    };
    model.ipv6_tunnel_remote = if proto.ipv6_tunnel_remote.is_empty() {
        None
    } else {
        Some(proto.ipv6_tunnel_remote.clone())
    };
    model.multiprotocol = proto.multiprotocol;
    model.extended_nexthop = proto.extended_nexthop;
    model.sessions = proto.sessions;
    model.passive = proto.passive;
    model.import_max_prefix = if proto.import_max_prefix == 0 {
        None
    } else {
        Some(proto.import_max_prefix as i64)
    };
    model.export_max_prefix = if proto.export_max_prefix == 0 {
        None
    } else {
        Some(proto.export_max_prefix as i64)
    };
    model.enabled = proto.enabled;
    model.updated_at = proto.updated_at.clone();
    model.origin_node_id = if proto.origin_node_id.is_empty() {
        None
    } else {
        Some(proto.origin_node_id.clone())
    };
}

/// Convert a local ProbeResult to its proto representation.
pub fn probe_result_to_proto(r: &ProbeResult) -> crate::grpc::generated::ProbeResult {
    crate::grpc::generated::ProbeResult {
        id: r.id.clone(),
        from_node_id: r.from_node_id.clone(),
        to_node_id: r.to_node_id.clone(),
        avg_latency_ms: r.avg_latency_ms,
        min_latency_ms: r.min_latency_ms,
        max_latency_ms: r.max_latency_ms,
        packet_loss_pct: r.packet_loss_pct,
        packets_sent: r.packets_sent,
        packets_received: r.packets_received,
        probed_at: r.probed_at.clone(),
    }
}
