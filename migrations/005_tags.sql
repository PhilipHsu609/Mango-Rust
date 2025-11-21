-- Create tags table
CREATE TABLE IF NOT EXISTS tags (
    id TEXT NOT NULL,
    tag TEXT NOT NULL,
    UNIQUE (id, tag)
);

CREATE INDEX IF NOT EXISTS idx_tags_id ON tags (id);
CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags (tag);
