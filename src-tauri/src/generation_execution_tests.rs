use super::*;
use crate::api_gateway::read_complete_http_request;
use base64::Engine;
use std::io::Cursor;
use tauri::Listener;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

fn submit(
    db: &Database,
    request: GenerationExecutionRequest,
) -> Result<GenerationJobTransition<EnqueueGenerationResult>, AppError> {
    enqueue(db, request, |_| Ok(()))
}

#[tokio::test]
async fn queued_and_replayed_requests_survive_reopen_and_run_serially() {
    // A real HTTP provider fixture pauses the first response until cancellation.
    tokio::time::timeout(Duration::from_secs(15), exercise_generation_queue())
        .await
        .expect("queue fixtures finish without waiting for a paid provider");
}

async fn exercise_generation_queue() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let root = std::env::temp_dir().join(format!("astro-queue-{}", uuid::Uuid::new_v4()));
    let paths = crate::storage::StoragePaths::new(root.clone(), root.join("test.db"), root.clone())
        .unwrap();
    let db = Database::open(&paths.database).unwrap();
    db.run_migrations().unwrap();
    crate::providers::save_model_provider_profiles_state(
        &db,
        ENGINE_GPT_IMAGE_2,
        ModelProviderProfilesState {
            active_provider_id: "fixture".into(),
            profiles: vec![ModelProviderProfile {
                id: "fixture".into(),
                name: "Fixture".into(),
                api_key: "fixture-credential".into(),
                endpoint_settings: EndpointSettings {
                    mode: ENDPOINT_MODE_FULL_URL.into(),
                    base_url: url.clone(),
                    generation_url: format!("{url}/generate"),
                    edit_url: format!("{url}/edit"),
                },
            }],
        },
    )
    .unwrap();
    let request = |id: &str| GenerationExecutionRequest {
        client_request_id: Some(id.into()),
        prompt: "Queue test".into(),
        model: Some(ENGINE_GPT_IMAGE_2.into()),
        ..Default::default()
    };
    let missing = submit(&db, request("missing recovery")).unwrap().value;
    let corrupt = submit(&db, request("damaged recovery")).unwrap().value;
    let first = submit(&db, request("first")).unwrap();
    let replay = submit(&db, request("first")).unwrap();
    assert!(replay.event.is_none());
    assert_eq!(replay.value.job_id, first.value.job_id);
    assert_eq!(replay.value.conversation_id, first.value.conversation_id);
    let mut original_profiles =
        crate::providers::read_model_provider_profiles_state(&db, ENGINE_GPT_IMAGE_2).unwrap();
    let mut new_profile = original_profiles.profiles[0].clone();
    new_profile.id = "new-active".into();
    new_profile.endpoint_settings.generation_url = "https://new.example.invalid/generate".into();
    original_profiles.profiles.push(new_profile);
    original_profiles.active_provider_id = "new-active".into();
    crate::providers::save_model_provider_profiles_state(
        &db,
        ENGINE_GPT_IMAGE_2,
        original_profiles.clone(),
    )
    .unwrap();
    let after_switch = enqueue(&db, request("first"), |_| {
        panic!("replay must bypass file authorization")
    })
    .unwrap();
    assert_eq!(after_switch.value.job_id, first.value.job_id);
    original_profiles.active_provider_id = "fixture".into();
    crate::providers::save_model_provider_profiles_state(
        &db,
        ENGINE_GPT_IMAGE_2,
        original_profiles,
    )
    .unwrap();
    let mut changed = request("first");
    changed.prompt = "Conflicting payload".into();
    assert!(matches!(
        submit(&db, changed),
        Err(AppError::GenerationJobIdempotencyConflict)
    ));

    let mut png = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(4, 4)
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let source = root.join("source.png");
    std::fs::write(&source, png.get_ref()).unwrap();
    let document = uuid::Uuid::new_v4().to_string();
    db.conn.lock().unwrap().execute("INSERT INTO canvas_documents(id,project_id,name,document_path,width,height,created_at,updated_at) VALUES(?1,'default','Fixture','fixture.json',4,4,?2,?2)", rusqlite::params![document, crate::current_timestamp()]).unwrap();
    let mut edit = request("second");
    edit.kind = GenerationLifecycleKind::Edit;
    edit.document_id = Some(document.clone());
    edit.source_image_paths = vec![source
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .into_owned()];
    let second = submit(&db, edit.clone()).unwrap();
    let cancelled = submit(&db, request("third")).unwrap();
    let rejected =
        generation_jobs::request_cancel(&db.conn.lock().unwrap(), &cancelled.value.job_id).unwrap();
    assert_eq!(rejected.status, GenerationJobStatus::Cancelled);
    assert!(matches!(
        read_result(&db, &cancelled.value.job_id),
        Err(AppError::GenerationJobCancelled)
    ));
    assert!(generation_jobs::request_cancel_with_event(
        &db.conn.lock().unwrap(),
        &cancelled.value.job_id
    )
    .unwrap()
    .event
    .is_none());

    assert_eq!(
        second.event.as_ref().unwrap().source_ref["document_id"],
        document
    );
    drop(db);

    let db = Database::open(&paths.database).unwrap();
    let engine = crate::api_gateway::GptImageEngine::new(&Default::default()).unwrap();
    let recovered = crate::generation_recovery::recover_pending(&db, &engine, &root, None, None)
        .await
        .unwrap();
    assert_eq!(recovered.interrupted, 0);
    let app = tauri::test::mock_app();
    app.manage(GenerationRuntime::default());
    app.manage(crate::storage::MaintenanceGate::default());
    app.manage(paths);
    app.manage(db);
    app.manage(engine);
    let db = app.state::<Database>();
    // Fixture vaults are per connection; restore the ephemeral credential after reopen.
    let mut profiles =
        crate::providers::read_model_provider_profiles_state(&db, ENGINE_GPT_IMAGE_2).unwrap();
    profiles
        .profiles
        .iter_mut()
        .find(|profile| profile.id == "fixture")
        .unwrap()
        .api_key = "fixture-credential".into();
    crate::providers::save_model_provider_profiles_state(&db, ENGINE_GPT_IMAGE_2, profiles)
        .unwrap();
    let removed = submit(&db, request("remove")).unwrap().value;
    let cancel_scope = DeletionScope::Conversation(&removed.conversation_id);
    // A failed delete must roll back cancellation together with the parent.
    let rejected = delete_with_jobs(app.handle(), &db, cancel_scope, |_| {
        Err(AppError::GenerationJobInvalidTransition)
    });
    assert!(rejected.is_err());
    assert_eq!(
        generation_jobs::get_job(&db.conn.lock().unwrap(), &removed.job_id)
            .unwrap()
            .status,
        GenerationJobStatus::Queued
    );
    delete_with_jobs(
        app.handle(),
        &db,
        DeletionScope::Conversation(&removed.conversation_id),
        |tx| crate::projects::conversations::delete_conversation(tx, &removed.conversation_id),
    )
    .unwrap();
    {
        let conn = db.conn.lock().unwrap();
        let listed = generation_jobs::list_visible_jobs(&conn, &Default::default()).unwrap();
        assert_eq!(listed.items.len(), 5);
        let scoped = generation_jobs::list_visible_jobs(
            &conn,
            &GenerationJobFilter {
                document_id: Some(document.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(scoped.items[0].id, second.value.job_id);
    }
    let (events, mut observed) = tokio::sync::mpsc::unbounded_channel();
    app.listen("generation:job", move |event| {
        events
            .send(serde_json::from_str::<GenerationJobEvent>(event.payload()).unwrap())
            .unwrap();
    });
    let mut missing_waiter = Box::pin(await_result(app.handle(), &db, &missing.job_id));
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut missing_waiter)
            .await
            .is_err()
    );
    db.connection()
        .unwrap()
        .execute(
            "DELETE FROM generation_recoveries WHERE generation_id=?1",
            [&missing.generation_id],
        )
        .unwrap();
    db.connection().unwrap().execute(
        "UPDATE generation_recoveries SET expected_response_file='invalid-binding' WHERE generation_id=?1",
        [&corrupt.generation_id],
    ).unwrap();
    let mut waiting = Box::pin(await_result(app.handle(), &db, &corrupt.job_id));
    assert!(
        tokio::time::timeout(Duration::from_millis(10), &mut waiting)
            .await
            .is_err()
    );
    assert!(
        tokio::time::timeout(Duration::from_secs(1), process_next(app.handle()))
            .await
            .expect("corrupt queued request is retired before any later HTTP")
            .unwrap()
    );
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(1), &mut waiting)
            .await
            .unwrap(),
        Err(AppError::ProviderOutcomeUnknown { .. })
    ));
    let failed = observed.try_recv().expect("terminal queue event");
    assert_eq!(failed.job_id, corrupt.job_id);
    assert_eq!(failed.status, GenerationJobStatus::Failed);
    let retained: String = db
        .connection()
        .unwrap()
        .query_row(
            "SELECT expected_response_file FROM generation_recoveries WHERE generation_id=?1",
            [&corrupt.generation_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(retained, "invalid-binding");
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(1), &mut missing_waiter)
            .await
            .unwrap(),
        Err(AppError::GenerationJobCorruptPersistedData)
    ));
    drop(missing_waiter);
    drop(waiting);
    let first_handle = app.handle().clone();
    let second_handle = app.handle().clone();
    let worker_one = tokio::spawn(async move { process_next(&first_handle).await });
    let (mut held, _) = listener.accept().await.unwrap();
    assert!(read_complete_http_request(&mut held)
        .await
        .starts_with(b"POST /generate "));
    let worker_two = tokio::spawn(async move { process_next(&second_handle).await });
    assert_eq!(
        generation_jobs::get_job(&db.conn.lock().unwrap(), &second.value.job_id)
            .unwrap()
            .status,
        GenerationJobStatus::Queued
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(40), listener.accept())
            .await
            .is_err()
    );

    let cancel =
        generation_jobs::request_cancel_with_event(&db.conn.lock().unwrap(), &first.value.job_id)
            .unwrap();
    publish_transition(app.handle(), cancel);
    let (mut provider, _) = listener.accept().await.unwrap();
    // Read the entire multipart body before closing the response socket.
    assert!(read_complete_http_request(&mut provider)
        .await
        .starts_with(b"POST /edit "));
    let encoded = base64::engine::general_purpose::STANDARD.encode(png.get_ref());
    let body = serde_json::json!({"data":[{"b64_json":encoded}]}).to_string();
    provider.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
    drop(provider);
    assert!(worker_one.await.unwrap().unwrap());
    assert!(worker_two.await.unwrap().unwrap());
    assert!(!process_next(app.handle()).await.unwrap());
    let finished = await_result(app.handle(), &db, &second.value.job_id)
        .await
        .unwrap();
    assert_eq!(finished.images.len(), 1);
    assert!(std::path::Path::new(&finished.images[0].file_path).is_file());
    assert_eq!(
        submit(&db, edit).unwrap().value.status,
        GenerationJobStatus::Completed
    );
    assert_eq!(
        generation_jobs::get_job(&db.conn.lock().unwrap(), &first.value.job_id)
            .unwrap()
            .status,
        GenerationJobStatus::Cancelled
    );
    let stages: Vec<_> = std::iter::from_fn(|| observed.try_recv().ok())
        .filter(|event| event.job_id == second.value.job_id)
        .map(|event| {
            assert_eq!(event.source_ref["document_id"], document);
            assert_eq!(event.project_id, "default");
            assert_eq!(event.conversation_id, second.value.conversation_id);
            event.stage
        })
        .collect();
    assert_eq!(
        stages,
        [
            GenerationJobStage::Preparing,
            GenerationJobStage::ProviderRequest,
            GenerationJobStage::LocalProcessing,
            GenerationJobStage::Terminal
        ]
    );
    drop(app);
    std::fs::remove_dir_all(root).unwrap();
}
