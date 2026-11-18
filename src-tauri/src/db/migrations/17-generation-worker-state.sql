ALTER TABLE generation_jobs
    ADD COLUMN stage TEXT NOT NULL DEFAULT 'migration_unknown';
UPDATE generation_jobs
    SET stage = CASE status
        WHEN 'queued' THEN 'queued'
        WHEN 'running' THEN 'startup_reconciliation'
        WHEN 'completed' THEN 'terminal'
        WHEN 'failed' THEN 'terminal'
        WHEN 'cancelled' THEN 'terminal'
        WHEN 'interrupted' THEN 'terminal'
        ELSE 'migration_unknown'
    END
    WHERE stage = 'migration_unknown';
CREATE TABLE IF NOT EXISTS generation_recoveries (
    generation_id TEXT PRIMARY KEY REFERENCES generations(id) ON DELETE CASCADE,
    request_kind TEXT NOT NULL,
    request_state TEXT NOT NULL,
    output_format TEXT NOT NULL,
    response_file TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_generation_recoveries_state
    ON generation_recoveries(request_state);
INSERT INTO generation_recoveries (
    generation_id, request_kind, request_state, output_format, response_file,
    created_at, updated_at
)
SELECT j.generation_id, g.request_kind, 'requesting', g.output_format, NULL,
       j.queued_at, j.queued_at
  FROM generation_jobs j
  JOIN generations g ON g.id = j.generation_id
 WHERE j.status = 'queued'
   AND j.stage = 'queued'
   AND NOT EXISTS (
       SELECT 1 FROM generation_recoveries r
        WHERE r.generation_id = j.generation_id
   );
ALTER TABLE generation_recoveries ADD COLUMN expected_response_file TEXT;
ALTER TABLE generation_recoveries
    ADD COLUMN response_size INTEGER
    CHECK (
        response_size IS NULL
        OR (typeof(response_size) = 'integer' AND response_size >= 0)
    );
ALTER TABLE generation_recoveries ADD COLUMN response_sha256 TEXT;
CREATE TABLE IF NOT EXISTS generation_worker_lease (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    owner_id TEXT,
    fencing_epoch INTEGER NOT NULL DEFAULT 0
        CHECK (typeof(fencing_epoch) = 'integer' AND fencing_epoch >= 0),
    acquired_at INTEGER
        CHECK (acquired_at IS NULL OR (typeof(acquired_at) = 'integer' AND acquired_at >= 0)),
    heartbeat_at INTEGER
        CHECK (heartbeat_at IS NULL OR (typeof(heartbeat_at) = 'integer' AND heartbeat_at >= 0)),
    expires_at INTEGER
        CHECK (expires_at IS NULL OR (typeof(expires_at) = 'integer' AND expires_at >= 0)),
    CHECK (
        (
            owner_id IS NULL
            AND acquired_at IS NULL
            AND heartbeat_at IS NULL
            AND expires_at IS NULL
        )
        OR (
            typeof(owner_id) = 'text'
            AND length(trim(owner_id)) > 0
            AND acquired_at IS NOT NULL
            AND heartbeat_at IS NOT NULL
            AND expires_at IS NOT NULL
            AND fencing_epoch > 0
            AND acquired_at <= heartbeat_at
            AND heartbeat_at < expires_at
        )
    )
);
INSERT OR IGNORE INTO generation_worker_lease (
    id, owner_id, fencing_epoch, acquired_at, heartbeat_at, expires_at
) VALUES (1, NULL, 0, NULL, NULL, NULL);
