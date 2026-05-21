# WG & BIRD Direct Management — Design Spec

## Goal

Peerman directly manages WireGuard and BIRD on the host system:
**peer CRUD → auto-generate config → auto-apply without manual steps.**
Cluster-mode nodes also auto-establish inter-node WG tunnels and iBGP full mesh.

## Principles (from DN42 wiki & Lantian's blog)

- WG uses `Table = off` — BGP handles routing, not WireGuard
- BGP export filter: `if source ~ [RTS_STATIC, RTS_BGP] then accept; reject;` — never leak IGP-learned routes
- Do NOT use OSPF/Babel to carry BGP routes — loses AS Path, risks route hijacking
- Inter-node iBGP uses independent tunnel IPs (not public listen_addr)

## Architecture

```
Node A (tunnel: 10.255.0.1)          Node B (tunnel: 10.255.0.2)
    │                                     │
    ├── wg0 (user peers)                  ├── wg0 (user peers)
    ├── wg-cluster (node interconnect)    ├── wg-cluster (node interconnect)
    │    [Peer B: pubkey, endpoint]       │    [Peer A: pubkey, endpoint]
    │    AllowedIPs = 10.255.0.0/24      │    AllowedIPs = 10.255.0.0/24
    │                                     │
    ├── iBGP → 10.255.0.2 ───────────────┤── iBGP → 10.255.0.1
    ├── iBGP → 10.255.0.3 ───────────────┤── iBGP → 10.255.0.3
    ...                                   ...
```

- **Single `wg-cluster` interface** per node, all nodes as `[Peer]` sections
- **Tunnel IP range** from `[cluster]` config (e.g. `10.255.0.0/24`), auto-assigned per node
- **iBGP full mesh** over tunnel IPs (manageable for 2–10 nodes)
- **User peers** share a single `wg0` interface, updated via `wg syncconf`

## Peer lifecycle → auto-apply

On create/update/delete/toggle of any peer:

1. Full-regenerate `wg0.conf` → write `/etc/wireguard/wg0.conf` → `wg syncconf wg0 /etc/wireguard/wg0.conf`
2. Full-regenerate `bird.conf` (user peers + cluster iBGP if enabled) → write `/etc/bird/bird.conf` → `birdc configure`
3. If cluster mode: propagate peer change to other nodes via existing PushPeer RPC

## Cluster inter-node setup

### Key exchange

1. Node startup: generate WG keypair (if not exists), store in `nodes` table
2. `ExchangeNodes` gossip includes `wg_public_key` + `tunnel_ip`
3. On receiving new/updated node info → rebuild `wg-cluster` config → `wg syncconf wg-cluster`
4. On receiving new/updated node info → rebuild `bird.conf` (add iBGP block) → `birdc configure`

### Tunnel IP assignment

- Config: `[cluster] tunnel_ip_range = "10.255.0.0/24"`
- Each node auto-assigns the first unused IP from the range (persisted in `nodes` table)
- IP collisions detected via gossip: if two nodes claim the same IP, the one with lexicographically smaller node_id keeps it, the other re-assigns

### iBGP config generation

For each pair of nodes (N×N-1), generate a BIRD protocol block:

```
protocol bgp node_<name> from <bird_template> {
    neighbor <tunnel_ip> as <local_asn>;
    direct;
    ipv4 {
        next hop self yes;
        import where source = RTS_BGP && is_valid_network() && !is_self_net();
        export where source = RTS_BGP && is_valid_network() && !is_self_net();
    };
    ipv6 {
        next hop self yes;
        import where source = RTS_BGP && is_valid_network_v6() && !is_self_net();
        export where source = RTS_BGP && is_valid_network_v6() && !is_self_net();
    };
}
```

## Proto changes

### ManagementService (new)

```proto
service ManagementService {
  rpc GetWireGuardStatus(GetWGStatusRequest) returns (WGStatusResponse);
  rpc GetBirdStatus(GetBirdStatusRequest) returns (BirdStatusResponse);
}

message GetWGStatusRequest {
  string interface = 1;  // "wg0", "wg-cluster", or empty = all
}

message WGStatusResponse {
  repeated WGInterface interfaces = 1;
}

message WGInterface {
  string name = 1;
  string public_key = 2;
  uint32 listen_port = 3;
  repeated WGPeerStatus peers = 4;
}

message WGPeerStatus {
  string public_key = 1;
  string endpoint = 2;
  string allowed_ips = 3;
  string latest_handshake = 4;
  string transfer_rx = 5;
  string transfer_tx = 6;
}

message GetBirdStatusRequest {}

message BirdStatusResponse {
  repeated BirdProtocol protocols = 1;
}

message BirdProtocol {
  string name = 1;
  string proto = 2;
  string table = 3;
  string state = 4;    // up/down/start
  string since = 5;
  string info = 6;
}
```

### NodeInfo additions

```proto
message NodeInfo {
  // ... existing fields 1-5 ...
  string wg_public_key = 6;
  string tunnel_ip = 7;
}
```

## Config additions

```toml
[cluster]
# ... existing fields ...
tunnel_ip_range = "10.255.0.0/24"    # internal tunnel IPv4 range
tunnel_ipv6_range = ""               # optional: internal tunnel IPv6 range
```

## Backend modules

| File | Purpose |
|------|---------|
| `src/services/wireguard.rs` | +`apply_syncconf(interface, config_text)`, +`get_wg_status(interface)`, +`generate_cluster_wg_config(nodes)` |
| `src/services/bird.rs` | +`apply_config(full_bird_conf)`, +`get_bird_status()`, +`generate_ibgp_blocks(nodes, settings)` |
| `src/grpc/management_service.rs` | New: `ManagementServiceImpl` |
| `src/cluster/tunnel.rs` | New: cluster keypair gen, tunnel IP assignment, WG peer list sync |
| `src/grpc/peer_service.rs` | Hook apply after create/update/delete |
| `src/grpc/cluster_service.rs` | `ExchangeNodes` include wg_pubkey + tunnel_ip |
| `src/config.rs` | Add `tunnel_ip_range` field |
| `src/main.rs` | Register `ManagementService`, init cluster tunnels on startup |

## Frontend

- **New page**: `/status` — WG interface status + BIRD protocol status (read-only, no apply buttons)
- **Peer detail**: on save → auto-applied, show brief toast/success indicator
- **NavBar**: add "Status" link

## Database migration

```sql
ALTER TABLE nodes ADD COLUMN wg_pubkey TEXT NOT NULL DEFAULT '';
ALTER TABLE nodes ADD COLUMN tunnel_ip TEXT NOT NULL DEFAULT '';
```

## Error handling

- `wg syncconf` failure → return error via gRPC, do NOT leave stale config on disk (rewrite previous known-good config)
- `birdc configure` failure → return error with BIRD stderr output, BIRD keeps running with previous config (inherently safe)
- Cluster WG key collision: detect in gossip, resolve deterministically, log warning
- Config file write: atomic write (write to `.tmp` then `rename`) to avoid partial reads by wg/bird

## Testing

- Unit: keypair generation, config generation (existing + new ibgp blocks)
- Unit: tunnel IP assignment logic
- Integration: `birdc configure` with generated config on a test bird instance
- Integration: `wg syncconf` with generated config on a test wg interface
