CREATE TABLE IF NOT EXISTS canvas_documents (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    document_path TEXT NOT NULL,
    preview_path TEXT,
    width INTEGER NOT NULL DEFAULT 0,
    height INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    deleted_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_canvas_documents_project_id ON canvas_documents(project_id);
CREATE INDEX IF NOT EXISTS idx_canvas_documents_updated_at ON canvas_documents(updated_at);
CREATE INDEX IF NOT EXISTS idx_canvas_documents_deleted_at ON canvas_documents(deleted_at);
