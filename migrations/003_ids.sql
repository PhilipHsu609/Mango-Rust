-- IDs table: Stores manga entries (chapters/volumes)
-- Schema matches original Mango exactly for compatibility
-- Paths are relative to library root (e.g., "Series Name/Chapter 01.zip")

CREATE TABLE IF NOT EXISTS ids (
    path TEXT NOT NULL,                     -- Relative path from library root
    id TEXT NOT NULL,                       -- Random UUID for the entry
    signature TEXT,                         -- File signature (inode on Unix, CRC32 on Windows)
    unavailable INTEGER NOT NULL DEFAULT 0, -- 1 if file no longer exists
    sort_title TEXT                         -- Optional sort override (not currently used)
);

CREATE UNIQUE INDEX IF NOT EXISTS path_idx ON ids (path);
CREATE UNIQUE INDEX IF NOT EXISTS id_idx ON ids (id);
