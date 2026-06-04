-- Add community filters toggle
ALTER TABLE settings ADD COLUMN enable_community_filters INTEGER NOT NULL DEFAULT 0;
