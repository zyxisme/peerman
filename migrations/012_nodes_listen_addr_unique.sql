-- Add UNIQUE constraint on nodes.listen_addr for upsert_self ON CONFLICT support
CREATE UNIQUE INDEX IF NOT EXISTS idx_nodes_listen_addr ON nodes(listen_addr);
