CREATE TABLE IF NOT EXISTS prompt_folders (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE TABLE IF NOT EXISTS prompt_folder_favorites (
    folder_id TEXT NOT NULL REFERENCES prompt_folders(id) ON DELETE CASCADE,
    prompt_favorite_id TEXT NOT NULL REFERENCES prompt_favorites(id) ON DELETE CASCADE,
    added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (folder_id, prompt_favorite_id)
);
CREATE INDEX IF NOT EXISTS idx_prompt_folder_favorites_favorite_id ON prompt_folder_favorites(prompt_favorite_id);
INSERT OR IGNORE INTO prompt_folders (id, name, created_at) VALUES ('default', 'Default', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));
UPDATE prompt_folders SET name = '默认收藏夹' WHERE id = 'default' AND name <> '默认收藏夹';
