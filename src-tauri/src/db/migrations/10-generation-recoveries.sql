CREATE TABLE IF NOT EXISTS generation_recoveries (
    generation_id TEXT PRIMARY KEY REFERENCES generations(id) ON DELETE CASCADE,
    request_kind TEXT NOT NULL,
    request_state TEXT NOT NULL,
    output_format TEXT NOT NULL,
    response_file TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_generation_recoveries_state ON generation_recoveries(request_state);
