ALTER TABLE generations ADD COLUMN deleted_at TEXT;
CREATE INDEX IF NOT EXISTS idx_generations_deleted_at ON generations(deleted_at);
