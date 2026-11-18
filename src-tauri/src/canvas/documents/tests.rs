use super::*;
use crate::canvas::test_support::*;

#[test]
fn canvas_revision_schema_starts_at_zero_and_tracks_managed_assets() {
    let (db, path, document) = status_fixture();
    let conn = db.conn.lock().unwrap();
    let revision: i64 = conn
        .query_row(
            "SELECT revision FROM canvas_documents WHERE id='status-canvas'",
            [],
            |r| r.get(0),
        )
        .expect("canvas revision migration");
    assert_eq!(revision, 0);
    conn.execute(
        "INSERT INTO canvas_assets(id,file_path) VALUES ('asset','/managed/image.png')",
        [],
    )
    .expect("managed asset registry");
    drop(conn);
    drop(db);
    remove_temp_test_db(path);
    fs::remove_file(document).unwrap();
}

#[test]
fn revision_save_rejects_stale_writer_and_retires_previous_snapshot() {
    let (db, db_path, old_path) = status_fixture();
    let mut next = default_canvas_content();
    next.frame.width = 777.0;
    let saved = save_revision(&db, "status-canvas", &next, None, 0).unwrap();
    assert_eq!(saved.revision, 1);
    assert_ne!(saved.document_path, old_path.to_string_lossy());
    assert!(
        !old_path.exists(),
        "committed autosaves retire the previous snapshot"
    );
    next.frame.width = 888.0;
    assert!(matches!(
        save_revision(&db, "status-canvas", &next, None, 0),
        Err(AppError::CanvasRevisionConflict {
            actual_revision: 1,
            ..
        })
    ));
    let current: CanvasDocumentContent =
        serde_json::from_slice(&fs::read(&saved.document_path).unwrap()).unwrap();
    assert_eq!(current.frame.width, 777.0);
    fs::remove_file(saved.document_path).unwrap();
    drop(db);
    remove_temp_test_db(db_path);
    let _ = fs::remove_file(old_path);
}

#[test]
fn concurrent_revision_writers_publish_only_the_winning_preview_and_document() {
    let (db, db_path, original) = status_fixture();
    let start = std::sync::Arc::new(std::sync::Barrier::new(2));
    let workers = [411.0, 522.0].map(|width| {
        let db = db.clone();
        let start = start.clone();
        std::thread::spawn(move || {
            let mut content = default_canvas_content();
            content.frame.width = width;
            start.wait();
            save_revision(&db, "status-canvas", &content, Some("cHJldmlldw=="), 0)
        })
    });
    let results = workers.map(|worker| worker.join().unwrap());
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(
                r,
                Err(AppError::CanvasRevisionConflict {
                    actual_revision: 1,
                    ..
                })
            ))
            .count(),
        1
    );
    let saved = read_canvas_document(&db.connection().unwrap(), "status-canvas").unwrap();
    assert_eq!(saved.revision, 1);
    assert!(Path::new(&saved.document_path).is_file());
    let preview = saved.preview_path.unwrap();
    assert_eq!(std::fs::read(&preview).unwrap(), b"preview");
    assert!(!original.exists());
    std::fs::remove_file(saved.document_path).unwrap();
    std::fs::remove_file(preview).unwrap();
    drop(db);
    remove_temp_test_db(db_path);
}

#[test]
fn revision_save_database_failure_leaves_previous_document_readable() {
    let (db, db_path, old_path) = status_fixture();
    let old_bytes = fs::read(&old_path).unwrap();
    db.connection().unwrap().execute_batch("CREATE TRIGGER reject_canvas_save BEFORE UPDATE ON canvas_documents BEGIN SELECT RAISE(ABORT,'disk failure'); END;").unwrap();
    assert!(save_revision(&db, "status-canvas", &default_canvas_content(), None, 0).is_err());
    assert_eq!(
        read_canvas_document(&db.connection().unwrap(), "status-canvas")
            .unwrap()
            .revision,
        0
    );
    assert_eq!(fs::read(&old_path).unwrap(), old_bytes);
    drop(db);
    remove_temp_test_db(db_path);
    let _ = fs::remove_file(old_path);
}

#[test]
fn revision_write_failure_keeps_previous_content_and_revision() {
    let (db, db_path, old_path) = status_fixture();
    let old_bytes = fs::read(&old_path).unwrap();
    let id = "x".repeat(300); // Real filesystem ENAMETOOLONG, before publishing any revision.
    db.connection()
        .unwrap()
        .execute(
            "UPDATE canvas_documents SET id=?1 WHERE id='status-canvas'",
            [&id],
        )
        .unwrap();
    assert!(matches!(
        save_revision(&db, &id, &default_canvas_content(), None, 0),
        Err(AppError::FileSystem { .. })
    ));
    assert_eq!(
        read_canvas_document(&db.connection().unwrap(), &id)
            .unwrap()
            .revision,
        0
    );
    assert_eq!(fs::read(&old_path).unwrap(), old_bytes);
    drop(db);
    remove_temp_test_db(db_path);
    fs::remove_file(old_path).unwrap();
}

#[test]
fn revision_save_preview_failure_does_not_reject_body() {
    let (db, db_path, old_path) = status_fixture();
    let mut next = default_canvas_content();
    next.frame.width = 444.0;
    let saved = save_revision(&db, "status-canvas", &next, Some("invalid base64"), 0).unwrap();
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.width, 444);
    assert!(saved.preview_path.is_none());
    fs::remove_file(saved.document_path).unwrap();
    drop(db);
    remove_temp_test_db(db_path);
    let _ = fs::remove_file(old_path);
}

#[test]
fn canvas_v2_shapes_and_generation_requests_roundtrip() {
    let raw = serde_json::json!({
        "version": 2,
        "viewport": {"x":0,"y":0,"scale":1},
        "frame": {"x":0,"y":0,"width":100,"height":100,"aspect":"1:1"},
        "layers": [{"id":"l","name":"Layer","visible":true,"locked":false,"objects":[
            {"type":"rectangle","id":"r","fill":"#f00","color":"#000","x":1,"y":2,"width":20,"height":30,"size":2,"opacity":1,"rotation":0},
            {"type":"text","id":"t","text":"hello","fontSize":24,"fontFamily":"serif","fontWeight":"bold","fontStyle":"italic","textAlign":"center","letterSpacing":3,"lineHeight":1.8,"rotation":45,"x":1,"y":2,"width":20,"height":30},
            {"type":"arrow","id":"a","points":[0,0,100,0],"size":1,"headSize":6}
        ]}],
        "generation_requests":[{"request_id":"req","status":"pending","image_ids":[]}]
    });
    let content: CanvasDocumentContent = serde_json::from_value(raw).unwrap();
    let saved = serde_json::to_value(content).unwrap();
    assert_eq!(saved["layers"][0]["objects"][0]["fill"], "#f00");
    assert_eq!(saved["layers"][0]["objects"][1]["text"], "hello");
    assert_eq!(saved["layers"][0]["objects"][1]["fontSize"], 24.0);
    let text = &saved["layers"][0]["objects"][1];
    assert_eq!(text["fontFamily"], "serif");
    assert_eq!(text["fontWeight"], "bold");
    assert_eq!(text["fontStyle"], "italic");
    assert_eq!(text["textAlign"], "center");
    assert_eq!(text["letterSpacing"], 3.0);
    assert_eq!(text["lineHeight"], 1.8);
    assert_eq!(text["rotation"], 45.0);
    assert_eq!(saved["generation_requests"][0]["request_id"], "req");
    assert_eq!(saved["layers"][0]["objects"][2]["headSize"], 6.0);
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
    drop(conn);
    let project = Some("project-canvas");
    let root = db_path.with_extension("assets");
    let created = create(&db, &root, project, Some(" Moodboard ")).unwrap();
    let rows = list(&db, project).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, created.id);
    assert_eq!(rows[0].name, "Moodboard");
    assert!(list(&db, None).unwrap().is_empty());

    assert_eq!(rename(&db, &created.id, "Final").unwrap().name, "Final");
    let loaded = load(&db, &root, &created.id).unwrap();
    assert_eq!(loaded.document.name, "Final");
    assert_eq!(loaded.content.version, 2);
    {
        let mut conn = db.connection().unwrap();
        let tx = conn.transaction().unwrap();
        delete(&tx, &created.id).unwrap();
        tx.commit().unwrap();
    }
    assert!(list(&db, project).unwrap().is_empty());
    fs::remove_dir_all(root).unwrap();
    drop(db);
    remove_temp_test_db(db_path);
}

#[test]
fn save_canvas_document_updates_json_and_preview() {
    let (db, db_path) = temp_test_db("astro-studio-canvas-save-test");
    let parent_dir = db_path.with_extension("canvas-documents");
    std::fs::create_dir_all(&parent_dir).unwrap();
    let document_path = parent_dir.join("canvas-save.json");
    write_json_file(&document_path, &default_canvas_content()).unwrap();

    db.connection().unwrap().execute(
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

    let content = CanvasDocumentContent {
        version: 2,
        generation_requests: vec![],
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
                asset_id: None,
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
                fill: None,
                text: None,
                font_size: None,
                font_family: None,
                font_weight: None,
                font_style: None,
                text_align: None,
                letter_spacing: None,
                line_height: None,
                head_size: None,
            }],
        }],
    };

    let document = save(
        &db,
        &parent_dir,
        "canvas-save",
        content,
        Some("cHJldmlldw=="),
        0,
        &|_| Ok(false),
    )
    .unwrap();
    assert_eq!(
        fs::read(document.preview_path.as_ref().unwrap()).unwrap(),
        b"preview"
    );
    let saved = fs::read_to_string(&document.document_path).unwrap();
    let parsed = serde_json::from_str::<CanvasDocumentContent>(&saved).unwrap();

    assert_eq!(parsed.frame.aspect, "3:2");
    assert_eq!(parsed.layers[0].objects[0].object_type, "stroke");

    drop(db);
    let _ = std::fs::remove_dir_all(parent_dir);
    remove_temp_test_db(db_path);
}
