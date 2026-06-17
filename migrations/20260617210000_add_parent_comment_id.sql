-- Add parent_comment_id to report_comment for threaded replies
-- Migration: 2026-06-17

ALTER TABLE report_comment 
ADD COLUMN IF NOT EXISTS parent_comment_id INTEGER REFERENCES report_comment(id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_report_comment_parent_id ON report_comment(parent_comment_id);

-- Add index for fetching replies by parent
CREATE INDEX IF NOT EXISTS idx_report_comment_root_id ON report_comment(
    COALESCE(parent_comment_id, id)
);
