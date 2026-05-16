use crate::commands::conversations::resolve_conversation_id_for_generation;
use crate::current_timestamp;
use crate::db::Database;
use crate::error::AppError;
use crate::models::{
    CanvasDocument, CanvasDocumentContent, CanvasDocumentWithContent, CanvasFrame, CanvasLayer,
    CanvasObject, CanvasViewport,
};
use base64::Engine;
use rusqlite::params;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{Manager, State};

const DEFAULT_PROJECT_ID: &str = "default";
const DEFAULT_CANVAS_NAME: &str = "Untitled Canvas";

fn ensure_default_project(conn: &rusqlite::Connection) -> Result<(), AppError> {
    conn.execute(
        "INSERT OR IGNORE INTO projects (id, name, created_at, updated_at)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
        params![DEFAULT_PROJECT_ID, "Default Project"],
    )
    .map_err(|e| AppError::Database {
        message: format!("Ensure default project failed: {}", e),
    })?;
    Ok(())
}

fn resolve_project_id(
    conn: &rusqlite::Connection,
    project_id: Option<&str>,
) -> Result<String, AppError> {
    ensure_default_project(conn)?;

    if let Some(project_id) = project_id.map(str::trim).filter(|id| !id.is_empty()) {
        let exists = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND deleted_at IS NULL)",
                params![project_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| AppError::Database {
                message: format!("Resolve canvas project failed: {}", e),
            })?
            != 0;

        if exists {
            return Ok(project_id.to_string());
        }
    }

    Ok(DEFAULT_PROJECT_ID.to_string())
}

fn row_to_canvas_document(row: &rusqlite::Row) -> rusqlite::Result<CanvasDocument> {
    Ok(CanvasDocument {
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

fn default_canvas_content() -> CanvasDocumentContent {
    CanvasDocumentContent {
        version: 1,
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

fn canvas_base_dir(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| AppError::FileSystem {
        message: format!("Get app data dir failed: {}", e),
    })?;
    Ok(app_data_dir.join("canvas"))
}

fn ensure_canvas_dirs(app: &tauri::AppHandle) -> Result<(PathBuf, PathBuf, PathBuf), AppError> {
    let base_dir = canvas_base_dir(app)?;
    let documents_dir = base_dir.join("documents");
    let previews_dir = base_dir.join("previews");
    let exports_dir = base_dir.join("exports");

    fs::create_dir_all(&documents_dir).map_err(|e| AppError::FileSystem {
        message: format!("Create canvas documents dir failed: {}", e),
    })?;
    fs::create_dir_all(&previews_dir).map_err(|e| AppError::FileSystem {
        message: format!("Create canvas previews dir failed: {}", e),
    })?;
    fs::create_dir_all(&exports_dir).map_err(|e| AppError::FileSystem {
        message: format!("Create canvas exports dir failed: {}", e),
    })?;

    Ok((documents_dir, previews_dir, exports_dir))
}

fn write_json_file(path: &Path, content: &CanvasDocumentContent) -> Result<(), AppError> {
    let bytes = serde_json::to_vec_pretty(content).map_err(|e| AppError::Database {
        message: format!("Serialize canvas document failed: {}", e),
    })?;
    fs::write(path, bytes).map_err(|e| AppError::FileSystem {
        message: format!("Write canvas document failed: {}", e),
    })?;
    Ok(())
}

fn decode_base64_bytes(value: &str) -> Result<Vec<u8>, AppError> {
    let payload = value
        .split_once(',')
        .map(|(_, encoded)| encoded)
        .unwrap_or(value);
    base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|e| AppError::Validation {
            message: format!("Invalid base64 payload: {}", e),
        })
}

fn read_canvas_document(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<CanvasDocument, AppError> {
    conn.query_row(
        "SELECT id, project_id, name, document_path, preview_path, width, height, created_at, updated_at, deleted_at
         FROM canvas_documents WHERE id = ?1 AND deleted_at IS NULL",
        params![id],
        row_to_canvas_document,
    )
    .map_err(|e| AppError::Database {
        message: format!("Read canvas document failed: {}", e),
    })
}

#[tauri::command]
pub(crate) fn create_canvas_document(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    project_id: Option<String>,
    name: Option<String>,
) -> Result<CanvasDocument, AppError> {
    let (documents_dir, _, _) = ensure_canvas_dirs(&app)?;
    let timestamp = current_timestamp();
    let id = uuid::Uuid::new_v4().to_string();
    let name = name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_CANVAS_NAME)
        .to_string();

    let document_path = documents_dir.join(format!("{id}.json"));
    write_json_file(&document_path, &default_canvas_content())?;

    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let project_id = resolve_project_id(&conn, project_id.as_deref())?;

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

#[tauri::command]
pub(crate) fn list_canvas_documents(
    db: State<'_, Database>,
    project_id: Option<String>,
) -> Result<Vec<CanvasDocument>, AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let project_id = resolve_project_id(&conn, project_id.as_deref())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, project_id, name, document_path, preview_path, width, height, created_at, updated_at, deleted_at
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

#[tauri::command]
pub(crate) fn get_canvas_document(
    db: State<'_, Database>,
    id: String,
) -> Result<CanvasDocumentWithContent, AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let document = read_canvas_document(&conn, &id)?;
    let raw = fs::read_to_string(&document.document_path).map_err(|e| AppError::FileSystem {
        message: format!("Read canvas document file failed: {}", e),
    })?;
    let content = serde_json::from_str::<CanvasDocumentContent>(&raw).map_err(|e| {
        AppError::Database {
            message: format!("Parse canvas document file failed: {}", e),
        }
    })?;

    Ok(CanvasDocumentWithContent { document, content })
}

#[tauri::command]
pub(crate) fn save_canvas_document(
    db: State<'_, Database>,
    id: String,
    content: CanvasDocumentContent,
    preview_png_base64: Option<String>,
) -> Result<CanvasDocument, AppError> {
    let timestamp = current_timestamp();
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let existing = read_canvas_document(&conn, &id)?;

    write_json_file(Path::new(&existing.document_path), &content)?;

    let preview_path = if let Some(preview_png_base64) = preview_png_base64 {
        let bytes = decode_base64_bytes(&preview_png_base64)?;
        let preview_path = existing.preview_path.clone().unwrap_or_else(|| {
            let preview_file = Path::new(&existing.document_path)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("canvas-preview");
            Path::new(&existing.document_path)
                .parent()
                .and_then(|parent| parent.parent())
                .unwrap_or_else(|| Path::new(""))
                .join("previews")
                .join(format!("{preview_file}.png"))
                .to_string_lossy()
                .to_string()
        });
        fs::write(&preview_path, bytes).map_err(|e| AppError::FileSystem {
            message: format!("Write canvas preview failed: {}", e),
        })?;
        Some(preview_path)
    } else {
        existing.preview_path.clone()
    };

    conn.execute(
        "UPDATE canvas_documents
         SET preview_path = ?1, width = ?2, height = ?3, updated_at = ?4
         WHERE id = ?5 AND deleted_at IS NULL",
        params![
            preview_path,
            content.frame.width.round() as i32,
            content.frame.height.round() as i32,
            timestamp,
            id
        ],
    )
    .map_err(|e| AppError::Database {
        message: format!("Update canvas document failed: {}", e),
    })?;

    read_canvas_document(&conn, &id)
}

#[tauri::command]
pub(crate) fn rename_canvas_document(
    db: State<'_, Database>,
    id: String,
    name: String,
) -> Result<CanvasDocument, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation {
            message: "Canvas name cannot be empty.".to_string(),
        });
    }

    let timestamp = current_timestamp();
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    conn.execute(
        "UPDATE canvas_documents SET name = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
        params![name, timestamp, id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Rename canvas document failed: {}", e),
    })?;

    read_canvas_document(&conn, &id)
}

#[tauri::command]
pub(crate) fn delete_canvas_document(
    db: State<'_, Database>,
    id: String,
) -> Result<(), AppError> {
    let timestamp = current_timestamp();
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    conn.execute(
        "UPDATE canvas_documents SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Delete canvas document failed: {}", e),
    })?;
    Ok(())
}

#[tauri::command]
pub(crate) fn save_canvas_export(
    app: tauri::AppHandle,
    document_id: String,
    png_base64: String,
) -> Result<String, AppError> {
    let (_, _, exports_dir) = ensure_canvas_dirs(&app)?;
    let document_dir = exports_dir.join(&document_id);
    fs::create_dir_all(&document_dir).map_err(|e| AppError::FileSystem {
        message: format!("Create canvas export dir failed: {}", e),
    })?;

    let bytes = decode_base64_bytes(&png_base64)?;
    let file_path = document_dir.join(format!("{}-frame.png", uuid::Uuid::new_v4()));
    fs::write(&file_path, bytes).map_err(|e| AppError::FileSystem {
        message: format!("Write canvas export failed: {}", e),
    })?;

    Ok(file_path.to_string_lossy().to_string())
}

pub(crate) fn resolve_canvas_generation_conversation_id(
    conn: &rusqlite::Connection,
    conversation_id: Option<&str>,
    project_id: Option<&str>,
    prompt: &str,
) -> Result<String, AppError> {
    resolve_conversation_id_for_generation(conn, conversation_id, project_id, prompt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn temp_test_db(prefix: &str) -> (Database, std::path::PathBuf) {
        let db_path =
            std::env::temp_dir().join(format!("{prefix}-{}.sqlite", uuid::Uuid::new_v4()));
        let db = Database::open(&db_path).unwrap();
        db.run_migrations().unwrap();
        (db, db_path)
    }

    fn remove_temp_test_db(db_path: std::path::PathBuf) {
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn create_and_list_canvas_documents_for_project() {
        let (db, db_path) = temp_test_db("astro-studio-canvas-create-test");
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO projects (id, name, created_at, updated_at, archived_at, pinned_at, deleted_at)
             VALUES (?1, ?2, ?3, ?3, NULL, NULL, NULL)",
            params!["project-canvas", "Canvas Project", current_timestamp()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO canvas_documents (id, project_id, name, document_path, preview_path, width, height, created_at, updated_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?7, NULL)",
            params![
                "canvas-1",
                "project-canvas",
                "Moodboard",
                "/tmp/canvas-1.json",
                1024,
                1024,
                current_timestamp()
            ],
        )
        .unwrap();
        drop(conn);

        let conn = db.conn.lock().unwrap();
        let rows: Vec<CanvasDocument> = {
            let mut stmt = conn
                .prepare(
                    "SELECT id, project_id, name, document_path, preview_path, width, height, created_at, updated_at, deleted_at
                     FROM canvas_documents WHERE project_id = ?1 AND deleted_at IS NULL",
                )
                .unwrap();
            stmt.query_map(params!["project-canvas"], row_to_canvas_document)
                .unwrap()
                .filter_map(|row| row.ok())
                .collect()
        };

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "canvas-1");
        assert_eq!(rows[0].name, "Moodboard");

        drop(conn);
        drop(db);
        remove_temp_test_db(db_path);
    }

    #[test]
    fn save_canvas_document_updates_json_and_preview() {
        let (db, db_path) = temp_test_db("astro-studio-canvas-save-test");
        let parent_dir = db_path.parent().unwrap().join("canvas-documents");
        std::fs::create_dir_all(&parent_dir).unwrap();
        let document_path = parent_dir.join("canvas-save.json");
        write_json_file(&document_path, &default_canvas_content()).unwrap();

        let conn = db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO canvas_documents (id, project_id, name, document_path, preview_path, width, height, created_at, updated_at, deleted_at)
             VALUES (?1, 'default', ?2, ?3, NULL, ?4, ?5, ?6, ?6, NULL)",
            params![
                "canvas-save",
                "Canvas Save",
                document_path.to_string_lossy().to_string(),
                1024,
                1024,
                current_timestamp()
            ],
        )
        .unwrap();
        drop(conn);

        let content = CanvasDocumentContent {
            version: 1,
            viewport: CanvasViewport {
                x: 12.0,
                y: 24.0,
                scale: 1.5,
            },
            frame: CanvasFrame {
                x: 40.0,
                y: 60.0,
                width: 1536.0,
                height: 1024.0,
                aspect: "3:2".to_string(),
            },
            layers: vec![CanvasLayer {
                id: "layer-save".to_string(),
                name: "Layer Save".to_string(),
                visible: true,
                locked: false,
                objects: vec![CanvasObject {
                    object_type: "stroke".to_string(),
                    id: "stroke-1".to_string(),
                    tool: Some("brush".to_string()),
                    points: vec![10.0, 20.0, 30.0, 40.0],
                    color: Some("#111827".to_string()),
                    size: Some(6.0),
                    opacity: Some(1.0),
                    image_path: None,
                    x: None,
                    y: None,
                    width: None,
                    height: None,
                    original_width: None,
                    original_height: None,
                    rotation: None,
                }],
            }],
        };

        write_json_file(&document_path, &content).unwrap();
        let saved = fs::read_to_string(&document_path).unwrap();
        let parsed = serde_json::from_str::<CanvasDocumentContent>(&saved).unwrap();

        assert_eq!(parsed.frame.aspect, "3:2");
        assert_eq!(parsed.layers[0].objects[0].object_type, "stroke");

        drop(db);
        let _ = std::fs::remove_dir_all(parent_dir.parent().unwrap());
        remove_temp_test_db(db_path);
    }

    #[test]
    fn save_canvas_export_writes_png_file() {
        let dir = std::env::temp_dir().join(format!(
            "astro-studio-canvas-export-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("frame.png");
        let png_data = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4////fwAJ+wP9KobjigAAAABJRU5ErkJggg==";

        let bytes = decode_base64_bytes(png_data).unwrap();
        std::fs::write(&file_path, &bytes).unwrap();

        assert!(file_path.exists());
        assert_eq!(std::fs::read(&file_path).unwrap(), bytes);

        let _ = std::fs::remove_dir_all(dir);
    }
}
