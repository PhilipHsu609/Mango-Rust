-- IDs table: Maps random UUIDs to file paths and signatures for tracking manga files
-- This allows Mango to detect renames/moves and maintain reading progress across changes
-- Schema matches original Mango for compatibility

CREATE TABLE IF NOT EXISTS ids (
    path TEXT NOT NULL,                 -- Absolute filesystem path
    id TEXT NOT NULL,                   -- Random UUID for title/entry
    is_title INTEGER NOT NULL,          -- 1 for titles (series), 0 for entries (chapters/volumes)
    signature TEXT,                     -- File signature for change detection (nullable for compatibility)
    unavailable INTEGER NOT NULL DEFAULT 0  -- 1 if file no longer exists, 0 if available
);

CREATE UNIQUE INDEX IF NOT EXISTS path_idx ON ids (path);
CREATE UNIQUE INDEX IF NOT EXISTS id_idx ON ids (id);
