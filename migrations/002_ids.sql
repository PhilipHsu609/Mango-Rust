-- IDs table: Maps random UUIDs to file paths and signatures for tracking manga files
-- This allows Mango to detect renames/moves and maintain reading progress across changes

CREATE TABLE IF NOT EXISTS ids (
    id TEXT NOT NULL PRIMARY KEY,      -- Random UUID for title/entry
    path TEXT NOT NULL,                 -- Absolute filesystem path
    signature INTEGER NOT NULL,         -- Inode (Unix) or CRC32 hash (Windows) for change detection
    type TEXT NOT NULL CHECK(type IN ('title', 'entry'))  -- Whether this is a series or chapter
);

CREATE INDEX idx_ids_path ON ids (path);
CREATE INDEX idx_ids_signature ON ids (signature);
CREATE INDEX idx_ids_type ON ids (type);
