# Distributed Flat HA Cluster — Design Spec

## Motivation

Current cluster mode: nodes self-register, probe each other, and compute community rules. But the web panel connects to a single node — if that node goes offline, the panel is unreachable. Peers are bound to individual nodes but there is no way to view or manage them across the cluster from one pane of glass.

**Goal:** distributed, flat-hierarchy (all nodes equal), high-availability management plane. Any node can serve the full panel. Node failure does not block viewing or managing the rest of the cluster.

## Network Topology

Two independent paths:

| Path | Address | Purpose |
|---|---|---|
| Public IP | `[server].listen_addr` (e.g. `1.2.3.4:3000`) | Inter-node gRPC, web panel access |
| WG mesh | DN42 IPs on WireGuard interfaces | DN42 BGP peering only |

Cluster control-plane traffic never depends on the WG mesh. A node can be cluster-offline while its DN42 BGP sessions remain up, and vice versa.

## Node Discovery

### Bootstrap

```toml
[cluster]
node_name = "node-sjc"
cluster_key = "<shared-secret>"
peer_nodes = ["1.2.3.4:3000", "5.6.7.8:3000"]
```

On startup, connect to each `peer_nodes` address (public IPs), self-register, and call `ExchangeNodes` to pull the peer's full node list.

### Anti-Entropy Exchange

Every `sync_interval_secs` (default 30s), each node picks a random online peer and calls `ExchangeNodes`:

```
A → ExchangeNodes([A, B, C]) → D
D → ExchangeNodes([C, D, E]) → A
A merges E into local nodes table
```

New nodes discovered this way are upserted into the local `nodes` table by `listen_addr`. This is a one-way merge (pull + upsert), not a full two-phase set reconciliation — sufficient for small DN42 clusters.

### Cluster Key Auth

All inter-node gRPC calls carry `x-cluster-key` metadata. Receivers validate it against their own `cluster_key`. Mismatch → `PermissionDenied`. If no peer accepts the key at startup, the node runs isolated (local-only) and logs a warning.

Distinct from `jwt_secret` (user auth) — separate keys, separate concerns.

## Data Model

### Per-node SQLite — no cross-node replication

Each node's SQLite holds only its own data. No rows are copied between databases.

### In-memory cache on every node

```rust
struct NodeCache {
    peers: Vec<Peer>,
    probe_results: Vec<ProbeResult>,
    community_rules: Vec<CommunityRule>,
    fetched_at: Instant,
    stale: bool,
}

struct ClusterCache {
    by_node: HashMap<String, NodeCache>,  // keyed by node listen_addr
}
```

Populated by fan-out reads: each successful fan-out response replaces that node's cache entry with `stale: false`. Failed/timeout fan-out targets keep their previous cache entry with `stale: true`. Cleared on restart (in-memory only — no persistence needed; re-populated on first query).

## API Aggregation

### Read: Fan-out + merge + cache

```
Client request: ListPeers
  │
  ▼
Gateway node (any node the client connects to)
  │
  ├─ Query local nodes table → who is online?
  │
  ├─ Concurrent gRPC calls to ALL online nodes:
  │     node_a.PullPeers()  ──┐
  │     node_b.PullPeers()  ──┤ tokio::join!
  │     node_c.PullPeers()  ──┘
  │
  ├─ Merge results, tag each peer with origin_node_id
  │
  ├─ For OFFLINE nodes: serve from NodeCache, set stale=true
  │
  └─ Response:
      {
        peers: [...],
        node_status: [
          { node_id: "a", online: true, staleness: "fresh" },
          { node_id: "d", online: false, staleness: "stale", last_seen: "..." }
        ]
      }
```

Timeout per fan-out call: 2s. If a node times out, mark it offline, use cache.

### Write: Proxy to target node

```
Client request: CreatePeer(node_id="node-b", ...)
  │
  ▼
Gateway node
  ├─ if target_node == self → execute locally
  └─ else → gRPC PushPeer(target_node, payload)
             → return result to client
```

The client must specify the target node for every write. The gateway enforces that peers are never cross-assigned without explicit user intent.

### Affected RPCs

| RPC | Change |
|---|---|
| `PullPeers` | Already exists — used for fan-out |
| `ListProbeResults` | Already exists — used for fan-out |
| `ListCommunityRules` | Already exists — fan-out |
| `ListNodes` | Already exists — serve from local nodes table (which now reflects full cluster) |
| `PushPeer` | Already exists — used for write proxying |
| `ExchangeNodes` | **New** — push/pull node list between peers |
| `HealthCheck` | **New** — lightweight `{}` → `{ online: bool }` for gRPC-level liveness |

## Health Checking

Reuse the existing ICMP ping probe mechanism but tighten the interval:

- Interval: 15s (configurable via `probe_interval_secs`)
- Flap suppression: 2 consecutive failures → offline; 2 consecutive successes → online
- `HealthCheck` gRPC as secondary signal: if a fan-out call returns `Unavailable`/timeout, that also marks the node for health re-evaluation
- On status transition → online: invalidate that node's cache (force re-fetch on next read)
- On status transition → offline: mark that node's cache as `stale`

## Frontend

### New: node status indicator

In the navbar: colored dot reflecting cluster health.
- Green: all known nodes online
- Yellow: some nodes offline
- Red: only self online (isolated)

### Modified: existing pages

**Peers list:**
- New column: `节点` (origin node name)
- Offline node's peers: grayed row, tooltip "节点离线，数据来自缓存"
- Create Peer form: target node dropdown (defaults to currently-connected node)

**Probes page:**
- Show `from_node` → `to_node` in results table
- Filter by node

**Community Rules:**
- Per-node scope indicator (rules are node-specific)

### New: node detail

Click a node name anywhere → modal/side panel showing:
- Node info (name, addr, ASN, online status, last seen)
- Peer count
- Latest probe stats (avg latency to each other node)

No new top-level routes. All additions are inline in existing pages.

## Failure Modes

| Scenario | Behavior |
|---|---|
| Gateway node goes down | User switches to another node's panel URL — no data lost (just a different gateway) |
| Remote node goes down mid-request | Fan-out times out (2s), that node's data served from cache with `stale` |
| All remote nodes down | Panel shows only local data + all other nodes marked offline |
| Cluster key mismatch | New node rejected by all peers, runs isolated, WARNING log |
| Network partition | Each partition sees the other side as offline; no split-brain (no replication to conflict) |
| Gateway restart | Cache empty — first query fan-out repopulates; stale marks for any still-offline nodes |

## Migration

- `cluster_key` field added to `[cluster]` config section
- `peer_nodes` added to `[cluster]` config section (replaces `bootstrap_nodes` — old field deprecated)
- `nodes` table unchanged (schema already supports this)
- `ExchangeNodes` and `HealthCheck` RPCs added to proto
- No database migration needed

## What This Does NOT Do

- No automatic BGP session failover (out of scope — peers are per-node by design)
- No distributed consensus (no Raft/Paxos)
- No cross-node SQL replication
- No service discovery beyond the anti-entropy node exchange
- No load balancing — the user picks which node URL to open
