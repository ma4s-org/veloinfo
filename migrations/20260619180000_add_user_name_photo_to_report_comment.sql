-- Add user_name and photo_path_thumbnail columns to report_comment
ALTER TABLE report_comment ADD COLUMN IF NOT EXISTS user_name TEXT;
ALTER TABLE report_comment ADD COLUMN IF NOT EXISTS photo_path_thumbnail TEXT;