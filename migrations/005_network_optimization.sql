-- Add WireGuard advanced settings columns
ALTER TABLE settings ADD COLUMN wg_mtu INTEGER NOT NULL DEFAULT 1420;
ALTER TABLE settings ADD COLUMN wg_fwmark INTEGER NOT NULL DEFAULT 0;
ALTER TABLE settings ADD COLUMN wg_post_up TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN wg_post_down TEXT NOT NULL DEFAULT '';

-- Add ROA/RPKI settings columns
ALTER TABLE settings ADD COLUMN roa_mode TEXT NOT NULL DEFAULT 'none';
ALTER TABLE settings ADD COLUMN roa_static_v4_url TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN roa_static_v6_url TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN roa_rtr_address TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN roa_rtr_port INTEGER NOT NULL DEFAULT 323;

-- Add BIRD filter settings columns
ALTER TABLE settings ADD COLUMN bird_import_limit INTEGER NOT NULL DEFAULT 9000;
ALTER TABLE settings ADD COLUMN bird_export_filter TEXT NOT NULL DEFAULT '';
ALTER TABLE settings ADD COLUMN bird_import_filter TEXT NOT NULL DEFAULT '';

-- Add community rule multi-dimension columns
ALTER TABLE community_rules ADD COLUMN min_bandwidth_mbps REAL NOT NULL DEFAULT 0;
ALTER TABLE community_rules ADD COLUMN crypto_weight INTEGER NOT NULL DEFAULT 0;
ALTER TABLE community_rules ADD COLUMN med_penalty INTEGER NOT NULL DEFAULT 0;
