CREATE TABLE IF NOT EXISTS settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    local_asn INTEGER NOT NULL DEFAULT 4242420000,
    bird_template_name TEXT NOT NULL DEFAULT 'dnpeers',
    bird_router_id TEXT NOT NULL DEFAULT '172.20.0.1',
    wg_default_listen_port INTEGER NOT NULL DEFAULT 42420,
    dn42_ipv4_prefix TEXT NOT NULL DEFAULT '172.20.0.0/14',
    dn42_ipv6_prefix TEXT NOT NULL DEFAULT 'fd00::/8',
    wg_table TEXT NOT NULL DEFAULT 'off'
);

INSERT OR IGNORE INTO settings (id) VALUES (1);

CREATE TABLE IF NOT EXISTS peers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    asn INTEGER NOT NULL,
    local_asn INTEGER NOT NULL,

    wg_private_key TEXT,
    wg_public_key TEXT,
    wg_remote_address TEXT NOT NULL,
    wg_remote_port INTEGER NOT NULL,
    wg_listen_port INTEGER NOT NULL,
    wg_interface_name TEXT NOT NULL,

    ipv4_tunnel_local TEXT,
    ipv4_tunnel_remote TEXT,
    ipv6_tunnel_local TEXT,
    ipv6_tunnel_remote TEXT,

    multiprotocol INTEGER NOT NULL DEFAULT 1,
    extended_nexthop INTEGER NOT NULL DEFAULT 1,
    sessions INTEGER NOT NULL DEFAULT 1,
    passive INTEGER NOT NULL DEFAULT 0,
    import_max_prefix INTEGER,
    export_max_prefix INTEGER,

    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_peers_name ON peers(name);
CREATE INDEX idx_peers_enabled ON peers(enabled);
