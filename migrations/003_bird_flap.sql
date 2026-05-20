CREATE TABLE flap_events (
    id TEXT PRIMARY KEY,
    prefix TEXT NOT NULL,
    prefix_type TEXT NOT NULL DEFAULT 'ipv4',
    node_id TEXT REFERENCES nodes(id),
    change_count INTEGER NOT NULL DEFAULT 0,
    window_start TEXT NOT NULL,
    window_end TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'ibgp',
    active INTEGER NOT NULL DEFAULT 1,
    detected_at TEXT NOT NULL,
    resolved_at TEXT
);

CREATE INDEX idx_flap_events_active ON flap_events(active);
CREATE INDEX idx_flap_events_prefix ON flap_events(prefix);
CREATE INDEX idx_flap_events_detected ON flap_events(detected_at);
