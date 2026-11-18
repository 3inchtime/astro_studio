CREATE TABLE IF NOT EXISTS prompt_favorites (
    id TEXT PRIMARY KEY,
    prompt TEXT NOT NULL COLLATE NOCASE,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_prompt_favorites_prompt ON prompt_favorites(prompt);
CREATE INDEX IF NOT EXISTS idx_prompt_favorites_updated_at ON prompt_favorites(updated_at);
