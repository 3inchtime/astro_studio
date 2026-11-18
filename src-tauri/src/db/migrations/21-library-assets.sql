-- Both historical tables retain their IDs and their existing write contracts.
-- The view stays consistent during writes, restoration and path repairs.
CREATE INDEX idx_images_file_path ON images(file_path);
CREATE INDEX idx_images_thumbnail_path ON images(thumbnail_path);
ALTER TABLE images ADD COLUMN managed_path TEXT;
ALTER TABLE canvas_assets ADD COLUMN managed_path TEXT;
ALTER TABLE canvas_assets ADD COLUMN external_source TEXT;
CREATE TABLE library_asset_roots(prefix TEXT PRIMARY KEY);
CREATE TABLE former_asset_roots(prefix TEXT PRIMARY KEY);
CREATE VIEW library_assets AS
    SELECT id, 'generated' AS kind, generation_id, file_path, thumbnail_path, managed_path, NULL AS external_source FROM images
    UNION ALL
    SELECT id, 'canvas', NULL, file_path, NULL, managed_path, external_source FROM canvas_assets;

-- Roots contain native and canonical spellings. Store only root-relative logical
-- names; a path repair or archive rewrite updates the derived location in place.
CREATE VIEW library_relative_paths AS
    SELECT a.kind, a.id, substr(replace(a.file_path, char(92), '/'), length(r.prefix)+2) AS relative_path
    FROM library_assets a JOIN library_asset_roots r
    ON substr(replace(a.file_path,char(92),'/'),1,length(r.prefix)+1)=r.prefix||'/'
    OR ((r.prefix GLOB '[A-Za-z]:/*' OR r.prefix GLOB '//*')
        AND substr(replace(a.file_path,char(92),'/'),1,length(r.prefix)+1)=(r.prefix||'/') COLLATE NOCASE);

CREATE TRIGGER images_managed_insert AFTER INSERT ON images
BEGIN
    UPDATE images SET managed_path=(
        SELECT relative_path FROM library_relative_paths WHERE kind='generated' AND id=NEW.id LIMIT 1
    ) WHERE id=NEW.id;
END;

CREATE TRIGGER images_managed_path AFTER UPDATE OF file_path ON images
BEGIN
    UPDATE images SET managed_path=(
        SELECT relative_path FROM library_relative_paths WHERE kind='generated' AND id=NEW.id LIMIT 1
    ) WHERE id=NEW.id;
END;

CREATE TRIGGER canvas_assets_managed_insert AFTER INSERT ON canvas_assets
BEGIN
    UPDATE canvas_assets SET managed_path=(
        SELECT relative_path FROM library_relative_paths WHERE kind='canvas' AND id=NEW.id LIMIT 1
    ) WHERE id=NEW.id;
END;

CREATE TRIGGER canvas_assets_managed_path AFTER UPDATE OF file_path ON canvas_assets
BEGIN
    UPDATE canvas_assets SET managed_path=(
        SELECT relative_path FROM library_relative_paths WHERE kind='canvas' AND id=NEW.id LIMIT 1
    ) WHERE id=NEW.id;
END;
