-- Titles table: Stores manga series/titles
-- Schema matches original Mango exactly for compatibility
-- Paths are relative to library root (e.g., "Series Name")

CREATE TABLE IF NOT EXISTS titles (
    id TEXT NOT NULL,                       -- Random UUID for the title
    path TEXT NOT NULL,                     -- Relative path from library root (just directory name)
    signature TEXT,                         -- Directory signature (CRC32 of entry signatures)
    unavailable INTEGER NOT NULL DEFAULT 0, -- 1 if directory no longer exists
    sort_title TEXT                         -- Optional sort override (not currently used)
);

CREATE UNIQUE INDEX IF NOT EXISTS titles_id_idx ON titles (id);
CREATE UNIQUE INDEX IF NOT EXISTS titles_path_idx ON titles (path);
