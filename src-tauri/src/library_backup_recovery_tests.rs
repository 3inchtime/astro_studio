use super::*;
use crate::api_gateway::ProviderAttemptBody;
use crate::generation_jobs::{enqueue_job, GenerationJobOptions, PreparedGenerationJob};
use crate::generation_lifecycle::{FileResponseArtifactStore, ResponseArtifactStore};
use crate::models::GenerationJobStatus;
struct Fixture {
    root: PathBuf,
    db: crate::db::Database,
    job_id: String,
}
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("astro-recovery-test-{}", uuid::Uuid::new_v4()));
        Self::at(root)
    }
    fn at(root: PathBuf) -> Self {
        std::fs::create_dir_all(&root).unwrap();
        let db = crate::db::Database::open(&root.join("fixture.db")).unwrap();
        db.run_migrations().unwrap();
        let job_id = uuid::Uuid::new_v4().to_string();
        let generation_id = uuid::Uuid::new_v4().to_string();
        let prepared = PreparedGenerationJob {
            job_id: job_id.clone(),
            client_request_id: uuid::Uuid::new_v4().to_string(),
            generation_id: generation_id.clone(),
            requested_conversation_id: None,
            requested_project_id: Some("default".into()),
            prompt: "recovery fixture".into(),
            model: "gpt-image-2".into(),
            request_kind: "generate".into(),
            size: "1024x1024".into(),
            quality: "high".into(),
            background: "auto".into(),
            output_format: "png".into(),
            output_compression: 100,
            moderation: "auto".into(),
            input_fidelity: "high".into(),
            image_count: 1,
            stream: false,
            partial_images: 0,
            source_image_paths: vec![],
            request_options: GenerationJobOptions {
                size: Some("1024x1024".into()),
                quality: Some("high".into()),
                background: Some("auto".into()),
                output_format: Some("png".into()),
                output_compression: Some(100),
                moderation: Some("auto".into()),
                input_fidelity: Some("high".into()),
                image_count: Some(1),
                stream: Some(false),
                partial_images: Some(0),
                ..Default::default()
            },
            parent_job_id: None,
            source_kind: "generate".into(),
            source_ref: serde_json::json!({}),
            provider_kind: "openai".into(),
            provider_profile_id: "fixture-profile".into(),
            endpoint_snapshot: "https://example.invalid/images/generations".into(),
            status: GenerationJobStatus::Queued,
            chain_attempt: 1,
            auto_attempt: 0,
            max_auto_attempts: 2,
            queued_at: "2026-07-10T00:00:00Z".into(),
            finished_at: None,
            error_code: None,
            error_message: None,
            retryable: false,
        };
        enqueue_job(&mut db.conn.lock().unwrap(), &prepared).unwrap();
        Self { root, db, job_id }
    }

    async fn save_response(&self, ready: bool) -> PathBuf {
        use crate::generation_jobs::{
            begin_generation_job_write_transaction, claim_job_in_transaction,
            load_generation_execution_snapshot, transition_running_job_stage_with_event,
            WorkerStageTransition,
        };
        use crate::generation_worker_lease::{
            acquire_worker_lease, release_worker_lease, WorkerLeaseAcquireOutcome,
        };
        let snapshot = {
            let mut conn = self.db.conn.lock().unwrap();
            let tx = begin_generation_job_write_transaction(&mut conn).unwrap();
            claim_job_in_transaction(&tx, &self.job_id).unwrap();
            tx.commit().unwrap();
            load_generation_execution_snapshot(&conn, &self.job_id).unwrap()
        };
        let store = FileResponseArtifactStore::new(self.root.clone());
        let path = store.expected_response_path(&snapshot.context).unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let authority = {
            let conn = self.db.conn.lock().unwrap();
            let WorkerLeaseAcquireOutcome::Acquired { authority, .. } =
                acquire_worker_lease(&conn, "fixture", now, std::time::Duration::from_secs(60))
                    .unwrap()
            else {
                panic!("lease")
            };
            transition_running_job_stage_with_event(
                &conn,
                &self.job_id,
                crate::models::GenerationJobStage::Preparing,
                WorkerStageTransition::BeginProviderRequest {
                    expected_response_file: path.clone(),
                },
                &authority,
                now,
            )
            .unwrap();
            authority
        };
        use base64::Engine;
        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(2, 2)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let response = store.persist_verified_response(&snapshot.context, ProviderAttemptBody { body_text: serde_json::json!({"data":[{"b64_json":base64::engine::general_purpose::STANDARD.encode(png.into_inner())}]}).to_string(), requested_image_count: 1 }).await.unwrap();
        if ready {
            crate::generation_lifecycle::promote_verified_response_fenced(
                &self.db,
                &store,
                &snapshot.context,
                &response,
                &authority,
                now,
            )
            .unwrap();
        }
        release_worker_lease(&self.db.conn.lock().unwrap(), &authority).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn normalize(fixture: &Fixture) -> Result<BTreeMap<String, PathBuf>> {
    let paths = StoragePaths::new(
        fixture.root.join("bootstrap"),
        fixture.root.join("fixture.db"),
        fixture.root.clone(),
    )
    .unwrap();
    let temp = fixture.root.join("normalized");
    fs::create_dir_all(&temp).unwrap();
    let ops = Operations::default();
    let op = ops.begin(&uuid::Uuid::new_v4().to_string()).unwrap();
    prepare_snapshot_files(&fixture.db.conn.lock().unwrap(), &paths, &temp, &op)
}
#[test]
fn requesting_backup_keeps_request_stage_for_verified_startup_promotion() {
    let fixture = Fixture::new();
    tauri::async_runtime::block_on(fixture.save_response(false));
    let overrides = normalize(&fixture).unwrap();
    assert_eq!(overrides.len(), 1);
    let conn = fixture.db.conn.lock().unwrap();
    let state:(String,Option<String>,String)=conn.query_row("SELECT r.request_state,r.response_file,j.stage FROM generation_recoveries r JOIN generation_jobs j ON j.generation_id=r.generation_id",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(
        state,
        ("requesting".into(), None, "provider_request".into())
    );
}
#[test]
fn ready_backup_rejects_database_checksum_disagreeing_with_verified_file() {
    let fixture = Fixture::new();
    tauri::async_runtime::block_on(fixture.save_response(true));
    fixture
        .db
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE generation_recoveries SET response_sha256=?1",
            ["0".repeat(64)],
        )
        .unwrap();
    assert!(normalize(&fixture).is_err());
}
#[test]
fn backup_rejects_extra_envelope_fields_and_wrong_response_job_identity() {
    let fixture = Fixture::new();
    let path = tauri::async_runtime::block_on(fixture.save_response(true));
    let original = fs::read(&path).unwrap();
    let mut envelope: Value = serde_json::from_slice(&original).unwrap();
    envelope["authorization"] = Value::String("secret".into());
    fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
    assert!(normalize(&fixture).is_err());
    fs::write(&path, original).unwrap();
    let wrong = fixture.root.join("other.response.json");
    fs::copy(&path, &wrong).unwrap();
    fixture
        .db
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE generation_recoveries SET response_file=?1,expected_response_file=?1",
            [wrong.to_string_lossy().as_ref()],
        )
        .unwrap();
    assert!(normalize(&fixture).is_err());
}

#[test]
fn normalized_backup_restores_to_verified_recoverable_job_without_provider_submission() {
    for ready in [false, true] {
        let fixture = Fixture::new();
        tauri::async_runtime::block_on(fixture.save_response(ready));
        let paths = StoragePaths::new(
            fixture.root.join("bootstrap"),
            fixture.root.join("fixture.db"),
            fixture.root.clone(),
        )
        .unwrap();
        let operations = Operations::default();
        let op = operations.begin(&uuid::Uuid::new_v4().to_string()).unwrap();
        let archive = fixture.root.join("recovery.astrobackup");
        create_archive(&fixture.db, &paths, &archive, &op, &|_, _, _| {}).unwrap();
        let restored = restore_archive(&paths, &archive, &op, &|_, _, _| {}).unwrap();
        let db = Database::open(&restored.join("library.db")).unwrap();
        let engine =
            crate::api_gateway::GptImageEngine::new(&crate::config::ApiConfig::default()).unwrap();
        let summary = tauri::async_runtime::block_on(crate::generation_recovery::recover_pending(
            &db,
            &engine,
            &restored.join("assets"),
            None,
            None,
        ))
        .unwrap();
        assert_eq!(summary.recovered, 1, "ready={ready}");
        let count: i64 = db
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        assert_eq!(
            tauri::async_runtime::block_on(crate::generation_recovery::recover_pending(
                &db,
                &engine,
                &restored.join("assets"),
                None,
                None
            ))
            .unwrap()
            .recovered,
            0
        );
    }
}
