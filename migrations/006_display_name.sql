-- Add display_name column to titles and ids tables
-- This allows users to customize the displayed name without changing the file/folder name

ALTER TABLE titles ADD COLUMN display_name TEXT;
ALTER TABLE ids ADD COLUMN display_name TEXT;
