-- Missing database indexes for common query patterns
CREATE INDEX IF NOT EXISTS idx_nodes_listen_addr ON nodes(listen_addr);
CREATE INDEX IF NOT EXISTS idx_probe_results_from_to_probed ON probe_results(from_node_id, to_node_id, probed_at DESC);
CREATE INDEX IF NOT EXISTS idx_flap_events_prefix_node_active ON flap_events(prefix, node_id, active);
