-- Known Peerman nodes in the cluster
CREATE TABLE IF NOT EXISTS nodes (
    id TEXT PRIMARY KEY,                       -- UUID
    name TEXT NOT NULL UNIQUE,
    listen_addr TEXT NOT NULL,                 -- e.g., "172.20.1.1:3000"
    local_asn INTEGER NOT NULL,
    description TEXT DEFAULT '',
    online INTEGER NOT NULL DEFAULT 0,
    last_seen_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Network probe results between nodes
CREATE TABLE IF NOT EXISTS probe_results (
    id TEXT PRIMARY KEY,
    from_node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    to_node_id TEXT NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    avg_latency_ms REAL NOT NULL,
    min_latency_ms REAL NOT NULL,
    max_latency_ms REAL NOT NULL,
    packet_loss_pct REAL NOT NULL DEFAULT 0,
    packets_sent INTEGER NOT NULL DEFAULT 0,
    packets_received INTEGER NOT NULL DEFAULT 0,
    probed_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_probe_results_from ON probe_results(from_node_id);
CREATE INDEX IF NOT EXISTS idx_probe_results_to ON probe_results(to_node_id);
CREATE INDEX IF NOT EXISTS idx_probe_results_probed_at ON probe_results(probed_at);

-- Community mapping rules
CREATE TABLE IF NOT EXISTS community_rules (
    id TEXT PRIMARY KEY,
    description TEXT DEFAULT '',
    max_latency_ms REAL NOT NULL,              -- upper bound (0 = infinity)
    max_packet_loss_pct REAL NOT NULL DEFAULT 100,
    community_ipv4 TEXT NOT NULL,              -- e.g., "local_asn,100"
    community_ipv6 TEXT NOT NULL,              -- e.g., "local_asn,600"
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Add origin_node_id to peers
ALTER TABLE peers ADD COLUMN origin_node_id TEXT DEFAULT '';
CREATE INDEX IF NOT EXISTS idx_peers_origin_node ON peers(origin_node_id);
