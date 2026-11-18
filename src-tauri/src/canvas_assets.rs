//! Managed canvas image identities, immutable files and reference safety.
use crate::storage::artifacts::{ArtifactColumn, ArtifactKind};
#[cfg(test)]
pub(crate) use crate::storage::assets::resolve_asset;
pub(crate) use crate::storage::assets::ImageAsset as CanvasAsset;
use crate::storage::assets::{asset_for_path, AssetPaths};
use crate::{
    error::AppError,
    models::{CanvasDocumentContent, MissingCanvasAsset},
};
use rusqlite::{params, Connection};
use std::{fs, io::Write, path::Path};

pub(crate) fn write_new_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temporary = path.with_extension(format!("{}.partial", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        crate::atomic_files::publish_no_replace(&temporary, path)?;
        #[cfg(unix)]
        fs::File::open(path.parent().unwrap())?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub(crate) fn import_asset(
    db: &crate::db::Database,
    root: &Path,
    source: &Path,
) -> Result<CanvasAsset, AppError> {
    let source = source.canonicalize()?;
    if let Some(asset) = asset_for_path(db, &source.to_string_lossy())? {
        return Ok(asset);
    }
    let metadata = fs::metadata(&source)?;
    if metadata.len() > crate::source_images::MAX_SOURCE_IMAGE_BYTES {
        return Err(AppError::Validation {
            message: "Canvas image exceeds 50 MiB".into(),
        });
    }
    let bytes = fs::read(&source)?;
    // Decode before publication: invalid image data never enters the managed registry.
    crate::image_assets::decode_image_for_preview(&bytes)?;
    let id = uuid::Uuid::new_v4().to_string();
    let directory = root.join("canvas/assets");
    fs::create_dir_all(&directory)?;
    let extension = source.extension().and_then(|v| v.to_str()).unwrap_or("png");
    let path = directory.join(format!("{id}.{extension}"));
    write_new_file(&path, &bytes)?;
    if let Err(error) = db.connection()?.execute(
        "INSERT INTO canvas_assets(id,file_path,external_source) VALUES(?1,?2,?3)",
        params![
            id,
            path.to_string_lossy(),
            (!source.starts_with(root)
                && !root.canonicalize().is_ok_and(|r| source.starts_with(r)))
            .then(|| source.to_string_lossy().into_owned())
        ],
    ) {
        let _ = fs::remove_file(&path);
        return Err(error.into());
    }
    Ok(CanvasAsset {
        id,
        file_path: path.to_string_lossy().into_owned(),
    })
}

pub(crate) fn normalize_content_assets(
    db: &crate::db::Database,
    root: &Path,
    content: &mut CanvasDocumentContent,
    authorized: &impl Fn(&Path) -> Result<bool, AppError>,
) -> Result<Vec<MissingCanvasAsset>, AppError> {
    let former = crate::storage::assets::FormerRoots::read(&*db.connection()?)?;
    for request in &mut content.generation_requests {
        crate::storage::artifacts::map_json_paths(request, &mut |path| {
            Ok(former.resolve(root, path)?)
        })
        .map_err(|e| AppError::Validation {
            message: e.to_string(),
        })?;
    }
    let mut missing = Vec::new();
    for layer in &mut content.layers {
        for object in &mut layer.objects {
            if object.object_type != "image" {
                continue;
            }
            let old_path =
                former.resolve(root, object.image_path.as_deref().unwrap_or_default())?;
            if object.image_path.is_some() {
                object.image_path = Some(old_path.clone());
            }
            if let Some(id) = &object.asset_id {
                if let Some(path) = crate::storage::assets::resolve_document_asset(
                    &*db.connection()?,
                    id,
                    root,
                    &old_path,
                )? {
                    object.image_path = Some(path);
                } else {
                    missing.push(MissingCanvasAsset {
                        object_id: object.id.clone(),
                        asset_id: Some(id.clone()),
                        image_path: old_path,
                    });
                    continue;
                }
            } else if let Some(asset) = asset_for_path(db, &old_path)? {
                object.asset_id = Some(asset.id);
                object.image_path = Some(asset.file_path);
            } else if Path::new(&old_path).is_file() {
                if !authorized(Path::new(&old_path))? {
                    return Err(AppError::Validation {
                        message: "Canvas image was not selected from Astro Studio".into(),
                    });
                }
                let asset = import_asset(db, root, Path::new(&old_path))?;
                object.asset_id = Some(asset.id);
                object.image_path = Some(asset.file_path);
            }
            let path = object.image_path.clone().unwrap_or_default();
            if !Path::new(&path).is_file() {
                missing.push(MissingCanvasAsset {
                    object_id: object.id.clone(),
                    asset_id: object.asset_id.clone(),
                    image_path: path,
                });
            }
        }
    }
    Ok(missing)
}

fn contains_reference(
    value: &serde_json::Value,
    paths: &AssetPaths,
    ids: &std::collections::HashSet<String>,
) -> bool {
    match value {
        serde_json::Value::Object(map) => map.iter().any(|(key, value)| {
            if matches!(key.as_str(), "asset_id" | "assetId") {
                return value.as_str().is_some_and(|v| ids.contains(v));
            }
            if crate::storage::artifacts::path_key(key) {
                return value.as_str().is_some_and(|v| paths.contains(v))
                    || value.as_array().is_some_and(|a| {
                        a.iter()
                            .any(|v| v.as_str().is_some_and(|v| paths.contains(v)))
                    });
            }
            contains_reference(value, paths, ids)
        }),
        serde_json::Value::Array(items) => items.iter().any(|v| contains_reference(v, paths, ids)),
        _ => false,
    }
}

/// Scan legacy JSON too: protection applies before a canvas is ever reopened.
/// Fail closed when reference data cannot be read; never silently erase an asset.
pub(crate) fn generation_usages(
    conn: &Connection,
    generation_id: &str,
) -> Result<Vec<String>, String> {
    use std::collections::HashSet;
    let rows = conn
        .prepare(
            "SELECT id,file_path,thumbnail_path,managed_path FROM images WHERE generation_id=?1",
        )
        .map_err(|e| e.to_string())?
        .query_map([generation_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut paths = AssetPaths::default();
    let mut ids = HashSet::new();
    let former = crate::storage::assets::FormerRoots::read(conn).map_err(|e| e.to_string())?;
    for (id, path, thumb, relative) in rows {
        if let Some(relative) = relative {
            former.extend_references(&mut paths, &relative);
        }
        ids.insert(id);
        paths.insert(&path);
        if let Some(t) = thumb {
            paths.insert(&t);
        }
    }
    let mut usages = Vec::new();
    if ids.is_empty() {
        return Ok(usages);
    }
    let related = conn.prepare(
        "SELECT kind,id,file_path,thumbnail_path FROM library_assets WHERE generation_id IS NULL OR generation_id<>?1
         UNION ALL SELECT 'prompt extraction',id,image_path,NULL FROM prompt_extractions"
    ).map_err(|e| e.to_string())?
    .query_map([generation_id], |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?, r.get::<_,String>(2)?, r.get::<_,Option<String>>(3)?)))
    .map_err(|e| e.to_string())?.collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?;
    for (kind, id, path, thumbnail) in related {
        if paths.contains(&path) || thumbnail.as_deref().is_some_and(|p| paths.contains(p)) {
            usages.push(format!("{kind}: {id}"));
        }
    }
    let mut indexed = conn
        .prepare(
            "SELECT d.name,d.id,r.object_id FROM canvas_asset_refs r
         JOIN images i ON i.id=r.asset_id JOIN canvas_documents d ON d.id=r.document_id
         WHERE i.generation_id=?1",
        )
        .map_err(|e| e.to_string())?;
    for row in indexed
        .query_map([generation_id], |row| {
            Ok(format!(
                "Canvas: {} ({}), object {}",
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?
            ))
        })
        .map_err(|e| e.to_string())?
    {
        usages.push(row.map_err(|e| e.to_string())?);
    }
    let docs = conn
        .prepare("SELECT id,name,document_path,deleted_at FROM canvas_documents")
        .map_err(|e| e.to_string())?
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    for (id, name, path, deleted_at) in docs {
        let name = if deleted_at.is_some() {
            format!("{name} [deleted canvas]")
        } else {
            name
        };
        let bytes =
            fs::read(&path).map_err(|e| format!("Cannot check canvas {name} ({id}): {e}"))?;
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Cannot check canvas {name} ({id}): {e}"))?;
        if let Some(layers) = value.get("layers").and_then(|v| v.as_array()) {
            for layer in layers {
                if let Some(objects) = layer.get("objects").and_then(|v| v.as_array()) {
                    for object in objects {
                        if contains_reference(object, &paths, &ids) {
                            usages.push(format!(
                                "Canvas: {name} ({id}), object {}",
                                object.get("id").and_then(|v| v.as_str()).unwrap_or("image")
                            ));
                        }
                    }
                }
            }
        }
        if value
            .get("generation_requests")
            .is_some_and(|v| contains_reference(v, &paths, &ids))
        {
            usages.push(format!("Canvas recipe: {name} ({id})"));
        }
    }
    for column in crate::generation_recipe::RECIPES_ARTIFACTS {
        let table = column.table;
        let column = column.field;
        let sql =
            format!("SELECT id,{column} FROM {table} WHERE {column} IS NOT NULL AND {column}<>''");
        let rows = conn
            .prepare(&sql)
            .map_err(|e| e.to_string())?
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        for (id, text) in rows {
            let mut value: serde_json::Value = serde_json::from_str(&text)
                .map_err(|e| format!("Cannot check recipe {id}: {e}"))?;
            if column == "source_image_paths" {
                value = serde_json::json!({"source_image_paths":value});
            }
            if contains_reference(&value, &paths, &ids) {
                usages.push(format!("Recipe: {id} ({table})"));
            }
        }
    }
    usages.sort();
    usages.dedup();
    Ok(usages)
}

pub(crate) fn ensure_generation_unreferenced(conn: &Connection, id: &str) -> Result<(), String> {
    let usages = generation_usages(conn, id)?;
    if usages.is_empty() {
        Ok(())
    } else {
        Err(format!("Asset is still in use:\n{}", usages.join("\n")))
    }
}

pub(crate) const CANVAS_ARTIFACTS: &[ArtifactColumn] = &[
    ArtifactColumn::new("canvas_assets", "id", "file_path", ArtifactKind::File),
    ArtifactColumn::new(
        "canvas_documents",
        "id",
        "document_path",
        ArtifactKind::Document,
    ),
    ArtifactColumn::new("canvas_documents", "id", "preview_path", ArtifactKind::File),
];

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        db: crate::db::Database,
        root: std::path::PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("canvas-assets-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&root).unwrap();
            let db = crate::db::Database::open(&root.join("library.db")).unwrap();
            db.run_migrations().unwrap();
            Self { db, root }
        }
        fn image(&self) -> std::path::PathBuf {
            let path = self.root.join("external.png");
            image::RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255]))
                .save(&path)
                .unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn imported_image_survives_external_move_and_keeps_stable_identity() {
        let f = Fixture::new();
        let external = f.image();
        let asset = import_asset(&f.db, &f.root.join("assets"), &external).unwrap();
        fs::rename(&external, f.root.join("moved.png")).unwrap();
        assert!(Path::new(&asset.file_path).is_file());
        assert_eq!(
            resolve_asset(&f.db.connection().unwrap(), &asset.id)
                .unwrap()
                .unwrap(),
            asset.file_path
        );
        assert_eq!(image::open(&asset.file_path).unwrap().width(), 8);
    }
    #[test]
    fn reference_guard_includes_legacy_canvases_and_historical_recipes() {
        let f = Fixture::new();
        let file = f.image();
        let conn = f.db.conn.lock().unwrap();
        conn.execute("INSERT INTO generations(id,prompt,engine,status) VALUES('source','Source','gpt-image-2','completed'),('recipe','Recipe','gpt-image-2','completed')",[]).unwrap();
        conn.execute("INSERT INTO images(id,generation_id,file_path,thumbnail_path) VALUES('image','source',?1,?1)",[file.to_string_lossy().as_ref()]).unwrap();
        conn.execute(
            "UPDATE generations SET source_image_paths=?1,deleted_at='old' WHERE id='recipe'",
            [serde_json::json!([file]).to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO projects(id,name) VALUES('project','Project')",
            [],
        )
        .unwrap();
        let doc = f.root.join("old.json");
        fs::write(&doc,serde_json::json!({"layers":[{"objects":[{"type":"image","id":"object","image_path":file}]}]}).to_string()).unwrap();
        conn.execute("INSERT INTO canvas_documents(id,project_id,name,document_path) VALUES('canvas','project','Board',?1)",[doc.to_string_lossy().as_ref()]).unwrap();
        let usages = generation_usages(&conn, "source").unwrap();
        assert!(usages
            .iter()
            .any(|u| u.contains("Board") && u.contains("object")));
        assert!(usages.iter().any(|u| u.contains("recipe")));
        assert!(ensure_generation_unreferenced(&conn, "source")
            .unwrap_err()
            .contains("Board"));
        conn.execute("DELETE FROM canvas_documents", []).unwrap();
        conn.execute("DELETE FROM generations WHERE id='recipe'", [])
            .unwrap();
        ensure_generation_unreferenced(&conn, "source").unwrap();
    }
    #[test]
    fn automatic_purge_only_selects_unreferenced_expired_assets() {
        let f = Fixture::new();
        let conn = f.db.conn.lock().unwrap();
        let image = f.image();
        conn.execute("INSERT INTO generations(id,prompt,engine,status,deleted_at) VALUES('used','Used','gpt-image-2','completed','2000'),('free','Free','gpt-image-2','completed','2000'),('recipe','Recipe','gpt-image-2','completed',NULL)",[]).unwrap();
        conn.execute(
            "INSERT INTO images(id,generation_id,file_path) VALUES('image','used',?1)",
            [image.to_string_lossy().as_ref()],
        )
        .unwrap();
        conn.execute(
            "UPDATE generations SET source_image_paths=?1 WHERE id='recipe'",
            [serde_json::json!([image]).to_string()],
        )
        .unwrap();
        assert_eq!(
            crate::gallery::purge_candidates(&conn, "2026").unwrap(),
            vec!["free"]
        );
        assert!(image.is_file());
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM generations WHERE id='used'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn protected_paths_include_unopened_canvas_ids_extractions_and_shared_originals() {
        let f = Fixture::new();
        let conn = f.db.conn.lock().unwrap();
        let source = r"C:\Library\Images\source.png";
        let alias = r"\\?\C:\Library\images\source.png";
        conn.execute_batch("INSERT INTO generations(id,prompt) VALUES('original','Original'),('duplicate','Duplicate');").unwrap();
        conn.execute(
            "INSERT INTO images(id,generation_id,file_path) VALUES('key','original',?1)",
            [source],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO images(id,generation_id,file_path) VALUES('second','duplicate',?1)",
            [alias],
        )
        .unwrap();
        conn.execute("INSERT INTO prompt_extractions(id,image_path,prompt,llm_config_id) VALUES('extraction',?1,'Text','config')",[alias]).unwrap();
        let unopened = f.root.join("closed.json");
        fs::write(&unopened, serde_json::json!({"layers":[{"objects":[{"type":"image","id":"layer-image","asset_id":"key"}]}]}).to_string()).unwrap();
        conn.execute("INSERT INTO canvas_documents(id,project_id,name,document_path) VALUES('closed','default','Closed board',?1)",[unopened.to_string_lossy().as_ref()]).unwrap();
        let usages = generation_usages(&conn, "original").unwrap();
        assert!(usages
            .iter()
            .any(|s| s.contains("Closed board") && s.contains("layer-image")));
        assert!(usages.iter().any(|s| s.contains("extraction")));
        assert!(usages.iter().any(|s| s.contains("second")));
    }

    #[test]
    fn missing_legacy_image_preserves_document_and_reports_relink_target() {
        let f = Fixture::new();
        let mut content: CanvasDocumentContent=serde_json::from_value(serde_json::json!({"version":1,"viewport":{"x":0,"y":0,"scale":1},"frame":{"x":0,"y":0,"width":100,"height":100,"aspect":"1:1"},"layers":[{"id":"layer","name":"Layer","visible":true,"locked":false,"objects":[{"type":"image","id":"lost","image_path":"/missing/old.png"}]}]})).unwrap();
        let missing =
            normalize_content_assets(&f.db, &f.root, &mut content, &|_| Ok(true)).unwrap();
        assert_eq!(missing[0].object_id, "lost");
        assert_eq!(content.layers[0].objects.len(), 1);
    }
}
