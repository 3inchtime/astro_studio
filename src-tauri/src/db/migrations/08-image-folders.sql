CREATE TABLE IF NOT EXISTS folders (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE TABLE IF NOT EXISTS folder_images (
    folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (folder_id, image_id)
);
CREATE INDEX IF NOT EXISTS idx_folder_images_image_id ON folder_images(image_id);
INSERT OR IGNORE INTO folders (id, name, created_at)
    VALUES ('default', '默认收藏', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));
