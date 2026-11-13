//! Managed canvas image identities, immutable files and reference safety.
use crate::{
    error::AppError,
    models::{CanvasDocumentContent, MissingCanvasAsset},
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
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

#[derive(Debug, Serialize)]
pub(crate) struct CanvasAsset {
    pub id: String,
    pub file_path: String,
}

pub(crate) fn resolve_asset(conn: &Connection, id: &str) -> Result<Option<String>, AppError> {
    Ok(conn.query_row("SELECT file_path FROM canvas_assets WHERE id=?1 UNION ALL SELECT file_path FROM images WHERE id=?1 LIMIT 1",[id],|r|r.get(0)).optional()?)
}

fn asset_for_path(conn: &Connection, path: &str) -> Result<Option<CanvasAsset>, AppError> {
    Ok(conn.query_row("SELECT id,file_path FROM canvas_assets WHERE file_path=?1 UNION ALL SELECT id,file_path FROM images WHERE file_path=?1 LIMIT 1",[path],|r|Ok(CanvasAsset{id:r.get(0)?,file_path:r.get(1)?})).optional()?)
}

pub(crate) fn import_asset(
    conn: &Connection,
    root: &Path,
    source: &Path,
) -> Result<CanvasAsset, AppError> {
    let source = source.canonicalize()?;
    if let Some(asset) = asset_for_path(conn, &source.to_string_lossy())? {
        return Ok(asset);
    }
    let metadata = fs::metadata(&source)?;
    if metadata.len() > crate::commands::generation::MAX_SOURCE_IMAGE_BYTES {
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
    if let Err(error) = conn.execute(
        "INSERT INTO canvas_assets(id,file_path) VALUES(?1,?2)",
        params![id, path.to_string_lossy()],
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
    conn: &Connection,
    root: &Path,
    content: &mut CanvasDocumentContent,
    authorized: &impl Fn(&Path) -> Result<bool, AppError>,
) -> Result<Vec<MissingCanvasAsset>, AppError> {
    let mut missing = Vec::new();
    for layer in &mut content.layers {
        for object in &mut layer.objects {
            if object.object_type != "image" {
                continue;
            }
            let old_path = object.image_path.clone().unwrap_or_default();
            if let Some(id) = &object.asset_id {
                if let Some(path) = resolve_asset(conn, id)? {
                    object.image_path = Some(path);
                } else {
                    missing.push(MissingCanvasAsset {
                        object_id: object.id.clone(),
                        asset_id: Some(id.clone()),
                        image_path: old_path,
                    });
                    continue;
                }
            } else if let Some(asset) = asset_for_path(conn, &old_path)? {
                object.asset_id = Some(asset.id);
                object.image_path = Some(asset.file_path);
            } else if Path::new(&old_path).is_file() {
                if !authorized(Path::new(&old_path))? {
                    return Err(AppError::Validation {
                        message: "Canvas image was not selected from Astro Studio".into(),
                    });
                }
                let asset = import_asset(conn, root, Path::new(&old_path))?;
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
    paths: &std::collections::HashSet<String>,
    ids: &std::collections::HashSet<String>,
) -> bool {
    match value {
        serde_json::Value::Object(map) => map.iter().any(|(key, value)| {
            if key == "asset_id" {
                return value.as_str().is_some_and(|v| ids.contains(v));
            }
            if matches!(
                key.as_str(),
                "image_path"
                    | "source_image_path"
                    | "sourceImagePath"
                    | "maskPath"
                    | "markupPath"
                    | "mask_path"
                    | "markup_path"
                    | "source_image_paths"
                    | "sourceImagePaths"
            ) {
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
        .prepare("SELECT id,file_path,thumbnail_path FROM images WHERE generation_id=?1")
        .map_err(|e| e.to_string())?
        .query_map([generation_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut paths = HashSet::new();
    let mut ids = HashSet::new();
    for (id, path, thumb) in rows {
        ids.insert(id);
        paths.insert(path);
        if let Some(t) = thumb {
            paths.insert(t);
        }
    }
    let mut usages = Vec::new();
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
    for (table, columns) in [
        ("generations", ["source_image_paths", "request_metadata"]),
        ("generation_jobs", ["request_json", "source_ref_json"]),
    ] {
        for column in columns {
            let sql = format!(
                "SELECT id,{column} FROM {table} WHERE {column} IS NOT NULL AND {column}<>''"
            );
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
        let conn = f.db.conn.lock().unwrap();
        let asset = import_asset(&conn, &f.root.join("assets"), &external).unwrap();
        fs::rename(&external, f.root.join("moved.png")).unwrap();
        assert!(Path::new(&asset.file_path).is_file());
        assert_eq!(
            resolve_asset(&conn, &asset.id).unwrap().unwrap(),
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
    fn missing_legacy_image_preserves_document_and_reports_relink_target() {
        let f = Fixture::new();
        let conn = f.db.conn.lock().unwrap();
        let mut content: CanvasDocumentContent=serde_json::from_value(serde_json::json!({"version":1,"viewport":{"x":0,"y":0,"scale":1},"frame":{"x":0,"y":0,"width":100,"height":100,"aspect":"1:1"},"layers":[{"id":"layer","name":"Layer","visible":true,"locked":false,"objects":[{"type":"image","id":"lost","image_path":"/missing/old.png"}]}]})).unwrap();
        let missing =
            normalize_content_assets(&conn, &f.root, &mut content, &|_| Ok(true)).unwrap();
        assert_eq!(missing[0].object_id, "lost");
        assert_eq!(content.layers[0].objects.len(), 1);
    }
}
