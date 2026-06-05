-- Add UNIQUE constraint on nodes.listen_addr for upsert_self ON CONFLICT support
-- Migration 004 created a non-unique index with the same name; drop it first
DROP INDEX IF EXISTS idx_nodes_listen_addr;
CREATE UNIQUE INDEX idx_nodes_listen_addr ON nodes(listen_addr);
