ALTER TABLE canvas_documents ADD COLUMN revision INTEGER NOT NULL DEFAULT 0;
CREATE TABLE canvas_assets (id TEXT PRIMARY KEY, file_path TEXT NOT NULL UNIQUE);
CREATE TABLE canvas_asset_refs (
    document_id TEXT NOT NULL REFERENCES canvas_documents(id) ON DELETE CASCADE,
    object_id TEXT NOT NULL, asset_id TEXT NOT NULL,
    PRIMARY KEY(document_id,object_id)
);
CREATE INDEX idx_canvas_asset_refs_asset ON canvas_asset_refs(asset_id);
