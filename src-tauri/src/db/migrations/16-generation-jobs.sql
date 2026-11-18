CREATE TABLE IF NOT EXISTS generation_jobs (
    id TEXT PRIMARY KEY,
    client_request_id TEXT NOT NULL UNIQUE,
    generation_id TEXT NOT NULL UNIQUE REFERENCES generations(id) ON DELETE CASCADE,
    parent_job_id TEXT REFERENCES generation_jobs(id) ON DELETE SET NULL,
    source_kind TEXT NOT NULL,
    source_ref_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL,
    request_json TEXT NOT NULL,
    provider_kind TEXT NOT NULL,
    provider_profile_id TEXT NOT NULL,
    endpoint_snapshot TEXT NOT NULL,
    chain_attempt INTEGER NOT NULL DEFAULT 1,
    auto_attempt INTEGER NOT NULL DEFAULT 0,
    max_auto_attempts INTEGER NOT NULL DEFAULT 2,
    queued_at TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    cancel_requested_at TEXT,
    last_heartbeat_at TEXT,
    error_code TEXT,
    error_message TEXT,
    retryable INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_generation_jobs_status_queued
    ON generation_jobs(status, queued_at);
CREATE INDEX IF NOT EXISTS idx_generation_jobs_parent
    ON generation_jobs(parent_job_id);
CREATE INDEX IF NOT EXISTS idx_generation_jobs_source
    ON generation_jobs(source_kind);
