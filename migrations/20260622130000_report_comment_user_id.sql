-- Replace user_name with user_id in report_comment
-- Migration: 2026-06-22

-- Step 1: Add user_id column
ALTER TABLE report_comment ADD COLUMN IF NOT EXISTS user_id UUID REFERENCES users(id);

-- Step 2: Migrate existing user_name to user_id by matching against users.name
UPDATE report_comment rc
SET user_id = u.id
FROM users u
WHERE rc.user_name = u.name
  AND rc.user_id IS NULL;

-- Step 3: Drop user_name column
ALTER TABLE report_comment DROP COLUMN IF EXISTS user_name;

-- Step 4: Add index for user_id lookups
CREATE INDEX IF NOT EXISTS idx_report_comment_user_id ON report_comment(user_id);