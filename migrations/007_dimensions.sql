-- Create dimensions cache table
-- Stores page dimensions to avoid repeated ZIP extraction
CREATE TABLE IF NOT EXISTS dimensions (
    entry_id TEXT NOT NULL,
    page_num INTEGER NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    PRIMARY KEY (entry_id, page_num)
);

-- Index for efficient lookups by entry_id
CREATE INDEX IF NOT EXISTS idx_dimensions_entry_id ON dimensions(entry_id);
