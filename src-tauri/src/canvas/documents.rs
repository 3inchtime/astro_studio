use super::{decode_base64_bytes, ensure_canvas_dirs};
use crate::models::{
    CanvasDocument, CanvasDocumentContent, CanvasDocumentWithContent, CanvasFrame, CanvasLayer,
    CanvasObject, CanvasViewport,
};
use crate::{current_timestamp, db::Database, error::AppError};
use rusqlite::params;
use std::{fs, path::Path};

const DEFAULT_CANVAS_NAME: &str = "Untitled Canvas";

fn row_to_canvas_document(row: &rusqlite::Row) -> rusqlite::Result<CanvasDocument> {
    Ok(CanvasDocument {
        revision: row.get("revision")?,
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        name: row.get("name")?,
        document_path: row.get("document_path")?,
        preview_path: row.get("preview_path")?,
        width: row.get("width")?,
        height: row.get("height")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

pub(super) fn default_canvas_content() -> CanvasDocumentContent {
    CanvasDocumentContent {
        version: 2,
        generation_requests: vec![],
        viewport: CanvasViewport {
            x: 0.0,
            y: 0.0,
            scale: 1.0,
        },
        frame: CanvasFrame {
            x: 0.0,
            y: 0.0,
            width: 1024.0,
            height: 1024.0,
            aspect: "1:1".to_string(),
        },
        layers: vec![CanvasLayer {
            id: "layer-1".to_string(),
            name: "Sketch".to_string(),
            visible: true,
            locked: false,
            objects: Vec::<CanvasObject>::new(),
        }],
    }
}

pub(super) fn write_json_file(
    path: &Path,
    content: &CanvasDocumentContent,
) -> Result<(), AppError> {
    let bytes = serde_json::to_vec_pretty(content).map_err(|e| AppError::Database {
        message: format!("Serialize canvas document failed: {}", e),
    })?;
    crate::canvas_assets::write_new_file(path, &bytes).map_err(|e| AppError::FileSystem {
        message: format!("Write canvas document failed: {}", e),
    })?;
    Ok(())
}

pub(super) fn read_canvas_document(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<CanvasDocument, AppError> {
    conn.query_row(
        "SELECT id, project_id, name, document_path, preview_path, width, height, created_at, updated_at, deleted_at, revision
         FROM canvas_documents WHERE id = ?1 AND deleted_at IS NULL",
        params![id],
        row_to_canvas_document,
    )
    .map_err(|e| AppError::Database {
        message: format!("Read canvas document failed: {}", e),
    })
}

pub(crate) fn create(
    db: &Database,
    root: &Path,
    project_id: Option<&str>,
    name: Option<&str>,
) -> Result<CanvasDocument, AppError> {
    let documents_dir = ensure_canvas_dirs(root)?.documents;
    let timestamp = current_timestamp();
    let id = uuid::Uuid::new_v4().to_string();
    let name = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_CANVAS_NAME)
        .to_string();

    let document_path = documents_dir.join(format!("{id}.json"));
    write_json_file(&document_path, &default_canvas_content())?;

    let conn = db.connection()?;
    let project_id = crate::projects::resolve_project_id(&conn, project_id)
        .map_err(|message| AppError::Database { message })?;

    conn.execute(
        "INSERT INTO canvas_documents (
            id, project_id, name, document_path, preview_path, width, height, created_at, updated_at, deleted_at
         ) VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?7, NULL)",
        params![
            id,
            project_id,
            name,
            document_path.to_string_lossy().to_string(),
            1024,
            1024,
            timestamp
        ],
    )
    .map_err(|e| AppError::Database {
        message: format!("Create canvas document failed: {}", e),
    })?;

    read_canvas_document(&conn, &id)
}

pub(crate) fn list(
    db: &Database,
    project_id: Option<&str>,
) -> Result<Vec<CanvasDocument>, AppError> {
    let conn = db.connection()?;
    let project_id = crate::projects::resolve_project_id(&conn, project_id)
        .map_err(|message| AppError::Database { message })?;

    let mut stmt = conn
        .prepare(
            "SELECT id, project_id, name, document_path, preview_path, width, height, created_at, updated_at, deleted_at, revision
             FROM canvas_documents
             WHERE project_id = ?1 AND deleted_at IS NULL
             ORDER BY updated_at DESC, created_at DESC",
        )
        .map_err(|e| AppError::Database {
            message: format!("Prepare list canvas documents failed: {}", e),
        })?;

    let documents = stmt
        .query_map(params![project_id], row_to_canvas_document)
        .map_err(|e| AppError::Database {
            message: format!("List canvas documents failed: {}", e),
        })?
        .filter_map(|row| row.ok())
        .collect();

    Ok(documents)
}

pub(crate) fn load(
    db: &Database,
    root: &Path,
    id: &str,
) -> Result<CanvasDocumentWithContent, AppError> {
    let (mut document, file) = {
        let conn = db.connection()?;
        let document = read_canvas_document(&conn, id)?;
        let file = fs::File::open(&document.document_path)?;
        (document, file)
    };
    let mut content: CanvasDocumentContent =
        serde_json::from_reader(std::io::BufReader::new(file))?;
    let before = serde_json::to_vec(&content)?;
    let missing_assets =
        crate::canvas_assets::normalize_content_assets(db, root, &mut content, &|_| Ok(true))?;
    if serde_json::to_vec(&content)? != before {
        document = save_revision(db, id, &content, None, document.revision)?;
    }
    Ok(CanvasDocumentWithContent {
        document,
        content,
        missing_assets,
    })
}

pub(crate) fn save(
    db: &Database,
    root: &Path,
    id: &str,
    mut content: CanvasDocumentContent,
    preview: Option<&str>,
    expected_revision: i64,
    authorized: &impl Fn(&Path) -> Result<bool, AppError>,
) -> Result<CanvasDocument, AppError> {
    let existing = read_canvas_document(&*db.connection()?, id)?;
    if existing.revision != expected_revision {
        return Err(AppError::CanvasRevisionConflict {
            expected_revision,
            actual_revision: existing.revision,
        });
    }
    crate::canvas_assets::normalize_content_assets(db, root, &mut content, authorized)?;
    save_revision(db, id, &content, preview, expected_revision)
}

/// Files are immutable. SQLite only points to a fully written, synced revision;
/// a failed write/commit or interrupted process leaves the prior revision intact.
fn save_revision(
    db: &Database,
    id: &str,
    content: &CanvasDocumentContent,
    preview: Option<&str>,
    expected_revision: i64,
) -> Result<CanvasDocument, AppError> {
    let existing = read_canvas_document(&*db.connection()?, id)?;
    if existing.revision != expected_revision {
        return Err(AppError::CanvasRevisionConflict {
            expected_revision,
            actual_revision: existing.revision,
        });
    }
    let revision = existing
        .revision
        .checked_add(1)
        .ok_or_else(|| AppError::Validation {
            message: "Canvas revision limit reached".into(),
        })?;
    let parent =
        Path::new(&existing.document_path)
            .parent()
            .ok_or_else(|| AppError::Validation {
                message: "Canvas document has no directory".into(),
            })?;
    let path = parent.join(format!("{}-r{revision}-{}.json", id, uuid::Uuid::new_v4()));
    write_json_file(&path, content)?;
    let preview_path = preview.and_then(|data| {
        let bytes = decode_base64_bytes(data).ok()?;
        let file = parent.join(format!("{}-preview-{}.png", id, uuid::Uuid::new_v4()));
        crate::canvas_assets::write_new_file(&file, &bytes).ok()?;
        Some(file)
    });
    let result = (|| -> Result<CanvasDocument, AppError> {
        let mut conn = db.connection()?;
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let updated=tx.execute(
            "UPDATE canvas_documents SET document_path=?1,revision=?2,width=?3,height=?4,updated_at=?5,preview_path=COALESCE(?8,preview_path) WHERE id=?6 AND revision=?7",
            params![path.to_string_lossy(),revision,content.frame.width.round() as i32,content.frame.height.round() as i32,current_timestamp(),id,expected_revision,preview_path.as_ref().map(|p|p.to_string_lossy())]
        )?;
        if updated != 1 {
            return Err(AppError::CanvasRevisionConflict {
                expected_revision,
                actual_revision: read_canvas_document(&tx, id)?.revision,
            });
        }
        tx.execute("DELETE FROM canvas_asset_refs WHERE document_id=?1", [id])?;
        for layer in &content.layers {
            for object in &layer.objects {
                if let Some(asset_id) = &object.asset_id {
                    tx.execute("INSERT INTO canvas_asset_refs(document_id,object_id,asset_id) VALUES (?1,?2,?3)",params![id,object.id,asset_id])?;
                }
            }
        }
        let saved = read_canvas_document(&tx, id)?;
        tx.commit()?;
        Ok(saved)
    })();
    if result.is_err() {
        let _ = fs::remove_file(path);
        if let Some(path) = preview_path {
            let _ = fs::remove_file(path);
        }
    } else {
        for old in std::iter::once(&existing.document_path).chain(
            existing
                .preview_path
                .as_ref()
                .filter(|_| preview_path.is_some()),
        ) {
            let in_use=db.connection().ok().and_then(|conn|conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM canvas_documents WHERE document_path=?1 OR preview_path=?1)",[old],|r|r.get::<_,bool>(0)
            ).ok());
            if in_use == Some(false) {
                let _ = fs::remove_file(old);
            }
        }
    }
    result
}

pub(crate) fn rename(db: &Database, id: &str, name: &str) -> Result<CanvasDocument, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation {
            message: "Canvas name cannot be empty.".to_string(),
        });
    }

    let timestamp = current_timestamp();
    let conn = db.connection()?;
    conn.execute(
        "UPDATE canvas_documents SET name = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
        params![name, timestamp, id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Rename canvas document failed: {}", e),
    })?;

    read_canvas_document(&conn, id)
}

pub(crate) fn delete(tx: &rusqlite::Transaction<'_>, id: &str) -> Result<(), AppError> {
    let timestamp = current_timestamp();
    tx.execute(
        "UPDATE canvas_documents SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Delete canvas document failed: {}", e),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests;
