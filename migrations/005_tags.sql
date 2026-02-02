-- Create tags table with foreign key (matches original Mango schema)
CREATE TABLE IF NOT EXISTS tags (
    id TEXT NOT NULL,
    tag TEXT NOT NULL,
    UNIQUE (id, tag),
    FOREIGN KEY (id) REFERENCES titles (id)
        ON UPDATE CASCADE
        ON DELETE CASCADE
);

-- Index names match original Mango
CREATE INDEX IF NOT EXISTS tags_id_idx ON tags (id);
CREATE INDEX IF NOT EXISTS tags_tag_idx ON tags (tag);
