use super::documents::{default_canvas_content, write_json_file};
use crate::db::Database;
use rusqlite::params;
use std::path::PathBuf;

pub(super) fn temp_test_db(prefix: &str) -> (Database, std::path::PathBuf) {
    let db_path = std::env::temp_dir().join(format!("{prefix}-{}.sqlite", uuid::Uuid::new_v4()));
    let db = Database::open(&db_path).unwrap();
    db.run_migrations().unwrap();
    (db, db_path)
}

pub(super) fn remove_temp_test_db(db_path: std::path::PathBuf) {
    let _ = std::fs::remove_file(&db_path);
    let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
    let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
}

pub(super) fn status_fixture() -> (Database, PathBuf, PathBuf) {
    let (db, db_path) = temp_test_db("astro-canvas-status");
    let document_path = db_path.with_extension("json");
    let mut content = default_canvas_content();
    content.generation_requests.push(serde_json::json!({
        "request_id": "canvas-request", "project_id": "default", "status": "pending"
    }));
    write_json_file(&document_path, &content).unwrap();
    {
        let conn = db.connection().unwrap();
        crate::projects::resolve_project_id(&conn, None).unwrap();
        conn.execute("INSERT INTO canvas_documents (id,project_id,name,document_path,width,height,created_at,updated_at) VALUES ('status-canvas','default','Status',?1,100,100,'now','now')", params![document_path.to_string_lossy()]).unwrap();
    }
    (db, db_path, document_path)
}
