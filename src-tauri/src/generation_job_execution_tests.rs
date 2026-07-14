use super::*;
use crate::api_gateway::{EngineCallError, PreparedEditImage};
use crate::generation_jobs::{
    claim_next_job_fenced_with_event, enqueue_job,
    load_generation_execution_snapshot_for_stage_in_transaction, request_cancel,
    transition_running_job_stage_with_event, GenerationJobOptions, PreparedGenerationJob,
};
use crate::generation_lifecycle::{
    persist_provider_attempt_response, promote_verified_response_fenced,
    GenerationExecutionContext, LocalGenerationFileStore,
};
use crate::generation_worker_lease::{acquire_worker_lease, WorkerLeaseAcquireOutcome};
use crate::models::{
    EndpointSettings, GptImageRequestOptions, ModelProviderProfile, ModelProviderProfilesState,
};
use async_trait::async_trait;
use chrono::SecondsFormat;
use rusqlite::params;
use serde_json::json;
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Mutex, TryLockError};
use tokio::sync::Notify;

const SECRET_ONE: &str = "sk-execution-secret-one";
const SECRET_TWO: &str = "sk-execution-secret-two";

fn assert_send_sync_static<T: Send + Sync + 'static>() {}

#[test]
fn adapter_and_seams_are_send_sync_static() {
    assert_send_sync_static::<GenerationJobExecutionAdapter>();
    assert_send_sync_static::<Arc<dyn GenerationExecutionEventSink>>();
    assert_send_sync_static::<Arc<dyn GenerationExecutionDiagnosticSink>>();
}

struct Fixture {
    root: PathBuf,
    db: Database,
    authority: WorkerTransitionAuthority,
    job_id: String,
    generation_id: String,
    source_path: Option<PathBuf>,
}

impl Fixture {
    fn new(max_auto_attempts: i32) -> Self {
        Self::new_for_kind(max_auto_attempts, false)
    }

    fn new_edit(max_auto_attempts: i32) -> Self {
        Self::new_for_kind(max_auto_attempts, true)
    }

    fn new_for_kind(max_auto_attempts: i32, edit: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "astro-studio-generation-execution-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("create execution fixture root");
        let root = root
            .canonicalize()
            .expect("canonicalize execution fixture root");
        let source_path = edit.then(|| root.join("source.png"));
        if let Some(path) = &source_path {
            std::fs::write(path, png_bytes()).expect("write edit source image");
        }
        let db = Database::open(&root.join("astro_studio.db")).expect("open execution database");
        db.run_migrations().expect("migrate execution database");
        save_profile(&db, SECRET_ONE);

        let now = Utc::now();
        let queued_at =
            (now - chrono::Duration::seconds(5)).to_rfc3339_opts(SecondsFormat::Secs, true);
        let job_id = uuid::Uuid::new_v4().to_string();
        let generation_id = uuid::Uuid::new_v4().to_string();
        let prepared = PreparedGenerationJob {
            job_id: job_id.clone(),
            client_request_id: uuid::Uuid::new_v4().to_string(),
            generation_id: generation_id.clone(),
            requested_conversation_id: None,
            requested_project_id: Some("default".to_string()),
            prompt: "draw an execution nebula".to_string(),
            model: "gpt-image-2".to_string(),
            request_kind: if edit { "edit" } else { "generate" }.to_string(),
            size: "1024x1024".to_string(),
            quality: "high".to_string(),
            background: "auto".to_string(),
            output_format: "png".to_string(),
            output_compression: 100,
            moderation: "auto".to_string(),
            input_fidelity: "high".to_string(),
            image_count: 1,
            stream: false,
            partial_images: 0,
            source_image_paths: source_path
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect(),
            request_options: GenerationJobOptions {
                size: Some("1024x1024".to_string()),
                quality: Some("high".to_string()),
                background: Some("auto".to_string()),
                output_format: Some("png".to_string()),
                output_compression: Some(100),
                moderation: Some("auto".to_string()),
                input_fidelity: Some("high".to_string()),
                stream: Some(false),
                partial_images: Some(0),
                image_count: Some(1),
            },
            parent_job_id: None,
            source_kind: if edit { "edit" } else { "generate" }.to_string(),
            source_ref: json!({ "id": job_id }),
            provider_kind: "openai".to_string(),
            provider_profile_id: "profile-1".to_string(),
            endpoint_snapshot: if edit {
                "https://provider.example.test/v1/images/edits"
            } else {
                "https://provider.example.test/v1/images/generations"
            }
            .to_string(),
            status: GenerationJobStatus::Queued,
            chain_attempt: 1,
            auto_attempt: 0,
            max_auto_attempts,
            queued_at,
            finished_at: None,
            error_code: None,
            error_message: None,
            retryable: false,
        };
        let now_ms = Utc::now().timestamp_millis();
        let authority = {
            let mut conn = db.conn.lock().expect("lock fixture database");
            enqueue_job(&mut conn, &prepared).expect("enqueue fixture job");
            match acquire_worker_lease(
                &conn,
                "execution-test-worker",
                now_ms,
                Duration::from_secs(60),
            )
            .expect("acquire fixture lease")
            {
                WorkerLeaseAcquireOutcome::Acquired { authority, .. } => authority,
                WorkerLeaseAcquireOutcome::Held { .. } => panic!("fixture lease unexpectedly held"),
            }
        };
        {
            let conn = db.conn.lock().expect("lock fixture claim");
            let claimed =
                claim_next_job_fenced_with_event(&conn, &authority, Utc::now().timestamp_millis())
                    .expect("claim fixture job")
                    .expect("fixture job available");
            assert_eq!(claimed.value.id, job_id);
            assert_eq!(claimed.value.stage, GenerationJobStage::Preparing);
        }
        Self {
            root,
            db,
            authority,
            job_id,
            generation_id,
            source_path,
        }
    }

    fn artifact_store(&self) -> FileResponseArtifactStore {
        FileResponseArtifactStore::new(self.root.join("responses"))
    }

    fn job(&self) -> GenerationJob {
        let conn = self.db.conn.lock().expect("lock fixture job read");
        get_job(&conn, &self.job_id).expect("read fixture job")
    }

    fn recovery_count(&self) -> i64 {
        let conn = self.db.conn.lock().expect("lock recovery count");
        conn.query_row(
            "SELECT COUNT(*) FROM generation_recoveries WHERE generation_id = ?1",
            [&self.generation_id],
            |row| row.get(0),
        )
        .expect("count recovery")
    }

    fn image_count(&self) -> i64 {
        let conn = self.db.conn.lock().expect("lock image count");
        conn.query_row(
            "SELECT COUNT(*) FROM images WHERE generation_id = ?1",
            [&self.generation_id],
            |row| row.get(0),
        )
        .expect("count images")
    }

    fn request_cancel(&self) {
        let conn = self.db.conn.lock().expect("lock cancel request");
        request_cancel(&conn, &self.job_id).expect("persist cancellation");
    }

    async fn make_response_ready(&self) {
        let store = self.artifact_store();
        let snapshot = {
            let conn = self.db.conn.lock().expect("lock response fixture");
            let tx = conn.unchecked_transaction().expect("open snapshot tx");
            let snapshot = load_generation_execution_snapshot_for_stage_in_transaction(
                &tx,
                &self.job_id,
                GenerationJobStage::Preparing,
            )
            .expect("load preparing snapshot");
            tx.commit().expect("commit snapshot read");
            snapshot
        };
        let expected_response_file = store
            .response_path(&snapshot.context)
            .expect("derive fixture response path");
        {
            let conn = self.db.conn.lock().expect("lock begin provider");
            transition_running_job_stage_with_event(
                &conn,
                &self.job_id,
                GenerationJobStage::Preparing,
                WorkerStageTransition::BeginProviderRequest {
                    expected_response_file,
                },
                &self.authority,
                Utc::now().timestamp_millis(),
            )
            .expect("begin fixture provider request");
        }
        let response = persist_provider_attempt_response(
            &store,
            &snapshot,
            ProviderAttemptBody {
                body_text: "{\"data\":[{\"fixture\":true}]}".to_string(),
                requested_image_count: 1,
            },
        )
        .await
        .expect("persist fixture response");
        promote_verified_response_fenced(
            &self.db,
            &store,
            &snapshot.context,
            &response,
            &self.authority,
            Utc::now().timestamp_millis(),
        )
        .expect("promote fixture response");
    }

    fn adapter(
        &self,
        engine: Arc<dyn ImageEngine>,
        decoder: Arc<dyn ImageResponseDecoder>,
        sleeper: Arc<dyn GenerationExecutionSleeper>,
        fail_events: bool,
    ) -> (
        GenerationJobExecutionAdapter,
        Arc<ObservedEvents>,
        Arc<ObservedDiagnostics>,
    ) {
        self.adapter_with_clock(
            engine,
            decoder,
            sleeper,
            Arc::new(SystemGenerationExecutionClock),
            fail_events,
        )
    }

    fn adapter_with_clock(
        &self,
        engine: Arc<dyn ImageEngine>,
        decoder: Arc<dyn ImageResponseDecoder>,
        sleeper: Arc<dyn GenerationExecutionSleeper>,
        clock: Arc<dyn GenerationExecutionClock>,
        fail_events: bool,
    ) -> (
        GenerationJobExecutionAdapter,
        Arc<ObservedEvents>,
        Arc<ObservedDiagnostics>,
    ) {
        let events = Arc::new(ObservedEvents {
            db: self.db.clone(),
            events: Mutex::new(Vec::new()),
            fail: fail_events,
        });
        let diagnostics = Arc::new(ObservedDiagnostics::default());
        let file_store = Arc::new(ObservedFileStore {
            db: self.db.clone(),
            inner: LocalGenerationFileStore::new(self.root.join("app-data")),
        });
        let adapter = GenerationJobExecutionAdapter::new(
            self.db.clone(),
            engine,
            self.artifact_store(),
            decoder,
            file_store,
            clock,
            sleeper,
            Arc::new(ZeroGenerationRetryJitter),
            events.clone(),
            diagnostics.clone(),
            AutomaticRetryPolicy::new(Duration::from_millis(1), Duration::from_secs(2)),
        );
        (adapter, events, diagnostics)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

fn save_profile(db: &Database, secret: &str) {
    settings::save_model_provider_profiles_state(
        db,
        "gpt-image-2",
        ModelProviderProfilesState {
            active_provider_id: "profile-1".to_string(),
            profiles: vec![ModelProviderProfile {
                id: "profile-1".to_string(),
                name: "Execution Provider".to_string(),
                api_key: secret.to_string(),
                endpoint_settings: EndpointSettings {
                    mode: "full_url".to_string(),
                    base_url: "https://provider.example.test/v1".to_string(),
                    generation_url: "https://provider.example.test/v1/images/generations"
                        .to_string(),
                    edit_url: "https://provider.example.test/v1/images/edits".to_string(),
                },
            }],
        },
    )
    .expect("save execution profile");
}

struct DatabaseLockAssertingClock {
    db: Database,
    now_ms: i64,
    calls: AtomicUsize,
}

impl DatabaseLockAssertingClock {
    fn new(db: &Database) -> Arc<Self> {
        Arc::new(Self {
            db: db.clone(),
            now_ms: Utc::now().timestamp_millis(),
            calls: AtomicUsize::new(0),
        })
    }
}

impl GenerationExecutionClock for DatabaseLockAssertingClock {
    fn now_ms(&self) -> i64 {
        assert!(
            matches!(self.db.conn.try_lock(), Err(TryLockError::WouldBlock)),
            "execution time must be sampled only after the repository mutex is held"
        );
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.now_ms
    }

    fn now_utc(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(self.now_ms).expect("valid lock-asserting clock timestamp")
    }
}

enum EngineOutcome {
    Success,
    Error(EngineCallError),
}

struct ScriptedEngine {
    db: Database,
    outcomes: Mutex<VecDeque<EngineOutcome>>,
    calls: AtomicUsize,
    secrets: Mutex<Vec<String>>,
    attempts: Mutex<Vec<(GenerationJobStage, i32)>>,
}

impl ScriptedEngine {
    fn new(db: &Database, outcomes: impl IntoIterator<Item = EngineOutcome>) -> Arc<Self> {
        Arc::new(Self {
            db: db.clone(),
            outcomes: Mutex::new(outcomes.into_iter().collect()),
            calls: AtomicUsize::new(0),
            secrets: Mutex::new(Vec::new()),
            attempts: Mutex::new(Vec::new()),
        })
    }

    fn run_attempt(&self, api_key: &str) -> Result<ProviderAttemptBody, EngineCallError> {
        let conn = self
            .db
            .conn
            .try_lock()
            .expect("provider await seam must not hold database mutex");
        let (stage, attempt) = conn
            .query_row(
                "SELECT stage, auto_attempt FROM generation_jobs WHERE status = 'running'",
                [],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?)),
            )
            .expect("read provider-stage projection");
        drop(conn);
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.secrets
            .lock()
            .expect("lock observed secrets")
            .push(api_key.to_string());
        self.attempts.lock().expect("lock observed attempts").push((
            match stage.as_str() {
                "provider_request" => GenerationJobStage::ProviderRequest,
                other => panic!("provider invoked from invalid stage {other}"),
            },
            attempt,
        ));
        match self
            .outcomes
            .lock()
            .expect("lock engine outcomes")
            .pop_front()
            .expect("scripted engine outcome")
        {
            EngineOutcome::Success => Ok(ProviderAttemptBody {
                body_text: "{\"data\":[{\"ok\":true}]}".to_string(),
                requested_image_count: 1,
            }),
            EngineOutcome::Error(error) => Err(error),
        }
    }
}

#[async_trait]
impl ImageEngine for ScriptedEngine {
    async fn generate(
        &self,
        _model: &str,
        api_key: &str,
        _endpoint_url: &str,
        _prompt: &str,
        _options: &GptImageRequestOptions,
    ) -> Result<ProviderAttemptBody, EngineCallError> {
        self.run_attempt(api_key)
    }

    async fn edit(
        &self,
        _model: &str,
        api_key: &str,
        _endpoint_url: &str,
        _prompt: &str,
        source_images: &[PreparedEditImage],
        _options: &GptImageRequestOptions,
    ) -> Result<ProviderAttemptBody, EngineCallError> {
        assert_eq!(source_images.len(), 1);
        assert!(!source_images[0].bytes().is_empty());
        self.run_attempt(api_key)
    }
}

struct BlockingEngine {
    db: Database,
    calls: AtomicUsize,
    entered: Notify,
    dropped: Arc<AtomicBool>,
}

impl BlockingEngine {
    fn new(db: &Database) -> Arc<Self> {
        Arc::new(Self {
            db: db.clone(),
            calls: AtomicUsize::new(0),
            entered: Notify::new(),
            dropped: Arc::new(AtomicBool::new(false)),
        })
    }
}

struct ProviderDropGuard(Arc<AtomicBool>);

impl Drop for ProviderDropGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[async_trait]
impl ImageEngine for BlockingEngine {
    async fn generate(
        &self,
        _model: &str,
        _api_key: &str,
        _endpoint_url: &str,
        _prompt: &str,
        _options: &GptImageRequestOptions,
    ) -> Result<ProviderAttemptBody, EngineCallError> {
        let guard = self
            .db
            .conn
            .try_lock()
            .expect("blocking provider must observe unlocked database");
        drop(guard);
        self.calls.fetch_add(1, Ordering::SeqCst);
        let _drop_guard = ProviderDropGuard(self.dropped.clone());
        self.entered.notify_one();
        std::future::pending().await
    }

    async fn edit(
        &self,
        _model: &str,
        _api_key: &str,
        _endpoint_url: &str,
        _prompt: &str,
        _source_images: &[PreparedEditImage],
        _options: &GptImageRequestOptions,
    ) -> Result<ProviderAttemptBody, EngineCallError> {
        panic!("generate fixture must not call edit")
    }
}

struct ObservedDecoder {
    db: Database,
    calls: AtomicUsize,
}

impl ObservedDecoder {
    fn new(db: &Database) -> Arc<Self> {
        Arc::new(Self {
            db: db.clone(),
            calls: AtomicUsize::new(0),
        })
    }
}

#[async_trait]
impl ImageResponseDecoder for ObservedDecoder {
    async fn decode_and_download(
        &self,
        _response: &ProviderAttemptResponse,
        cancellation: &CancellationProbe,
    ) -> Result<Vec<Vec<u8>>, GenerationExecutionError> {
        let guard = self
            .db
            .conn
            .try_lock()
            .expect("decoder await seam must not hold database mutex");
        drop(guard);
        self.calls.fetch_add(1, Ordering::SeqCst);
        cancellation.checkpoint("test_decoder")?;
        Ok(vec![png_bytes()])
    }
}

struct BlockingDecoder {
    db: Database,
    entered: Notify,
    cleanup_finished: AtomicBool,
}

impl BlockingDecoder {
    fn new(db: &Database) -> Arc<Self> {
        Arc::new(Self {
            db: db.clone(),
            entered: Notify::new(),
            cleanup_finished: AtomicBool::new(false),
        })
    }
}

#[async_trait]
impl ImageResponseDecoder for BlockingDecoder {
    async fn decode_and_download(
        &self,
        _response: &ProviderAttemptResponse,
        cancellation: &CancellationProbe,
    ) -> Result<Vec<Vec<u8>>, GenerationExecutionError> {
        let guard = self
            .db
            .conn
            .try_lock()
            .expect("blocking decoder must observe unlocked database");
        drop(guard);
        self.entered.notify_one();
        while !cancellation.is_cancelled() {
            tokio::task::yield_now().await;
        }
        tokio::task::yield_now().await;
        self.cleanup_finished.store(true, Ordering::SeqCst);
        cancellation.checkpoint("test_decoder_cleanup")?;
        unreachable!("cancelled decoder checkpoint returns an error")
    }
}

struct ObservedFileStore {
    db: Database,
    inner: LocalGenerationFileStore,
}

#[async_trait]
impl GenerationFileStore for ObservedFileStore {
    async fn stage_images(
        &self,
        snapshot: &GenerationExecutionSnapshot,
        images: Vec<Vec<u8>>,
        cancellation: &CancellationProbe,
    ) -> Result<StagedGenerationFiles, GenerationExecutionError> {
        let guard = self
            .db
            .conn
            .try_lock()
            .expect("file-store await seam must not hold database mutex");
        drop(guard);
        self.inner
            .stage_images(snapshot, images, cancellation)
            .await
    }
}

struct ImmediateSleeper {
    db: Database,
    calls: AtomicUsize,
    replace_secret: bool,
    cancel_job_id: Option<String>,
    delete_source: Option<PathBuf>,
}

impl ImmediateSleeper {
    fn plain(db: &Database) -> Arc<Self> {
        Arc::new(Self {
            db: db.clone(),
            calls: AtomicUsize::new(0),
            replace_secret: false,
            cancel_job_id: None,
            delete_source: None,
        })
    }

    fn replace_secret(db: &Database) -> Arc<Self> {
        Arc::new(Self {
            db: db.clone(),
            calls: AtomicUsize::new(0),
            replace_secret: true,
            cancel_job_id: None,
            delete_source: None,
        })
    }

    fn cancel_durably(db: &Database, job_id: String) -> Arc<Self> {
        Arc::new(Self {
            db: db.clone(),
            calls: AtomicUsize::new(0),
            replace_secret: false,
            cancel_job_id: Some(job_id),
            delete_source: None,
        })
    }

    fn delete_source(db: &Database, source: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            db: db.clone(),
            calls: AtomicUsize::new(0),
            replace_secret: false,
            cancel_job_id: None,
            delete_source: Some(source),
        })
    }
}

#[async_trait]
impl GenerationExecutionSleeper for ImmediateSleeper {
    async fn sleep(&self, _delay: Duration) {
        let conn = self
            .db
            .conn
            .try_lock()
            .expect("sleeper await seam must not hold database mutex");
        let (stage, auto_attempt): (String, i32) = conn
            .query_row(
                "SELECT stage, auto_attempt FROM generation_jobs WHERE status = 'running'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read retry backoff state");
        assert_eq!(stage, "retry_backoff");
        assert_eq!(auto_attempt, 0, "ordinal is reserved only after sleep");
        drop(conn);
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.replace_secret {
            save_profile(&self.db, SECRET_TWO);
        }
        if let Some(job_id) = &self.cancel_job_id {
            let conn = self.db.conn.lock().expect("lock sleeper cancellation");
            request_cancel(&conn, job_id).expect("persist cross-process cancellation");
        }
        if let Some(path) = &self.delete_source {
            std::fs::remove_file(path).expect("delete edit source during retry backoff");
        }
    }
}

struct ObservedEvents {
    db: Database,
    events: Mutex<Vec<GenerationJobEvent>>,
    fail: bool,
}

impl GenerationExecutionEventSink for ObservedEvents {
    fn emit(&self, event: GenerationJobEvent) -> Result<(), ()> {
        let conn = self
            .db
            .conn
            .try_lock()
            .expect("event callback must run after database unlock");
        let committed = get_job(&conn, &event.job_id).expect("event job is committed");
        assert_eq!(committed.status, event.status);
        assert_eq!(committed.stage, event.stage);
        assert_eq!(committed.auto_attempt, event.auto_attempt);
        drop(conn);
        self.events
            .lock()
            .expect("lock observed events")
            .push(event);
        if self.fail {
            Err(())
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
struct ObservedDiagnostics {
    values: Mutex<Vec<GenerationExecutionDiagnostic>>,
}

impl GenerationExecutionDiagnosticSink for ObservedDiagnostics {
    fn record(&self, diagnostic: GenerationExecutionDiagnostic) {
        self.values
            .lock()
            .expect("lock observed diagnostics")
            .push(diagnostic);
    }
}

fn png_bytes() -> Vec<u8> {
    let image = image::DynamicImage::new_rgba8(2, 2);
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("encode fixture PNG");
    bytes.into_inner()
}

#[tokio::test]
async fn preparing_completes_once_and_emits_only_committed_unlocked_events() {
    let fixture = Fixture::new(2);
    let engine = ScriptedEngine::new(&fixture.db, [EngineOutcome::Success]);
    let decoder = ObservedDecoder::new(&fixture.db);
    let sleeper = ImmediateSleeper::plain(&fixture.db);
    let (adapter, events, diagnostics) =
        fixture.adapter(engine.clone(), decoder.clone(), sleeper, true);
    let (_cancel, cancellation) = watch::channel(false);

    let outcome = adapter
        .execute(&fixture.authority, &fixture.job_id, cancellation)
        .await
        .expect("execute preparing job");

    assert_eq!(outcome, WorkerExecutionOutcome::DurablyFinished);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    assert_eq!(decoder.calls.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.job().status, GenerationJobStatus::Completed);
    assert_eq!(fixture.recovery_count(), 0);
    assert_eq!(fixture.image_count(), 1);
    assert_eq!(
        engine.secrets.lock().expect("lock secrets").as_slice(),
        [SECRET_ONE]
    );
    assert_eq!(
        events
            .events
            .lock()
            .expect("lock events")
            .iter()
            .map(|event| event.stage)
            .collect::<Vec<_>>(),
        vec![
            GenerationJobStage::ProviderRequest,
            GenerationJobStage::ResponseReady,
            GenerationJobStage::LocalProcessing,
            GenerationJobStage::Terminal,
        ]
    );
    assert!(diagnostics
        .values
        .lock()
        .expect("lock diagnostics")
        .iter()
        .all(|diagnostic| diagnostic.kind == GenerationExecutionDiagnosticKind::EventSink));
}

#[tokio::test]
async fn execution_time_is_sampled_only_inside_repository_writes() {
    let fixture = Fixture::new(2);
    let engine = ScriptedEngine::new(&fixture.db, [EngineOutcome::Success]);
    let decoder = ObservedDecoder::new(&fixture.db);
    let sleeper = ImmediateSleeper::plain(&fixture.db);
    let clock = DatabaseLockAssertingClock::new(&fixture.db);
    let (adapter, _, _) = fixture.adapter_with_clock(
        engine,
        decoder,
        sleeper,
        Arc::clone(&clock) as Arc<dyn GenerationExecutionClock>,
        false,
    );
    let (_cancel, cancellation) = watch::channel(false);

    let outcome = adapter
        .execute(&fixture.authority, &fixture.job_id, cancellation)
        .await
        .expect("execute with transaction-time clock");

    assert_eq!(outcome, WorkerExecutionOutcome::DurablyFinished);
    assert_eq!(fixture.job().status, GenerationJobStatus::Completed);
    assert_eq!(clock.calls.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn corrupt_provider_snapshot_fails_before_exact_secret_lookup() {
    let fixture = Fixture::new(2);
    {
        let conn = fixture.db.conn.lock().expect("lock provider corruption");
        conn.execute(
            "UPDATE generation_jobs SET provider_kind = 'gemini' WHERE id = ?1",
            [&fixture.job_id],
        )
        .expect("corrupt provider kind");
    }
    settings::save_model_provider_profiles_state(
        &fixture.db,
        "gpt-image-2",
        ModelProviderProfilesState {
            active_provider_id: String::new(),
            profiles: Vec::new(),
        },
    )
    .expect("remove profile secret");
    let engine = ScriptedEngine::new(&fixture.db, []);
    let decoder = ObservedDecoder::new(&fixture.db);
    let sleeper = ImmediateSleeper::plain(&fixture.db);
    let clock = DatabaseLockAssertingClock::new(&fixture.db);
    let (adapter, _, _) = fixture.adapter_with_clock(
        engine.clone(),
        decoder,
        sleeper,
        Arc::clone(&clock) as Arc<dyn GenerationExecutionClock>,
        false,
    );
    let (_cancel, cancellation) = watch::channel(false);

    let outcome = adapter
        .execute(&fixture.authority, &fixture.job_id, cancellation)
        .await
        .expect("execute corrupt snapshot");

    assert_eq!(outcome, WorkerExecutionOutcome::DurablyFinished);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
    let job = fixture.job();
    assert_eq!(job.status, GenerationJobStatus::Failed);
    assert_eq!(
        job.error_code.as_deref(),
        Some("provider_configuration_invalid"),
        "pure snapshot validation must run before the missing-profile secret lookup"
    );
    assert_eq!(clock.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn retry_waits_before_reserving_ordinal_and_reloads_exact_profile_secret() {
    let fixture = Fixture::new(2);
    let engine = ScriptedEngine::new(
        &fixture.db,
        [
            EngineOutcome::Error(EngineCallError::network_before_response()),
            EngineOutcome::Success,
        ],
    );
    let decoder = ObservedDecoder::new(&fixture.db);
    let sleeper = ImmediateSleeper::replace_secret(&fixture.db);
    let clock = DatabaseLockAssertingClock::new(&fixture.db);
    let (adapter, events, _) = fixture.adapter_with_clock(
        engine.clone(),
        decoder,
        sleeper.clone(),
        Arc::clone(&clock) as Arc<dyn GenerationExecutionClock>,
        false,
    );
    let (_cancel, cancellation) = watch::channel(false);

    let outcome = adapter
        .execute(&fixture.authority, &fixture.job_id, cancellation)
        .await
        .expect("execute retrying job");

    assert_eq!(outcome, WorkerExecutionOutcome::DurablyFinished);
    assert_eq!(sleeper.calls.load(Ordering::SeqCst), 1);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        engine
            .secrets
            .lock()
            .expect("lock retry secrets")
            .as_slice(),
        [SECRET_ONE, SECRET_TWO]
    );
    assert_eq!(
        engine.attempts.lock().expect("lock attempts").as_slice(),
        [
            (GenerationJobStage::ProviderRequest, 0),
            (GenerationJobStage::ProviderRequest, 1),
        ]
    );
    assert_eq!(
        events
            .events
            .lock()
            .expect("lock retry events")
            .iter()
            .map(|event| (event.stage, event.auto_attempt))
            .collect::<Vec<_>>(),
        vec![
            (GenerationJobStage::ProviderRequest, 0),
            (GenerationJobStage::RetryBackoff, 0),
            (GenerationJobStage::ProviderRequest, 1),
            (GenerationJobStage::ResponseReady, 1),
            (GenerationJobStage::LocalProcessing, 1),
            (GenerationJobStage::Terminal, 1),
        ]
    );
    assert_eq!(clock.calls.load(Ordering::SeqCst), 6);
}

#[tokio::test]
async fn edit_retry_revalidates_source_after_reservation_before_second_provider_call() {
    let fixture = Fixture::new_edit(2);
    let source = fixture
        .source_path
        .clone()
        .expect("edit fixture source path");
    let engine = ScriptedEngine::new(
        &fixture.db,
        [EngineOutcome::Error(
            EngineCallError::network_before_response(),
        )],
    );
    let decoder = ObservedDecoder::new(&fixture.db);
    let sleeper = ImmediateSleeper::delete_source(&fixture.db, source);
    let (adapter, events, _) = fixture.adapter(engine.clone(), decoder, sleeper, false);
    let (_cancel, cancellation) = watch::channel(false);

    let outcome = adapter
        .execute(&fixture.authority, &fixture.job_id, cancellation)
        .await
        .expect("execute edit retry revalidation");

    assert_eq!(outcome, WorkerExecutionOutcome::DurablyFinished);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    let job = fixture.job();
    assert_eq!(job.status, GenerationJobStatus::Failed);
    assert_eq!(job.auto_attempt, 1, "retry ordinal was durably reserved");
    assert_eq!(job.error_code.as_deref(), Some("source_image_invalid"));
    assert_eq!(
        events
            .events
            .lock()
            .expect("lock edit retry events")
            .iter()
            .map(|event| (event.stage, event.auto_attempt))
            .collect::<Vec<_>>(),
        vec![
            (GenerationJobStage::ProviderRequest, 0),
            (GenerationJobStage::RetryBackoff, 0),
            (GenerationJobStage::ProviderRequest, 1),
            (GenerationJobStage::Terminal, 1),
        ]
    );
}

#[tokio::test]
async fn retry_backoff_rereads_cross_process_cancel_before_reserving_or_replaying() {
    let fixture = Fixture::new(2);
    let engine = ScriptedEngine::new(
        &fixture.db,
        [EngineOutcome::Error(
            EngineCallError::network_before_response(),
        )],
    );
    let decoder = ObservedDecoder::new(&fixture.db);
    let sleeper = ImmediateSleeper::cancel_durably(&fixture.db, fixture.job_id.clone());
    let (adapter, _, _) = fixture.adapter(engine.clone(), decoder, sleeper, false);
    let (_cancel, cancellation) = watch::channel(false);

    let outcome = adapter
        .execute(&fixture.authority, &fixture.job_id, cancellation)
        .await
        .expect("execute cancelled backoff job");

    assert_eq!(outcome, WorkerExecutionOutcome::DurablyFinished);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    let job = fixture.job();
    assert_eq!(job.status, GenerationJobStatus::Cancelled);
    assert_eq!(
        job.auto_attempt, 0,
        "cancelled wait must not reserve an ordinal"
    );
    assert_eq!(fixture.recovery_count(), 0);
}

#[tokio::test]
async fn response_ready_and_local_processing_recovery_never_call_provider() {
    for enter_local_before_execute in [false, true] {
        let fixture = Fixture::new(2);
        fixture.make_response_ready().await;
        if enter_local_before_execute {
            let conn = fixture.db.conn.lock().expect("lock local transition");
            transition_running_job_stage_with_event(
                &conn,
                &fixture.job_id,
                GenerationJobStage::ResponseReady,
                WorkerStageTransition::EnterLocalProcessing,
                &fixture.authority,
                Utc::now().timestamp_millis(),
            )
            .expect("enter local processing before recovery");
        }
        let engine = ScriptedEngine::new(&fixture.db, []);
        let decoder = ObservedDecoder::new(&fixture.db);
        let sleeper = ImmediateSleeper::plain(&fixture.db);
        let (adapter, _, _) = fixture.adapter(engine.clone(), decoder.clone(), sleeper, false);
        let (_cancel, cancellation) = watch::channel(false);

        let outcome = adapter
            .execute(&fixture.authority, &fixture.job_id, cancellation)
            .await
            .expect("execute local recovery");

        assert_eq!(outcome, WorkerExecutionOutcome::DurablyFinished);
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
        assert_eq!(decoder.calls.load(Ordering::SeqCst), 1);
        assert_eq!(fixture.job().status, GenerationJobStatus::Completed);
    }
}

#[tokio::test]
async fn persisted_provider_and_backoff_stages_require_reconciliation_without_replay() {
    for persisted_stage in [
        GenerationJobStage::ProviderRequest,
        GenerationJobStage::RetryBackoff,
    ] {
        let fixture = Fixture::new(2);
        let expected_response_file = fixture
            .artifact_store()
            .response_path(&GenerationExecutionContext {
                generation_id: fixture.generation_id.clone(),
                job_id: fixture.job_id.clone(),
                conversation_id: String::new(),
                provider_kind: String::new(),
                model: String::new(),
                endpoint_url: String::new(),
                provider_profile_id: String::new(),
            })
            .expect("derive unsupported-stage response path");
        {
            let conn = fixture.db.conn.lock().expect("lock unsupported stage");
            transition_running_job_stage_with_event(
                &conn,
                &fixture.job_id,
                GenerationJobStage::Preparing,
                WorkerStageTransition::BeginProviderRequest {
                    expected_response_file,
                },
                &fixture.authority,
                Utc::now().timestamp_millis(),
            )
            .expect("enter persisted provider request");
            if persisted_stage == GenerationJobStage::RetryBackoff {
                transition_running_job_stage_with_event(
                    &conn,
                    &fixture.job_id,
                    GenerationJobStage::ProviderRequest,
                    WorkerStageTransition::EnterRetryBackoff,
                    &fixture.authority,
                    Utc::now().timestamp_millis(),
                )
                .expect("enter persisted retry backoff");
            }
        }
        let before = fixture.job();
        let engine = ScriptedEngine::new(&fixture.db, [EngineOutcome::Success]);
        let decoder = ObservedDecoder::new(&fixture.db);
        let sleeper = ImmediateSleeper::plain(&fixture.db);
        let (adapter, events, _) = fixture.adapter(engine.clone(), decoder, sleeper, false);
        let (_cancel, cancellation) = watch::channel(false);

        let outcome = adapter
            .execute(&fixture.authority, &fixture.job_id, cancellation)
            .await
            .expect("reject unsupported persisted stage");

        assert_eq!(outcome, WorkerExecutionOutcome::NeedsReconciliation);
        assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
        let after = fixture.job();
        assert_eq!(after.stage, persisted_stage);
        assert_eq!(after.auto_attempt, before.auto_attempt);
        assert!(events
            .events
            .lock()
            .expect("lock unsupported events")
            .is_empty());
    }
}

#[tokio::test]
async fn response_descriptor_tamper_fails_before_entering_local_processing() {
    let fixture = Fixture::new(2);
    fixture.make_response_ready().await;
    {
        let conn = fixture.db.conn.lock().expect("lock descriptor tamper");
        conn.execute(
            "UPDATE generation_recoveries SET response_sha256 = ?1 WHERE generation_id = ?2",
            params!["0".repeat(64), fixture.generation_id],
        )
        .expect("tamper persisted descriptor");
    }
    let engine = ScriptedEngine::new(&fixture.db, []);
    let decoder = ObservedDecoder::new(&fixture.db);
    let sleeper = ImmediateSleeper::plain(&fixture.db);
    let (adapter, _, _) = fixture.adapter(engine.clone(), decoder.clone(), sleeper, false);
    let (_cancel, cancellation) = watch::channel(false);

    let outcome = adapter
        .execute(&fixture.authority, &fixture.job_id, cancellation)
        .await
        .expect("execute tampered response");

    assert_eq!(outcome, WorkerExecutionOutcome::DurablyFinished);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
    assert_eq!(decoder.calls.load(Ordering::SeqCst), 0);
    let job = fixture.job();
    assert_eq!(job.status, GenerationJobStatus::Failed);
    assert_eq!(job.error_code.as_deref(), Some("recovery_failed"));
    assert_eq!(
        fixture.recovery_count(),
        0,
        "bad ResponseReady evidence is not preserved as LocalProcessing"
    );
}

#[tokio::test]
async fn response_ready_durable_cancel_acknowledges_without_entering_local_processing() {
    let fixture = Fixture::new(2);
    fixture.make_response_ready().await;
    fixture.request_cancel();
    let engine = ScriptedEngine::new(&fixture.db, []);
    let decoder = ObservedDecoder::new(&fixture.db);
    let sleeper = ImmediateSleeper::plain(&fixture.db);
    let clock = DatabaseLockAssertingClock::new(&fixture.db);
    let (adapter, events, _) = fixture.adapter_with_clock(
        engine.clone(),
        decoder.clone(),
        sleeper,
        Arc::clone(&clock) as Arc<dyn GenerationExecutionClock>,
        false,
    );
    let (_cancel, cancellation) = watch::channel(false);

    let outcome = adapter
        .execute(&fixture.authority, &fixture.job_id, cancellation)
        .await
        .expect("acknowledge response-ready cancellation");

    assert_eq!(outcome, WorkerExecutionOutcome::DurablyFinished);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
    assert_eq!(decoder.calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.job().status, GenerationJobStatus::Cancelled);
    assert_eq!(
        events
            .events
            .lock()
            .expect("lock cancellation events")
            .iter()
            .map(|event| event.stage)
            .collect::<Vec<_>>(),
        vec![GenerationJobStage::Terminal],
        "ResponseReady cancellation must not publish LocalProcessing"
    );
    assert_eq!(clock.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn response_ready_cancel_after_loaded_snapshot_before_transition_is_fenced_and_acknowledged()
{
    let fixture = Fixture::new(2);
    fixture.make_response_ready().await;
    let engine = ScriptedEngine::new(&fixture.db, []);
    let decoder = ObservedDecoder::new(&fixture.db);
    let sleeper = ImmediateSleeper::plain(&fixture.db);
    let (adapter, events, _) = fixture.adapter(engine.clone(), decoder.clone(), sleeper, false);
    let loaded = adapter
        .load_execution_at_stage(&fixture.job_id, GenerationJobStage::ResponseReady)
        .await
        .expect("load response-ready snapshot before cancellation");
    assert!(loaded.job.cancel_requested_at.is_none());
    fixture.request_cancel();
    let (_cancel, cancellation) = watch::channel(false);

    let outcome = adapter
        .execute_response_ready(&fixture.authority, loaded, cancellation)
        .await
        .expect("fence response-ready cancellation race");

    assert_eq!(outcome, WorkerExecutionOutcome::DurablyFinished);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
    assert_eq!(decoder.calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.job().status, GenerationJobStatus::Cancelled);
    assert_eq!(
        events
            .events
            .lock()
            .expect("lock raced cancellation events")
            .iter()
            .map(|event| event.stage)
            .collect::<Vec<_>>(),
        vec![GenerationJobStage::Terminal],
        "the fenced transition must not publish LocalProcessing"
    );
}

#[tokio::test]
async fn watch_only_provider_cancel_drops_http_but_does_not_ack_without_durable_cancel() {
    let fixture = Fixture::new(2);
    let engine = BlockingEngine::new(&fixture.db);
    let decoder = ObservedDecoder::new(&fixture.db);
    let sleeper = ImmediateSleeper::plain(&fixture.db);
    let (adapter, _, _) = fixture.adapter(engine.clone(), decoder, sleeper, false);
    let adapter = Arc::new(adapter);
    let (cancel, cancellation) = watch::channel(false);
    let authority = fixture.authority.clone();
    let job_id = fixture.job_id.clone();
    let running = {
        let adapter = adapter.clone();
        tokio::spawn(async move { adapter.execute(&authority, &job_id, cancellation).await })
    };
    engine.entered.notified().await;
    cancel.send_replace(true);

    let outcome = running
        .await
        .expect("join watch-only cancellation")
        .expect("execute watch-only cancellation");

    assert_eq!(outcome, WorkerExecutionOutcome::NeedsReconciliation);
    assert!(engine.dropped.load(Ordering::SeqCst));
    let job = fixture.job();
    assert_eq!(job.status, GenerationJobStatus::Running);
    assert_eq!(job.stage, GenerationJobStage::ProviderRequest);
    assert!(job.cancel_requested_at.is_none());
    assert_eq!(fixture.recovery_count(), 1);
}

#[tokio::test]
async fn durable_provider_cancel_drops_http_then_acknowledges() {
    let fixture = Fixture::new(2);
    let engine = BlockingEngine::new(&fixture.db);
    let decoder = ObservedDecoder::new(&fixture.db);
    let sleeper = ImmediateSleeper::plain(&fixture.db);
    let (adapter, _, _) = fixture.adapter(engine.clone(), decoder, sleeper, false);
    let adapter = Arc::new(adapter);
    let (cancel, cancellation) = watch::channel(false);
    let authority = fixture.authority.clone();
    let job_id = fixture.job_id.clone();
    let running = {
        let adapter = adapter.clone();
        tokio::spawn(async move { adapter.execute(&authority, &job_id, cancellation).await })
    };
    engine.entered.notified().await;
    fixture.request_cancel();
    cancel.send_replace(true);

    let outcome = running
        .await
        .expect("join durable cancellation")
        .expect("execute durable cancellation");

    assert_eq!(outcome, WorkerExecutionOutcome::DurablyFinished);
    assert!(engine.dropped.load(Ordering::SeqCst));
    assert_eq!(fixture.job().status, GenerationJobStatus::Cancelled);
    assert_eq!(fixture.recovery_count(), 0);
}

#[tokio::test]
async fn local_cancel_sets_probe_waits_for_cleanup_then_acknowledges_without_provider() {
    let fixture = Fixture::new(2);
    fixture.make_response_ready().await;
    let engine = ScriptedEngine::new(&fixture.db, []);
    let decoder = BlockingDecoder::new(&fixture.db);
    let sleeper = ImmediateSleeper::plain(&fixture.db);
    let (adapter, _, _) = fixture.adapter(engine.clone(), decoder.clone(), sleeper, false);
    let adapter = Arc::new(adapter);
    let (cancel, cancellation) = watch::channel(false);
    let authority = fixture.authority.clone();
    let job_id = fixture.job_id.clone();
    let running = {
        let adapter = adapter.clone();
        tokio::spawn(async move { adapter.execute(&authority, &job_id, cancellation).await })
    };
    decoder.entered.notified().await;
    fixture.request_cancel();
    cancel.send_replace(true);

    let outcome = running
        .await
        .expect("join local cancellation")
        .expect("execute local cancellation");

    assert_eq!(outcome, WorkerExecutionOutcome::DurablyFinished);
    assert!(decoder.cleanup_finished.load(Ordering::SeqCst));
    assert_eq!(engine.calls.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.job().status, GenerationJobStatus::Cancelled);
    assert_eq!(fixture.recovery_count(), 0);
}

#[tokio::test]
async fn installed_response_with_promotion_fault_requires_reconciliation_without_replay() {
    let fixture = Fixture::new(2);
    {
        let conn = fixture.db.conn.lock().expect("lock promotion trigger");
        conn.execute_batch(
            "CREATE TRIGGER fail_execution_response_promotion
             BEFORE UPDATE OF request_state ON generation_recoveries
             WHEN NEW.request_state = 'response_ready'
             BEGIN
                 SELECT RAISE(ABORT, 'injected response promotion failure');
             END;",
        )
        .expect("install promotion failure trigger");
    }
    let engine = ScriptedEngine::new(&fixture.db, [EngineOutcome::Success]);
    let decoder = ObservedDecoder::new(&fixture.db);
    let sleeper = ImmediateSleeper::plain(&fixture.db);
    let (adapter, _, _) = fixture.adapter(engine.clone(), decoder.clone(), sleeper, false);
    let (_cancel, cancellation) = watch::channel(false);

    let outcome = adapter
        .execute(&fixture.authority, &fixture.job_id, cancellation)
        .await
        .expect("execute promotion fault");

    assert_eq!(outcome, WorkerExecutionOutcome::NeedsReconciliation);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 1);
    assert_eq!(decoder.calls.load(Ordering::SeqCst), 0);
    let job = fixture.job();
    assert_eq!(job.status, GenerationJobStatus::Running);
    assert_eq!(job.stage, GenerationJobStage::ProviderRequest);
    let response_path = fixture
        .artifact_store()
        .response_path(&GenerationExecutionContext {
            generation_id: fixture.generation_id.clone(),
            job_id: fixture.job_id.clone(),
            conversation_id: String::new(),
            provider_kind: String::new(),
            model: String::new(),
            endpoint_url: String::new(),
            provider_profile_id: String::new(),
        })
        .expect("derive installed response path");
    assert!(
        response_path.is_file(),
        "installed artifact must be retained"
    );
    let conn = fixture.db.conn.lock().expect("lock requesting recovery");
    let state: String = conn
        .query_row(
            "SELECT request_state FROM generation_recoveries WHERE generation_id = ?1",
            [&fixture.generation_id],
            |row| row.get(0),
        )
        .expect("read requesting recovery");
    assert_eq!(state, "requesting");
}
