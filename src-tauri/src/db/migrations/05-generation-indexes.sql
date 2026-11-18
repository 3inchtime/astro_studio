CREATE INDEX IF NOT EXISTS idx_generations_engine ON generations(engine);
CREATE INDEX IF NOT EXISTS idx_generations_request_kind ON generations(request_kind);
CREATE INDEX IF NOT EXISTS idx_generations_size ON generations(size);
CREATE INDEX IF NOT EXISTS idx_generations_quality ON generations(quality);
CREATE INDEX IF NOT EXISTS idx_generations_output_format ON generations(output_format);
CREATE INDEX IF NOT EXISTS idx_conversations_updated_at ON conversations(updated_at);
