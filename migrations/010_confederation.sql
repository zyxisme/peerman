ALTER TABLE settings ADD COLUMN enable_confederation INTEGER NOT NULL DEFAULT 0;
ALTER TABLE settings ADD COLUMN confederation_local_asn INTEGER NOT NULL DEFAULT 0;
