-- Add BFD (Bidirectional Forwarding Detection) settings
ALTER TABLE settings ADD COLUMN enable_bfd INTEGER NOT NULL DEFAULT 0;
ALTER TABLE settings ADD COLUMN bfd_interval_ms INTEGER NOT NULL DEFAULT 300;
ALTER TABLE settings ADD COLUMN bfd_multiplier INTEGER NOT NULL DEFAULT 3;
