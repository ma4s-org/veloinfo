-- Rename cyclability_score table to report
-- Create separate report_comment table for comments
-- Migration: 2026-06-17

-- Step 1: Rename cyclability_score to report (if exists)
DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'cyclability_score') THEN
        ALTER TABLE cyclability_score RENAME TO report;
    END IF;
END $$;

DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM pg_sequences WHERE sequencename = 'cyclability_score_id_seq') THEN
        ALTER SEQUENCE cyclability_score_id_seq RENAME TO report_id_seq;
    END IF;
END $$;

-- Rename indexes (if they exist with old names)
DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'cyclability_score_pkey') THEN
        ALTER INDEX cyclability_score_pkey RENAME TO report_pkey;
    END IF;
END $$;

DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = 'idx_cyclability_score_created_at') THEN
        ALTER INDEX idx_cyclability_score_created_at RENAME TO idx_report_created_at;
    END IF;
END $$;

-- Step 2: Create report_comment table
CREATE TABLE IF NOT EXISTS report_comment (
    id SERIAL PRIMARY KEY,
    report_id INT NOT NULL REFERENCES report(id),
    comment TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_report_comment_report_id ON report_comment(report_id);
CREATE INDEX IF NOT EXISTS idx_report_comment_created_at ON report_comment(created_at);

-- Step 3: Migrate existing comments from report to report_comment (if comment column exists)
DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'report' AND column_name = 'comment') THEN
        INSERT INTO report_comment (report_id, comment, created_at)
        SELECT id, comment, created_at
        FROM report
        WHERE comment IS NOT NULL AND comment != '';
    END IF;
END $$;

-- Step 4: Drop comment column from report table (if exists)
DO $$ 
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns WHERE table_name = 'report' AND column_name = 'comment') THEN
        ALTER TABLE report DROP COLUMN comment;
    END IF;
END $$;

-- Step 5: Recreate geometry index with new name
DROP INDEX IF EXISTS idx_report_geom;
CREATE INDEX idx_report_geom ON report USING GIST (geom);
