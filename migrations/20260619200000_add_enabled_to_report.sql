-- Add enabled column to report (boolean, defaults to true)
ALTER TABLE report ADD COLUMN IF NOT EXISTS enabled BOOLEAN NOT NULL DEFAULT TRUE;