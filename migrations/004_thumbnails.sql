-- Create thumbnails table with foreign key (matches original Mango schema)
CREATE TABLE IF NOT EXISTS thumbnails (
    id TEXT NOT NULL,
    data BLOB NOT NULL,
    filename TEXT NOT NULL,
    mime TEXT NOT NULL,
    size INTEGER NOT NULL,
    FOREIGN KEY (id) REFERENCES ids (id)
        ON UPDATE CASCADE
        ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS tn_index ON thumbnails (id);
