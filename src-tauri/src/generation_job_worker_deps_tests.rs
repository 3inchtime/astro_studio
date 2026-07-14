use super::*;
use crate::generation_job_execution::{GenerationExecutionClock, GenerationExecutionEventSink};
use crate::generation_job_worker::{
    ClaimedGenerationJob, GenerationJobWorkerDeps, StartupReconciliation, WorkerCoreError,
    WorkerCoreErrorKind, WorkerDiagnostic, WorkerDiagnosticKind, WorkerExecutionJob,
    WorkerExecutionOutcome,
};
use crate::generation_jobs::{
    enqueue_job, get_job, request_cancel, GenerationJobOptions, PreparedGenerationJob,
};
use crate::generation_worker_lease::WorkerLeaseAcquireOutcome;
use crate::models::{GenerationJobStage, GenerationJobStatus};
use async_trait::async_trait;
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::watch;

fn assert_send_sync_static<T: Send + Sync + 'static>() {}

#[derive(Default)]
struct FixedClock {
    now_ms: AtomicI64,
}

impl FixedClock {
    fn new(now_ms: i64) -> Self {
        Self {
            now_ms: AtomicI64::new(now_ms),
        }
    }

    fn advance(&self, millis: i64) {
        self.now_ms.fetch_add(millis, Ordering::SeqCst);
    }
}

impl GenerationExecutionClock for FixedClock {
    fn now_ms(&self) -> i64 {
        self.now_ms.load(Ordering::SeqCst)
    }

    fn now_utc(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(self.now_ms()).expect("fixed clock is representable")
    }
}

#[derive(Default)]
struct FakeExecutor {
    calls: Mutex<Vec<String>>,
    outcome: Mutex<Option<Result<WorkerExecutionOutcome, WorkerCoreError>>>,
}

impl FakeExecutor {
    fn returning(outcome: Result<WorkerExecutionOutcome, WorkerCoreError>) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            outcome: Mutex::new(Some(outcome)),
        })
    }
}

#[async_trait]
impl GenerationJobExecutor for FakeExecutor {
    async fn execute(
        &self,
        _authority: &crate::generation_worker_lease::WorkerTransitionAuthority,
        job_id: &str,
        cancellation: watch::Receiver<bool>,
    ) -> Result<WorkerExecutionOutcome, WorkerCoreError> {
        self.calls
            .lock()
            .expect("lock executor calls")
            .push(format!("{job_id}:{}", *cancellation.borrow()));
        self.outcome
            .lock()
            .expect("lock executor outcome")
            .take()
            .expect("one scripted execution outcome")
    }
}

struct FakeReconciler {
    calls: Mutex<Vec<i64>>,
    result: Mutex<Option<Result<StartupReconciliation, WorkerCoreError>>>,
}

impl FakeReconciler {
    fn returning(result: Result<StartupReconciliation, WorkerCoreError>) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            result: Mutex::new(Some(result)),
        })
    }
}

#[async_trait]
impl GenerationJobStartupReconciler for FakeReconciler {
    async fn reconcile(
        &self,
        _authority: &crate::generation_worker_lease::WorkerTransitionAuthority,
        now_ms: i64,
    ) -> Result<StartupReconciliation, WorkerCoreError> {
        self.calls
            .lock()
            .expect("lock reconcile calls")
            .push(now_ms);
        self.result
            .lock()
            .expect("lock reconcile result")
            .take()
            .expect("one scripted reconciliation result")
    }
}

struct ObservedEvents {
    db: crate::db::Database,
    fail: AtomicBool,
    committed_reads: AtomicUsize,
    values: Mutex<Vec<crate::models::GenerationJobEvent>>,
}

impl ObservedEvents {
    fn new(db: crate::db::Database) -> Self {
        Self {
            db,
            fail: AtomicBool::new(false),
            committed_reads: AtomicUsize::new(0),
            values: Mutex::new(Vec::new()),
        }
    }
}

impl GenerationExecutionEventSink for ObservedEvents {
    fn emit(&self, event: crate::models::GenerationJobEvent) -> Result<(), ()> {
        let conn = self
            .db
            .conn
            .try_lock()
            .expect("committed event must be emitted after releasing the database lock");
        let persisted = get_job(&conn, &event.job_id).expect("event job must already be committed");
        assert_eq!(persisted.generation_id, event.generation_id);
        assert_eq!(persisted.status, event.status);
        assert_eq!(persisted.stage, event.stage);
        self.committed_reads.fetch_add(1, Ordering::SeqCst);
        drop(conn);
        self.values.lock().expect("lock events").push(event);
        if self.fail.load(Ordering::SeqCst) {
            Err(())
        } else {
            Ok(())
        }
    }
}

#[derive(Default)]
struct ObservedDiagnostics {
    values: Mutex<Vec<WorkerDiagnostic>>,
}

impl GenerationJobWorkerDiagnosticSink for ObservedDiagnostics {
    fn record(&self, diagnostic: WorkerDiagnostic) {
        self.values
            .lock()
            .expect("lock worker diagnostics")
            .push(diagnostic);
    }
}

struct Fixture {
    root: std::path::PathBuf,
    db: crate::db::Database,
    job_id: String,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "astro-studio-worker-deps-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).expect("create worker deps fixture");
        let db = crate::db::Database::open(&root.join("astro_studio.db"))
            .expect("open worker deps database");
        db.run_migrations().expect("migrate worker deps database");
        let now = Utc::now();
        let job_id = uuid::Uuid::new_v4().to_string();
        let prepared = PreparedGenerationJob {
            job_id: job_id.clone(),
            client_request_id: uuid::Uuid::new_v4().to_string(),
            generation_id: uuid::Uuid::new_v4().to_string(),
            requested_conversation_id: None,
            requested_project_id: Some("default".to_string()),
            prompt: "draw a repository worker".to_string(),
            model: "gpt-image-2".to_string(),
            request_kind: "generate".to_string(),
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
            source_image_paths: Vec::new(),
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
            source_kind: "generate".to_string(),
            source_ref: json!({ "id": job_id }),
            provider_kind: "openai".to_string(),
            provider_profile_id: "profile-1".to_string(),
            endpoint_snapshot: "https://provider.example.test/v1/images/generations".to_string(),
            status: GenerationJobStatus::Queued,
            chain_attempt: 1,
            auto_attempt: 0,
            max_auto_attempts: 2,
            queued_at: (now - chrono::Duration::seconds(5))
                .to_rfc3339_opts(SecondsFormat::Secs, true),
            finished_at: None,
            error_code: None,
            error_message: None,
            retryable: false,
        };
        {
            let mut conn = db.conn.lock().expect("lock enqueue database");
            enqueue_job(&mut conn, &prepared).expect("enqueue worker deps job");
        }
        Self { root, db, job_id }
    }

    fn deps(
        &self,
        clock: Arc<FixedClock>,
        executor: Arc<FakeExecutor>,
        reconciler: Arc<FakeReconciler>,
        events: Arc<ObservedEvents>,
        diagnostics: Arc<ObservedDiagnostics>,
    ) -> RepositoryGenerationJobWorkerDeps {
        RepositoryGenerationJobWorkerDeps::new(
            self.db.clone(),
            executor,
            reconciler,
            events,
            diagnostics,
            clock,
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

#[tokio::test]
async fn deps_are_send_sync_static_and_delegate_clock_reconcile_and_execute() {
    assert_send_sync_static::<RepositoryGenerationJobWorkerDeps>();
    assert_send_sync_static::<Arc<dyn GenerationJobExecutor>>();
    assert_send_sync_static::<Arc<dyn GenerationJobStartupReconciler>>();
    assert_send_sync_static::<Arc<dyn GenerationJobWorkerDiagnosticSink>>();

    let fixture = Fixture::new();
    let clock = Arc::new(FixedClock::new(Utc::now().timestamp_millis()));
    let executor = FakeExecutor::returning(Ok(WorkerExecutionOutcome::NeedsReconciliation));
    let expected_reconciliation =
        StartupReconciliation::new(Vec::new(), vec![WorkerExecutionJob::new("recover-local")]);
    let reconciler = FakeReconciler::returning(Ok(expected_reconciliation.clone()));
    let events = Arc::new(ObservedEvents::new(fixture.db.clone()));
    let diagnostics = Arc::new(ObservedDiagnostics::default());
    let deps = fixture.deps(
        Arc::clone(&clock),
        Arc::clone(&executor),
        Arc::clone(&reconciler),
        events,
        diagnostics,
    );

    assert_eq!(deps.now_ms(), clock.now_ms());
    let acquired = deps
        .acquire_lease("worker-deps", deps.now_ms(), Duration::from_secs(60))
        .await
        .expect("acquire production deps lease");
    let authority = match acquired {
        WorkerLeaseAcquireOutcome::Acquired { authority, .. } => authority,
        WorkerLeaseAcquireOutcome::Held { .. } => panic!("fixture lease unexpectedly held"),
    };
    assert_eq!(
        deps.reconcile_startup(&authority, deps.now_ms())
            .await
            .expect("delegate startup reconciliation"),
        expected_reconciliation
    );
    let (_, cancellation) = watch::channel(false);
    assert_eq!(
        deps.execute_job(
            &authority,
            WorkerExecutionJob::new("delegated-job"),
            cancellation,
        )
        .await
        .expect("delegate execution"),
        WorkerExecutionOutcome::NeedsReconciliation
    );
    assert_eq!(
        executor
            .calls
            .lock()
            .expect("read executor calls")
            .as_slice(),
        ["delegated-job:false"]
    );
    assert_eq!(
        reconciler
            .calls
            .lock()
            .expect("read reconcile calls")
            .as_slice(),
        [clock.now_ms()]
    );
}

#[tokio::test]
async fn repository_methods_are_fenced_and_claim_event_is_not_emitted_implicitly() {
    let fixture = Fixture::new();
    let clock = Arc::new(FixedClock::new(Utc::now().timestamp_millis()));
    let events = Arc::new(ObservedEvents::new(fixture.db.clone()));
    let diagnostics = Arc::new(ObservedDiagnostics::default());
    let deps = fixture.deps(
        Arc::clone(&clock),
        FakeExecutor::returning(Ok(WorkerExecutionOutcome::DurablyFinished)),
        FakeReconciler::returning(Ok(StartupReconciliation::empty())),
        Arc::clone(&events),
        diagnostics,
    );
    let authority = match deps
        .acquire_lease("repository-worker", deps.now_ms(), Duration::from_secs(60))
        .await
        .expect("acquire repository lease")
    {
        WorkerLeaseAcquireOutcome::Acquired { authority, .. } => authority,
        WorkerLeaseAcquireOutcome::Held { .. } => panic!("repository lease unexpectedly held"),
    };
    let claimed: ClaimedGenerationJob = deps
        .claim_next(&authority, deps.now_ms())
        .await
        .expect("claim through repository deps")
        .expect("queued job is claimable");
    assert_eq!(claimed.job_id(), fixture.job_id);
    assert_eq!(
        claimed.committed_event().stage,
        GenerationJobStage::Preparing
    );
    assert!(events.values.lock().expect("read claim events").is_empty());

    deps.emit_committed_event(claimed.committed_event().clone())
        .expect("emit claimed event explicitly");
    assert_eq!(events.values.lock().expect("read emitted event").len(), 1);
    assert_eq!(events.committed_reads.load(Ordering::SeqCst), 1);
    assert!(!deps
        .reread_cancel_requested(&authority, &fixture.job_id)
        .await
        .expect("reread uncancelled job"));
    clock.advance(1_000);
    deps.heartbeat_job(&authority, &fixture.job_id, deps.now_ms())
        .await
        .expect("heartbeat current running stage");
    {
        let conn = fixture.db.conn.lock().expect("lock cancellation request");
        request_cancel(&conn, &fixture.job_id).expect("request durable cancellation");
    }
    assert!(deps
        .reread_cancel_requested(&authority, &fixture.job_id)
        .await
        .expect("reread durable cancellation"));
    deps.release_lease(&authority)
        .await
        .expect("release repository lease");
}

#[tokio::test]
async fn repository_writes_resample_the_clock_instead_of_trusting_stale_caller_timestamps() {
    let fixture = Fixture::new();
    let initial_now_ms = Utc::now().timestamp_millis();
    let clock = Arc::new(FixedClock::new(initial_now_ms));
    let deps = fixture.deps(
        Arc::clone(&clock),
        FakeExecutor::returning(Ok(WorkerExecutionOutcome::DurablyFinished)),
        FakeReconciler::returning(Ok(StartupReconciliation::empty())),
        Arc::new(ObservedEvents::new(fixture.db.clone())),
        Arc::new(ObservedDiagnostics::default()),
    );
    let ttl = Duration::from_secs(60);
    let authority_a = match deps
        .acquire_lease("clock-worker-a", initial_now_ms, ttl)
        .await
        .expect("acquire worker A lease")
    {
        WorkerLeaseAcquireOutcome::Acquired { authority, .. } => authority,
        WorkerLeaseAcquireOutcome::Held { .. } => panic!("worker A lease unexpectedly held"),
    };

    clock.advance(61_000);
    let stale_claim = deps
        .claim_next(&authority_a, initial_now_ms)
        .await
        .expect_err("claim must observe the expired lease");
    assert_eq!(stale_claim.kind, WorkerCoreErrorKind::LeaseLost);
    let stale_renewal = deps
        .renew_lease(&authority_a, initial_now_ms, ttl)
        .await
        .expect_err("renewal must observe the expired lease");
    assert_eq!(stale_renewal.kind, WorkerCoreErrorKind::LeaseLost);

    let authority_b = match deps
        .acquire_lease("clock-worker-b", initial_now_ms, ttl)
        .await
        .expect("expired lease must be acquirable using the current clock")
    {
        WorkerLeaseAcquireOutcome::Acquired { authority, .. } => authority,
        WorkerLeaseAcquireOutcome::Held { .. } => {
            panic!("stale caller timestamp incorrectly kept the expired lease alive")
        }
    };
    let worker_b_now_ms = clock.now_ms();
    let claimed = deps
        .claim_next(&authority_b, worker_b_now_ms)
        .await
        .expect("worker B claim must be fenced by its live lease")
        .expect("queued fixture job must be claimable");
    assert_eq!(claimed.job_id(), fixture.job_id);

    clock.advance(61_000);
    let stale_heartbeat = deps
        .heartbeat_job(&authority_b, &fixture.job_id, worker_b_now_ms)
        .await
        .expect_err("heartbeat must observe the expired worker B lease");
    assert_eq!(stale_heartbeat.kind, WorkerCoreErrorKind::LeaseLost);
}

#[tokio::test]
async fn lease_loss_is_exact_while_repository_and_event_failures_are_transient_and_sanitized() {
    let fixture = Fixture::new();
    let clock = Arc::new(FixedClock::new(Utc::now().timestamp_millis()));
    let events = Arc::new(ObservedEvents::new(fixture.db.clone()));
    let diagnostics = Arc::new(ObservedDiagnostics::default());
    let deps = fixture.deps(
        Arc::clone(&clock),
        FakeExecutor::returning(Ok(WorkerExecutionOutcome::DurablyFinished)),
        FakeReconciler::returning(Ok(StartupReconciliation::empty())),
        Arc::clone(&events),
        Arc::clone(&diagnostics),
    );
    let authority_a = match deps
        .acquire_lease("worker-a", deps.now_ms(), Duration::from_secs(60))
        .await
        .expect("acquire worker A")
    {
        WorkerLeaseAcquireOutcome::Acquired { authority, .. } => authority,
        WorkerLeaseAcquireOutcome::Held { .. } => panic!("worker A lease unexpectedly held"),
    };
    deps.release_lease(&authority_a)
        .await
        .expect("release worker A");
    let authority_b = match deps
        .acquire_lease("worker-b", deps.now_ms(), Duration::from_secs(60))
        .await
        .expect("acquire worker B")
    {
        WorkerLeaseAcquireOutcome::Acquired { authority, .. } => authority,
        WorkerLeaseAcquireOutcome::Held { .. } => panic!("worker B lease unexpectedly held"),
    };

    let lease_lost = deps
        .renew_lease(&authority_a, deps.now_ms(), Duration::from_secs(60))
        .await
        .expect_err("stale renewal loses the lease");
    assert_eq!(lease_lost.kind, WorkerCoreErrorKind::LeaseLost);
    assert!(!lease_lost.message.contains("worker-a"));

    let transition_lease_lost = deps
        .reread_cancel_requested(&authority_a, &fixture.job_id)
        .await
        .expect_err("stale repository transition loses the lease");
    assert_eq!(transition_lease_lost.kind, WorkerCoreErrorKind::LeaseLost);
    assert!(!transition_lease_lost.message.contains("worker-a"));

    let repository = deps
        .heartbeat_job(&authority_b, "missing-job-secret-sentinel", deps.now_ms())
        .await
        .expect_err("missing heartbeat is repository failure");
    assert_eq!(repository.kind, WorkerCoreErrorKind::Transient);
    assert!(!repository.message.contains("secret-sentinel"));

    events.fail.store(true, Ordering::SeqCst);
    let claimed = deps
        .claim_next(&authority_b, deps.now_ms())
        .await
        .expect("claim for event failure")
        .expect("queued job remains available");
    let event_error = deps
        .emit_committed_event(claimed.committed_event().clone())
        .expect_err("event sink failure is typed");
    assert_eq!(event_error.kind, WorkerCoreErrorKind::Transient);
    assert_eq!(
        event_error.message,
        "committed generation event could not be emitted"
    );

    let diagnostic = WorkerDiagnostic {
        kind: WorkerDiagnosticKind::EmitCommittedEvent,
        job_id: Some(fixture.job_id.clone()),
        message: "fixed diagnostic".to_string(),
    };
    deps.record_diagnostic(diagnostic.clone());
    assert_eq!(
        diagnostics
            .values
            .lock()
            .expect("read forwarded diagnostics")
            .as_slice(),
        [diagnostic]
    );
}

#[tokio::test]
async fn delegated_reconciliation_and_execution_errors_are_sanitized_without_losing_kind() {
    let fixture = Fixture::new();
    let clock = Arc::new(FixedClock::new(Utc::now().timestamp_millis()));
    let deps = fixture.deps(
        Arc::clone(&clock),
        FakeExecutor::returning(Err(WorkerCoreError {
            kind: WorkerCoreErrorKind::Transient,
            message: "provider-key-secret-sentinel".to_string(),
        })),
        FakeReconciler::returning(Err(WorkerCoreError {
            kind: WorkerCoreErrorKind::LeaseLost,
            message: "database-path-secret-sentinel".to_string(),
        })),
        Arc::new(ObservedEvents::new(fixture.db.clone())),
        Arc::new(ObservedDiagnostics::default()),
    );
    let authority = match deps
        .acquire_lease(
            "sanitization-worker",
            deps.now_ms(),
            Duration::from_secs(60),
        )
        .await
        .expect("acquire sanitization lease")
    {
        WorkerLeaseAcquireOutcome::Acquired { authority, .. } => authority,
        WorkerLeaseAcquireOutcome::Held { .. } => {
            panic!("sanitization lease unexpectedly held")
        }
    };

    let reconciliation = deps
        .reconcile_startup(&authority, deps.now_ms())
        .await
        .expect_err("scripted reconciliation must fail");
    assert_eq!(reconciliation.kind, WorkerCoreErrorKind::LeaseLost);
    assert_eq!(
        reconciliation.message,
        "generation startup reconciliation failed"
    );
    assert!(!reconciliation.message.contains("secret-sentinel"));

    let (_, cancellation) = watch::channel(false);
    let execution = deps
        .execute_job(
            &authority,
            WorkerExecutionJob::new(&fixture.job_id),
            cancellation,
        )
        .await
        .expect_err("scripted execution must fail");
    assert_eq!(execution.kind, WorkerCoreErrorKind::Transient);
    assert_eq!(execution.message, "generation job execution failed");
    assert!(!execution.message.contains("secret-sentinel"));
}
