use super::*;
use crate::canvas::test_support::*;

fn insert_status_job(conn: &rusqlite::Connection, status: &str) {
    conn.execute("INSERT INTO conversations (id,title,project_id,created_at,updated_at) VALUES ('status-conversation','Canvas','default','now','now')", []).unwrap();
    conn.execute("INSERT INTO generations (id,prompt,status,conversation_id) VALUES ('status-generation','draw',?1,'status-conversation')", [status]).unwrap();
    conn.execute("INSERT INTO generation_jobs (id,client_request_id,generation_id,source_kind,status,stage,request_json,provider_kind,provider_profile_id,endpoint_snapshot,queued_at,error_message) VALUES ('status-job','canvas-request','status-generation','edit',?1,'terminal','{}','openai','test','https://example.invalid','now',?2)", params![status, if status == "failed" { Some("provider unavailable") } else { None }]).unwrap();
}

#[test]
fn canvas_generation_status_scopes_requests_and_handles_not_yet_submitted() {
    let (db, db_path, document_path) = status_fixture();
    {
        let conn = db.conn.lock().unwrap();
        assert!(
            read_canvas_generation_status(&conn, "status-canvas", "canvas-request")
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            read_canvas_generation_status(&conn, "status-canvas", "other-request"),
            Err(AppError::Validation { .. })
        ));
    }
    drop(db);
    fs::remove_file(document_path).unwrap();
    remove_temp_test_db(db_path);
}

#[test]
fn canvas_generation_status_returns_exact_images_and_failure() {
    let (db, db_path, document_path) = status_fixture();
    {
        let conn = db.conn.lock().unwrap();
        insert_status_job(&conn, "completed");
        conn.execute("INSERT INTO images (id,generation_id,file_path,thumbnail_path,width,height) VALUES ('status-image','status-generation','/result.png','/thumb.png',300,200)", []).unwrap();
        let result = read_canvas_generation_status(&conn, "status-canvas", "canvas-request")
            .unwrap()
            .unwrap();
        assert_eq!(result.status, "completed");
        assert_eq!(result.generation_id.as_deref(), Some("status-generation"));
        assert_eq!(result.images.len(), 1);
        assert_eq!(result.images[0].id, "status-image");
        conn.execute(
            "UPDATE generation_jobs SET status='failed',error_code='provider_outcome_unknown',error_message='provider unavailable'",
            [],
        )
        .unwrap();
        let failed = read_canvas_generation_status(&conn, "status-canvas", "canvas-request")
            .unwrap()
            .unwrap();
        assert_eq!(failed.status, "failed");
        assert_eq!(
            failed.error_code.as_deref(),
            Some("provider_outcome_unknown")
        );
        assert_eq!(failed.error.as_deref(), Some("provider unavailable"));
    }
    drop(db);
    fs::remove_file(document_path).unwrap();
    remove_temp_test_db(db_path);
}

#[test]
fn canvas_generation_status_rejects_cross_project_results() {
    let (db, db_path, document_path) = status_fixture();
    {
        let conn = db.conn.lock().unwrap();
        insert_status_job(&conn, "running");
        conn.execute("INSERT INTO projects (id,name,created_at,updated_at) VALUES ('other-project','Other','now','now')", []).unwrap();
        conn.execute(
            "UPDATE conversations SET project_id='other-project' WHERE id='status-conversation'",
            [],
        )
        .unwrap();
        assert!(matches!(
            read_canvas_generation_status(&conn, "status-canvas", "canvas-request"),
            Err(AppError::Validation { .. })
        ));
    }
    drop(db);
    fs::remove_file(document_path).unwrap();
    remove_temp_test_db(db_path);
}
