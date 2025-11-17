-- Add unavailable column to track missing manga files
-- Items marked as unavailable (1) are not shown in library but kept in database
-- This allows users to restore accidentally deleted files without losing metadata

-- Add unavailable column to ids table (for entries)
ALTER TABLE ids ADD COLUMN unavailable INTEGER NOT NULL DEFAULT 0;

-- Create index for quick filtering of unavailable items
CREATE INDEX idx_ids_unavailable ON ids (unavailable);
