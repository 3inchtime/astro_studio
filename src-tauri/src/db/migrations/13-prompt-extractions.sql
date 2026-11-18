CREATE TABLE IF NOT EXISTS prompt_extractions (
    id TEXT PRIMARY KEY,
    image_path TEXT NOT NULL,
    prompt TEXT NOT NULL,
    llm_config_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_prompt_extractions_updated_at ON prompt_extractions(updated_at);
