CREATE TABLE IF NOT EXISTS generations (
    id TEXT PRIMARY KEY,
    prompt TEXT NOT NULL,
    engine TEXT NOT NULL DEFAULT 'gpt-image-2',
    size TEXT NOT NULL DEFAULT '1024x1024',
    quality TEXT NOT NULL DEFAULT 'auto',
    status TEXT NOT NULL DEFAULT 'pending',
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE TABLE IF NOT EXISTS images (
    id TEXT PRIMARY KEY,
    generation_id TEXT NOT NULL REFERENCES generations(id) ON DELETE CASCADE,
    file_path TEXT NOT NULL,
    thumbnail_path TEXT,
    width INTEGER NOT NULL DEFAULT 0,
    height INTEGER NOT NULL DEFAULT 0,
    file_size INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_images_generation_id ON images(generation_id);
CREATE INDEX IF NOT EXISTS idx_generations_created_at ON generations(created_at);
