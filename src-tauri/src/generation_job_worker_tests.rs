use super::*;
use crate::generation_worker_lease::{WorkerLeaseAcquireOutcome, WorkerTransitionAuthority};
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

fn assert_send_sync_static<T: Send + Sync + 'static>() {}

fn generation_job_event(job_id: impl Into<String>) -> GenerationJobEvent {
    let job_id = job_id.into();
    GenerationJobEvent {
        job_id: job_id.clone(),
        generation_id: format!("generation-{job_id}"),
        conversation_id: "worker-test-conversation".to_owned(),
        source_kind: "worker_test".to_owned(),
        source_ref: serde_json::json!({"job_id": job_id}),
        status: GenerationJobStatus::Running,
        stage: crate::models::GenerationJobStage::Preparing,
        queue_position: None,
        chain_attempt: 1,
        auto_attempt: 0,
        max_auto_attempts: 2,
        cancel_requested_at: None,
        error_code: None,
        error_message: None,
        retryable: false,
        queued_at: "2026-07-13T00:00:00Z".to_owned(),
        started_at: Some("2026-07-13T00:00:01Z".to_owned()),
        finished_at: None,
    }
}

fn claimed_job(job_id: impl Into<String>) -> ClaimedGenerationJob {
    ClaimedGenerationJob::from_committed_event(generation_job_event(job_id))
}

#[test]
fn managed_queue_and_object_safe_dependencies_are_send_sync_static() {
    assert_send_sync_static::<GenerationJobQueue>();
    assert_send_sync_static::<Arc<dyn GenerationJobWorkerDeps>>();
}

#[test]
fn queue_owner_and_lease_ttl_match_worker_lease_canonical_validation() {
    let config = GenerationJobWorkerConfig::new(
        Duration::from_secs(1),
        Duration::from_millis(100),
        Duration::from_millis(100),
    )
    .expect("valid canonical config");
    for owner in ["worker", "worker-1", "worker_1", "worker.1", "worker:1"] {
        GenerationJobQueue::new(owner, config).expect("canonical owner accepted");
    }
    let too_long = "a".repeat(129);
    for owner in [
        "".to_owned(),
        " leading".to_owned(),
        "trailing ".to_owned(),
        "worker/slash".to_owned(),
        "工作者".to_owned(),
        too_long,
    ] {
        assert_eq!(
            GenerationJobQueue::new(owner, config).err(),
            Some(GenerationJobQueueStartError::InvalidOwner)
        );
    }

    assert_eq!(
        GenerationJobWorkerConfig::new(
            Duration::from_micros(1_500),
            Duration::from_micros(500),
            Duration::from_millis(1),
        ),
        Err(GenerationJobWorkerConfigError::InvalidInterval)
    );
    assert_eq!(
        GenerationJobWorkerConfig::new(
            Duration::from_millis(i64::MAX as u64).saturating_add(Duration::from_millis(1)),
            Duration::from_millis(1),
            Duration::from_millis(1),
        ),
        Err(GenerationJobWorkerConfigError::InvalidInterval)
    );
}

#[test]
fn duplicate_cancellation_registration_preserves_the_original_channel() {
    let cancellations = StdMutex::new(std::collections::HashMap::new());
    let (first_sender, _) = tokio::sync::watch::channel(false);
    let (duplicate_sender, _) = tokio::sync::watch::channel(false);
    let first = CancellationRegistration::insert(
        &cancellations,
        "duplicate-job".to_owned(),
        first_sender.clone(),
    )
    .expect("first registration succeeds");

    let duplicate = CancellationRegistration::insert(
        &cancellations,
        "duplicate-job".to_owned(),
        duplicate_sender,
    );
    assert_eq!(
        duplicate.err(),
        Some(CancellationRegistrationError::DuplicateJobId)
    );
    assert!(cancellations
        .lock()
        .expect("lock cancellation registry")
        .get("duplicate-job")
        .is_some_and(|sender| sender.same_channel(&first_sender)));

    drop(first);
    assert!(cancellations
        .lock()
        .expect("lock cancellation registry")
        .is_empty());
}

#[tokio::test(start_paused = true)]
async fn wall_clock_rollback_cannot_extend_monotonic_expiry_beyond_ttl() {
    let ttl = Duration::from_secs(1);
    let mut expiry = KnownLeaseExpiry::from_wall_clock(10_000, 11_000, ttl)
        .expect("initial lease expiry maps to monotonic time");
    tokio::time::advance(Duration::from_millis(100)).await;

    assert!(expiry.replace_from_wall_clock(9_000, 11_000, ttl));
    assert_eq!(
        expiry
            .deadline(9_000)
            .saturating_duration_since(tokio::time::Instant::now()),
        ttl
    );
}

struct PassiveDeps {
    acquire_calls: AtomicUsize,
    held_for_ms: AtomicU64,
    now_ms: AtomicI64,
    advance_clock_during_acquire: AtomicI64,
}

impl Default for PassiveDeps {
    fn default() -> Self {
        Self {
            acquire_calls: AtomicUsize::new(0),
            held_for_ms: AtomicU64::new(0),
            now_ms: AtomicI64::new(1_000),
            advance_clock_during_acquire: AtomicI64::new(-1),
        }
    }
}

impl PassiveDeps {
    fn hold_for(duration: Duration) -> Self {
        Self {
            acquire_calls: AtomicUsize::new(0),
            held_for_ms: AtomicU64::new(
                u64::try_from(duration.as_millis()).expect("test held duration fits u64"),
            ),
            now_ms: AtomicI64::new(1_000),
            advance_clock_during_acquire: AtomicI64::new(-1),
        }
    }

    fn hold_for_and_advance_clock(held_for: Duration, now_ms: i64) -> Self {
        let deps = Self::hold_for(held_for);
        deps.advance_clock_during_acquire
            .store(now_ms, Ordering::SeqCst);
        deps
    }
}

#[async_trait]
impl GenerationJobWorkerDeps for PassiveDeps {
    fn now_ms(&self) -> i64 {
        self.now_ms.load(Ordering::SeqCst)
    }

    async fn acquire_lease(
        &self,
        _owner_id: &str,
        now_ms: i64,
        ttl: Duration,
    ) -> Result<WorkerLeaseAcquireOutcome, WorkerCoreError> {
        self.acquire_calls.fetch_add(1, Ordering::SeqCst);
        let held_for_ms = self.held_for_ms.load(Ordering::SeqCst);
        let expires = now_ms
            + if held_for_ms == 0 {
                i64::try_from(ttl.as_millis()).expect("test ttl fits i64")
            } else {
                i64::try_from(held_for_ms).expect("test held duration fits i64")
            };
        let advance_to = self.advance_clock_during_acquire.swap(-1, Ordering::SeqCst);
        if advance_to >= 0 {
            self.now_ms.store(advance_to, Ordering::SeqCst);
        }
        Ok(WorkerLeaseAcquireOutcome::Held { expires })
    }

    async fn reconcile_startup(
        &self,
        _authority: &WorkerTransitionAuthority,
        _now_ms: i64,
    ) -> Result<StartupReconciliation, WorkerCoreError> {
        Ok(StartupReconciliation::empty())
    }

    async fn claim_next(
        &self,
        _authority: &WorkerTransitionAuthority,
        _now_ms: i64,
    ) -> Result<Option<ClaimedGenerationJob>, WorkerCoreError> {
        Ok(None)
    }

    fn emit_committed_event(&self, _event: GenerationJobEvent) -> Result<(), WorkerCoreError> {
        Ok(())
    }

    async fn reread_cancel_requested(
        &self,
        _authority: &WorkerTransitionAuthority,
        _job_id: &str,
    ) -> Result<bool, WorkerCoreError> {
        Ok(false)
    }

    async fn execute_job(
        &self,
        _authority: &WorkerTransitionAuthority,
        _job: WorkerExecutionJob,
        _cancellation: tokio::sync::watch::Receiver<bool>,
    ) -> Result<WorkerExecutionOutcome, WorkerCoreError> {
        Ok(WorkerExecutionOutcome::DurablyFinished)
    }

    async fn renew_lease(
        &self,
        _authority: &WorkerTransitionAuthority,
        now_ms: i64,
        ttl: Duration,
    ) -> Result<i64, WorkerCoreError> {
        Ok(now_ms + i64::try_from(ttl.as_millis()).expect("test ttl fits i64"))
    }

    async fn heartbeat_job(
        &self,
        _authority: &WorkerTransitionAuthority,
        _job_id: &str,
        _now_ms: i64,
    ) -> Result<(), WorkerCoreError> {
        Ok(())
    }

    async fn release_lease(
        &self,
        _authority: &WorkerTransitionAuthority,
    ) -> Result<(), WorkerCoreError> {
        Ok(())
    }

    fn record_diagnostic(&self, _diagnostic: WorkerDiagnostic) {}
}

#[tokio::test]
async fn queue_start_is_single_wakeable_and_shutdown_joins_the_owned_task() {
    let deps = Arc::new(PassiveDeps::default());
    let queue = GenerationJobQueue::new(
        "pure-worker-a",
        GenerationJobWorkerConfig::new(
            Duration::from_millis(100),
            Duration::from_millis(10),
            Duration::from_millis(10),
        )
        .expect("valid worker config"),
    )
    .expect("valid worker queue");

    queue.start(deps.clone()).await.expect("start queue once");
    let second_start = queue
        .start(deps.clone())
        .await
        .expect_err("second start must fail deterministically");
    assert_eq!(second_start, GenerationJobQueueStartError::AlreadyStarted);
    queue.wake();
    tokio::time::timeout(Duration::from_secs(1), async {
        while deps.acquire_calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("worker task must observe start or wake");

    tokio::time::timeout(Duration::from_secs(1), queue.shutdown())
        .await
        .expect("shutdown must join the owned task");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_start_and_shutdown_publish_and_join_one_lifecycle_handle() {
    let deps = Arc::new(PassiveDeps::default());
    let queue = GenerationJobQueue::new(
        "lifecycle-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_millis(100),
            Duration::from_millis(20),
            Duration::from_millis(10),
        )
        .expect("valid lifecycle config"),
    )
    .expect("valid lifecycle queue");
    assert_eq!(
        queue.lifecycle_state_for_test(),
        QueueLifecycleSnapshot::Created
    );

    let starts = (0..16)
        .map(|_| {
            let queue = queue.clone();
            let deps = deps.clone();
            tokio::spawn(async move { queue.start(deps).await })
        })
        .collect::<Vec<_>>();
    let mut started = 0;
    let mut already_started = 0;
    for start in starts {
        match start.await.expect("start task must not panic") {
            Ok(()) => started += 1,
            Err(GenerationJobQueueStartError::AlreadyStarted) => already_started += 1,
            result => panic!("unexpected concurrent start result: {result:?}"),
        }
    }
    assert_eq!((started, already_started), (1, 15));
    assert_eq!(
        queue.lifecycle_state_for_test(),
        QueueLifecycleSnapshot::Running
    );

    let shutdowns = (0..8)
        .map(|_| {
            let queue = queue.clone();
            tokio::spawn(async move { queue.shutdown().await })
        })
        .collect::<Vec<_>>();
    for shutdown in shutdowns {
        tokio::time::timeout(Duration::from_secs(1), shutdown)
            .await
            .expect("every shutdown caller waits for join")
            .expect("shutdown task must not panic");
    }
    assert_eq!(
        queue.lifecycle_state_for_test(),
        QueueLifecycleSnapshot::Stopped
    );
}

#[tokio::test(start_paused = true)]
async fn held_lease_waits_for_the_earlier_of_expiry_and_bounded_poll() {
    async fn acquire_calls_after(
        held_for: Duration,
        poll: Duration,
        advance: Duration,
        owner_id: &str,
    ) -> usize {
        let deps = Arc::new(PassiveDeps::hold_for(held_for));
        let queue = GenerationJobQueue::new(
            owner_id,
            GenerationJobWorkerConfig::new(Duration::from_secs(20), Duration::from_secs(1), poll)
                .expect("valid held config"),
        )
        .expect("valid held queue");
        queue.start(deps.clone()).await.expect("start held worker");
        for _ in 0..100 {
            if deps.acquire_calls.load(Ordering::SeqCst) >= 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
        tokio::time::advance(advance).await;
        for _ in 0..100 {
            if deps.acquire_calls.load(Ordering::SeqCst) >= 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
        let calls = deps.acquire_calls.load(Ordering::SeqCst);
        queue.shutdown().await;
        calls
    }

    let expiry_calls = acquire_calls_after(
        Duration::from_millis(50),
        Duration::from_secs(5),
        Duration::from_millis(50),
        "held-expiry-worker",
    )
    .await;
    let poll_calls = acquire_calls_after(
        Duration::from_secs(10),
        Duration::from_secs(5),
        Duration::from_secs(5),
        "held-poll-worker",
    )
    .await;

    assert!(expiry_calls >= 2, "lease expiry must beat bounded poll");
    assert!(
        poll_calls >= 2,
        "bounded poll must discover work without wake"
    );
}

#[derive(Debug, Clone, Copy)]
enum LeaseLossPoint {
    Reconcile,
    Claim,
    CancelRead,
    ExecutionOutcome,
    Heartbeat,
}

struct ScriptedDeps {
    now_ms: AtomicI64,
    state: StdMutex<ScriptedState>,
    durable_cancel_requested: AtomicBool,
    cancel_read_fails: AtomicBool,
    lose_lease_on_reconcile: AtomicBool,
    lose_lease_on_claim: AtomicBool,
    lose_lease_on_cancel_read: AtomicBool,
    next_execute_behavior: AtomicUsize,
    execution_delay_ms: AtomicU64,
    block_next_execution: AtomicBool,
    fail_next_heartbeat: AtomicBool,
    lose_lease_on_heartbeat: AtomicBool,
    lose_lease_on_next_renew: AtomicBool,
    fail_next_renew_transiently: AtomicBool,
    renew_always_transient: AtomicBool,
    advance_clock_during_renew: AtomicI64,
    fail_next_event_emit: AtomicBool,
    advance_clock_after_claim: AtomicI64,
    advance_clock_after_cancel_read: AtomicI64,
    block_next_claim: AtomicBool,
    resume_claim: tokio::sync::Notify,
    block_next_cancel_read: AtomicBool,
    resume_cancel_read: tokio::sync::Notify,
    startup_reconciliation: StdMutex<Option<StartupReconciliation>>,
    cancel_probe: StdMutex<Option<Arc<dyn Fn(&str) -> bool + Send + Sync>>>,
}

struct ScriptedState {
    queued: VecDeque<String>,
    operations: Vec<String>,
    miss_next_claim: bool,
}

impl ScriptedDeps {
    fn with_jobs(job_ids: impl IntoIterator<Item = &'static str>) -> Self {
        Self {
            now_ms: AtomicI64::new(10_000),
            state: StdMutex::new(ScriptedState {
                queued: job_ids.into_iter().map(str::to_string).collect(),
                operations: Vec::new(),
                miss_next_claim: false,
            }),
            durable_cancel_requested: AtomicBool::new(false),
            cancel_read_fails: AtomicBool::new(false),
            lose_lease_on_reconcile: AtomicBool::new(false),
            lose_lease_on_claim: AtomicBool::new(false),
            lose_lease_on_cancel_read: AtomicBool::new(false),
            next_execute_behavior: AtomicUsize::new(0),
            execution_delay_ms: AtomicU64::new(0),
            block_next_execution: AtomicBool::new(false),
            fail_next_heartbeat: AtomicBool::new(false),
            lose_lease_on_heartbeat: AtomicBool::new(false),
            lose_lease_on_next_renew: AtomicBool::new(false),
            fail_next_renew_transiently: AtomicBool::new(false),
            renew_always_transient: AtomicBool::new(false),
            advance_clock_during_renew: AtomicI64::new(-1),
            fail_next_event_emit: AtomicBool::new(false),
            advance_clock_after_claim: AtomicI64::new(-1),
            advance_clock_after_cancel_read: AtomicI64::new(-1),
            block_next_claim: AtomicBool::new(false),
            resume_claim: tokio::sync::Notify::new(),
            block_next_cancel_read: AtomicBool::new(false),
            resume_cancel_read: tokio::sync::Notify::new(),
            startup_reconciliation: StdMutex::new(None),
            cancel_probe: StdMutex::new(None),
        }
    }

    fn with_job_after_empty_snapshot(job_id: &'static str) -> Self {
        Self {
            now_ms: AtomicI64::new(10_000),
            state: StdMutex::new(ScriptedState {
                queued: VecDeque::from([job_id.to_owned()]),
                operations: Vec::new(),
                miss_next_claim: true,
            }),
            durable_cancel_requested: AtomicBool::new(false),
            cancel_read_fails: AtomicBool::new(false),
            lose_lease_on_reconcile: AtomicBool::new(false),
            lose_lease_on_claim: AtomicBool::new(false),
            lose_lease_on_cancel_read: AtomicBool::new(false),
            next_execute_behavior: AtomicUsize::new(0),
            execution_delay_ms: AtomicU64::new(0),
            block_next_execution: AtomicBool::new(false),
            fail_next_heartbeat: AtomicBool::new(false),
            lose_lease_on_heartbeat: AtomicBool::new(false),
            lose_lease_on_next_renew: AtomicBool::new(false),
            fail_next_renew_transiently: AtomicBool::new(false),
            renew_always_transient: AtomicBool::new(false),
            advance_clock_during_renew: AtomicI64::new(-1),
            fail_next_event_emit: AtomicBool::new(false),
            advance_clock_after_claim: AtomicI64::new(-1),
            advance_clock_after_cancel_read: AtomicI64::new(-1),
            block_next_claim: AtomicBool::new(false),
            resume_claim: tokio::sync::Notify::new(),
            block_next_cancel_read: AtomicBool::new(false),
            resume_cancel_read: tokio::sync::Notify::new(),
            startup_reconciliation: StdMutex::new(None),
            cancel_probe: StdMutex::new(None),
        }
    }

    fn request_durable_cancellation(&self) {
        self.durable_cancel_requested.store(true, Ordering::SeqCst);
    }

    fn fail_cancel_reread(&self) {
        self.cancel_read_fails.store(true, Ordering::SeqCst);
    }

    fn lose_lease_at(&self, point: LeaseLossPoint) {
        match point {
            LeaseLossPoint::Reconcile => self.lose_lease_on_reconcile.store(true, Ordering::SeqCst),
            LeaseLossPoint::Claim => self.lose_lease_on_claim.store(true, Ordering::SeqCst),
            LeaseLossPoint::CancelRead => {
                self.lose_lease_on_cancel_read.store(true, Ordering::SeqCst)
            }
            LeaseLossPoint::ExecutionOutcome => {
                self.next_execute_behavior.store(4, Ordering::SeqCst)
            }
            LeaseLossPoint::Heartbeat => self.lose_lease_on_heartbeat.store(true, Ordering::SeqCst),
        }
    }

    fn fail_next_execution(&self) {
        self.next_execute_behavior.store(1, Ordering::SeqCst);
    }

    fn panic_next_execution(&self) {
        self.next_execute_behavior.store(2, Ordering::SeqCst);
    }

    fn require_reconciliation_after_next_execution(&self) {
        self.next_execute_behavior.store(3, Ordering::SeqCst);
    }

    fn delay_each_execution(&self, delay: Duration) {
        self.execution_delay_ms.store(
            u64::try_from(delay.as_millis()).expect("test execution delay fits u64"),
            Ordering::SeqCst,
        );
    }

    fn block_next_execution_until_cancelled(&self) {
        self.block_next_execution.store(true, Ordering::SeqCst);
    }

    fn fail_next_heartbeat(&self) {
        self.fail_next_heartbeat.store(true, Ordering::SeqCst);
    }

    fn lose_lease_on_next_renew(&self) {
        self.lose_lease_on_next_renew.store(true, Ordering::SeqCst);
    }

    fn fail_next_renew_transiently_and_advance_clock(&self, now_ms: i64) {
        self.advance_clock_during_renew
            .store(now_ms, Ordering::SeqCst);
        self.fail_next_renew_transiently
            .store(true, Ordering::SeqCst);
    }

    fn make_renewals_transient(&self) {
        self.renew_always_transient.store(true, Ordering::SeqCst);
    }

    fn advance_clock_during_next_successful_renew(&self, now_ms: i64) {
        self.advance_clock_during_renew
            .store(now_ms, Ordering::SeqCst);
    }

    fn fail_next_event_emit(&self) {
        self.fail_next_event_emit.store(true, Ordering::SeqCst);
    }

    fn advance_clock_after_claim(&self, now_ms: i64) {
        self.advance_clock_after_claim
            .store(now_ms, Ordering::SeqCst);
    }

    fn advance_clock_after_cancel_read(&self, now_ms: i64) {
        self.advance_clock_after_cancel_read
            .store(now_ms, Ordering::SeqCst);
    }

    fn block_claim_until_resumed(&self) {
        self.block_next_claim.store(true, Ordering::SeqCst);
    }

    fn resume_claim(&self) {
        self.resume_claim.notify_one();
    }

    fn block_cancel_read_until_resumed(&self) {
        self.block_next_cancel_read.store(true, Ordering::SeqCst);
    }

    fn resume_cancel_read(&self) {
        self.resume_cancel_read.notify_one();
    }

    fn install_startup_reconciliation(
        &self,
        event_job_ids: impl IntoIterator<Item = &'static str>,
        recovery_job_ids: impl IntoIterator<Item = &'static str>,
    ) {
        *self
            .startup_reconciliation
            .lock()
            .expect("lock startup reconciliation") = Some(StartupReconciliation::new(
            event_job_ids
                .into_iter()
                .map(generation_job_event)
                .collect(),
            recovery_job_ids
                .into_iter()
                .map(WorkerExecutionJob::new)
                .collect(),
        ));
    }

    fn install_cancel_probe(&self, probe: impl Fn(&str) -> bool + Send + Sync + 'static) {
        *self.cancel_probe.lock().expect("lock cancel probe") = Some(Arc::new(probe));
    }

    fn record(&self, operation: impl Into<String>) {
        self.state
            .lock()
            .expect("lock scripted worker state")
            .operations
            .push(operation.into());
    }

    fn operations(&self) -> Vec<String> {
        self.state
            .lock()
            .expect("lock scripted worker state")
            .operations
            .clone()
    }
}

#[async_trait]
impl GenerationJobWorkerDeps for ScriptedDeps {
    fn now_ms(&self) -> i64 {
        self.now_ms.load(Ordering::SeqCst)
    }

    async fn acquire_lease(
        &self,
        owner_id: &str,
        now_ms: i64,
        _ttl: Duration,
    ) -> Result<WorkerLeaseAcquireOutcome, WorkerCoreError> {
        self.record(format!("acquire:{owner_id}:{now_ms}"));
        Ok(WorkerLeaseAcquireOutcome::Acquired {
            authority: WorkerTransitionAuthority::for_test(owner_id, 7),
            expires: now_ms + 1_000,
        })
    }

    async fn reconcile_startup(
        &self,
        authority: &WorkerTransitionAuthority,
        now_ms: i64,
    ) -> Result<StartupReconciliation, WorkerCoreError> {
        self.record(format!(
            "reconcile:{}:{}:{now_ms}",
            authority.owner_id(),
            authority.fencing_epoch()
        ));
        if self.lose_lease_on_reconcile.swap(false, Ordering::SeqCst) {
            return Err(WorkerCoreError {
                kind: WorkerCoreErrorKind::LeaseLost,
                message: "scripted reconciliation lease loss".to_owned(),
            });
        }
        Ok(self
            .startup_reconciliation
            .lock()
            .expect("lock startup reconciliation")
            .take()
            .unwrap_or_else(StartupReconciliation::empty))
    }

    async fn claim_next(
        &self,
        authority: &WorkerTransitionAuthority,
        now_ms: i64,
    ) -> Result<Option<ClaimedGenerationJob>, WorkerCoreError> {
        if self.lose_lease_on_claim.swap(false, Ordering::SeqCst) {
            self.record(format!(
                "claim:{}:{}:{now_ms}:lease-lost",
                authority.owner_id(),
                authority.fencing_epoch()
            ));
            return Err(WorkerCoreError {
                kind: WorkerCoreErrorKind::LeaseLost,
                message: "scripted claim lease loss".to_owned(),
            });
        }
        let job_id = {
            let mut state = self.state.lock().expect("lock scripted worker state");
            if std::mem::take(&mut state.miss_next_claim) {
                None
            } else {
                state.queued.pop_front()
            }
        };
        self.record(format!(
            "claim:{}:{}:{now_ms}:{}",
            authority.owner_id(),
            authority.fencing_epoch(),
            job_id.as_deref().unwrap_or("none")
        ));
        let advance_to = self.advance_clock_after_claim.swap(-1, Ordering::SeqCst);
        if advance_to >= 0 && job_id.is_some() {
            self.now_ms.store(advance_to, Ordering::SeqCst);
        }
        if self.block_next_claim.swap(false, Ordering::SeqCst) {
            self.record(format!(
                "claim-await:{}",
                job_id.as_deref().unwrap_or("none")
            ));
            self.resume_claim.notified().await;
        }
        Ok(job_id.map(claimed_job))
    }

    fn emit_committed_event(&self, event: GenerationJobEvent) -> Result<(), WorkerCoreError> {
        self.record(format!(
            "event:{}:{:?}:{:?}",
            event.job_id, event.status, event.stage
        ));
        if self.fail_next_event_emit.swap(false, Ordering::SeqCst) {
            return Err(WorkerCoreError {
                kind: WorkerCoreErrorKind::Transient,
                message: "scripted event sink failure".to_owned(),
            });
        }
        Ok(())
    }

    async fn reread_cancel_requested(
        &self,
        authority: &WorkerTransitionAuthority,
        job_id: &str,
    ) -> Result<bool, WorkerCoreError> {
        self.record(format!(
            "cancel-read:{}:{}:{job_id}",
            authority.owner_id(),
            authority.fencing_epoch()
        ));
        if self.block_next_cancel_read.swap(false, Ordering::SeqCst) {
            self.record(format!("cancel-read-await:{job_id}"));
            self.resume_cancel_read.notified().await;
        }
        let advance_to = self
            .advance_clock_after_cancel_read
            .swap(-1, Ordering::SeqCst);
        if advance_to >= 0 {
            self.now_ms.store(advance_to, Ordering::SeqCst);
        }
        let probe = self.cancel_probe.lock().expect("lock cancel probe").clone();
        if let Some(probe) = probe {
            self.record(format!("cancel-probe:{job_id}:{}", probe(job_id)));
        }
        if self.cancel_read_fails.load(Ordering::SeqCst) {
            return Err(WorkerCoreError {
                kind: WorkerCoreErrorKind::Transient,
                message: "cancel reread unavailable".to_owned(),
            });
        }
        if self.lose_lease_on_cancel_read.swap(false, Ordering::SeqCst) {
            return Err(WorkerCoreError {
                kind: WorkerCoreErrorKind::LeaseLost,
                message: "scripted cancel-read lease loss".to_owned(),
            });
        }
        Ok(self.durable_cancel_requested.load(Ordering::SeqCst))
    }

    async fn execute_job(
        &self,
        authority: &WorkerTransitionAuthority,
        job: WorkerExecutionJob,
        mut cancellation: tokio::sync::watch::Receiver<bool>,
    ) -> Result<WorkerExecutionOutcome, WorkerCoreError> {
        self.record(format!(
            "cancel-observed:{}:{}",
            job.job_id(),
            *cancellation.borrow()
        ));
        self.record(format!(
            "execute:{}:{}:{}",
            authority.owner_id(),
            authority.fencing_epoch(),
            job.job_id()
        ));
        let execution_delay_ms = self.execution_delay_ms.load(Ordering::SeqCst);
        if execution_delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(execution_delay_ms)).await;
            self.record(format!("execute-finished:{}", job.job_id()));
        }
        if self.block_next_execution.swap(false, Ordering::SeqCst) {
            while !*cancellation.borrow() {
                if cancellation.changed().await.is_err() {
                    break;
                }
            }
            self.record(format!("executor-cancelled:{}", job.job_id()));
            return Ok(WorkerExecutionOutcome::DurablyFinished);
        }
        match self.next_execute_behavior.swap(0, Ordering::SeqCst) {
            1 => Err(WorkerCoreError {
                kind: WorkerCoreErrorKind::Transient,
                message: "scripted execution failure".to_owned(),
            }),
            2 => panic!("scripted executor panic api-key=panic-secret-sentinel"),
            3 => Ok(WorkerExecutionOutcome::NeedsReconciliation),
            4 => Ok(WorkerExecutionOutcome::LeaseLost),
            _ => Ok(WorkerExecutionOutcome::DurablyFinished),
        }
    }

    async fn renew_lease(
        &self,
        authority: &WorkerTransitionAuthority,
        now_ms: i64,
        ttl: Duration,
    ) -> Result<i64, WorkerCoreError> {
        self.record(format!(
            "renew:{}:{}:{now_ms}",
            authority.owner_id(),
            authority.fencing_epoch()
        ));
        if self.lose_lease_on_next_renew.swap(false, Ordering::SeqCst) {
            return Err(WorkerCoreError {
                kind: WorkerCoreErrorKind::LeaseLost,
                message: "scripted lease loss".to_owned(),
            });
        }
        let expires = now_ms + i64::try_from(ttl.as_millis()).expect("test ttl fits i64");
        let advance_to = self.advance_clock_during_renew.swap(-1, Ordering::SeqCst);
        if advance_to >= 0 {
            self.now_ms.store(advance_to, Ordering::SeqCst);
        }
        if self
            .fail_next_renew_transiently
            .swap(false, Ordering::SeqCst)
            || self.renew_always_transient.load(Ordering::SeqCst)
        {
            return Err(WorkerCoreError {
                kind: WorkerCoreErrorKind::Transient,
                message: "scripted transient renewal failure".to_owned(),
            });
        }
        Ok(expires)
    }

    async fn heartbeat_job(
        &self,
        authority: &WorkerTransitionAuthority,
        job_id: &str,
        now_ms: i64,
    ) -> Result<(), WorkerCoreError> {
        self.record(format!(
            "heartbeat:{}:{}:{now_ms}:{job_id}",
            authority.owner_id(),
            authority.fencing_epoch()
        ));
        if self.fail_next_heartbeat.swap(false, Ordering::SeqCst) {
            return Err(WorkerCoreError {
                kind: WorkerCoreErrorKind::Transient,
                message: "scripted heartbeat failure".to_owned(),
            });
        }
        if self.lose_lease_on_heartbeat.swap(false, Ordering::SeqCst) {
            return Err(WorkerCoreError {
                kind: WorkerCoreErrorKind::LeaseLost,
                message: "scripted heartbeat lease loss".to_owned(),
            });
        }
        Ok(())
    }

    async fn release_lease(
        &self,
        authority: &WorkerTransitionAuthority,
    ) -> Result<(), WorkerCoreError> {
        self.record(format!(
            "release:{}:{}",
            authority.owner_id(),
            authority.fencing_epoch()
        ));
        Ok(())
    }

    fn record_diagnostic(&self, diagnostic: WorkerDiagnostic) {
        self.record(format!("diagnostic:{:?}", diagnostic.kind));
        self.record(format!("diagnostic-message:{}", diagnostic.message));
    }
}

#[tokio::test]
async fn acquired_worker_reconciles_drains_fifo_and_releases_the_same_authority() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["first", "second", "third"]));
    let queue = GenerationJobQueue::new(
        "drain-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_millis(500),
            Duration::from_millis(100),
            Duration::from_millis(50),
        )
        .expect("valid drain config"),
    )
    .expect("valid drain queue");
    queue.start(deps.clone()).await.expect("start drain worker");

    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let executed = deps
                .operations()
                .into_iter()
                .filter(|operation| operation.starts_with("execute:"))
                .count();
            if executed == 3 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("worker must drain all queued jobs");
    queue.shutdown().await;

    let operations = deps.operations();
    let executed = operations
        .iter()
        .filter(|operation| operation.starts_with("execute:"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        executed,
        [
            "execute:drain-worker:7:first",
            "execute:drain-worker:7:second",
            "execute:drain-worker:7:third",
        ]
    );
    let reconcile = operations
        .iter()
        .position(|operation| operation.starts_with("reconcile:"))
        .expect("startup reconciliation recorded");
    let first_claim = operations
        .iter()
        .position(|operation| operation.contains(":first"))
        .expect("first claim recorded");
    assert!(reconcile < first_claim);
    assert_eq!(
        operations.last().map(String::as_str),
        Some("release:drain-worker:7")
    );
}

#[tokio::test]
async fn empty_snapshot_registers_waiter_then_rechecks_before_bounded_poll() {
    let deps = Arc::new(ScriptedDeps::with_job_after_empty_snapshot("late-job"));
    let queue = GenerationJobQueue::new(
        "lost-wake-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(10),
            Duration::from_secs(1),
            Duration::from_secs(5),
        )
        .expect("valid lost-wake config"),
    )
    .expect("valid lost-wake queue");
    queue
        .start(deps.clone())
        .await
        .expect("start lost-wake worker");

    let observed_without_polling = tokio::time::timeout(Duration::from_millis(200), async {
        loop {
            if deps
                .operations()
                .iter()
                .any(|operation| operation == "execute:lost-wake-worker:7:late-job")
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    queue.shutdown().await;

    observed_without_polling.expect(
        "worker must register its Notify waiter and recheck after an empty snapshot before polling",
    );
}

#[tokio::test]
async fn cancellation_is_registered_before_durable_reread_and_cleaned_after_execution() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["cancel-me"]));
    deps.request_durable_cancellation();
    let queue = GenerationJobQueue::new(
        "cancel-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(2),
            Duration::from_millis(200),
            Duration::from_millis(50),
        )
        .expect("valid cancel config"),
    )
    .expect("valid cancel queue");
    let cancel_queue = queue.clone();
    deps.install_cancel_probe(move |job_id| cancel_queue.cancel(job_id));
    queue
        .start(deps.clone())
        .await
        .expect("start cancel worker");

    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if deps
                .operations()
                .iter()
                .any(|operation| operation == "cancel-observed:cancel-me:true")
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("durable cancellation must reach execution");
    assert_eq!(queue.registered_cancellation_count_for_test(), 0);
    queue.shutdown().await;

    let operations = deps.operations();
    assert!(operations
        .iter()
        .any(|operation| operation == "cancel-probe:cancel-me:true"));
    let cancel_read = operations
        .iter()
        .position(|operation| operation.starts_with("cancel-read:"))
        .expect("durable cancel reread recorded");
    let execution = operations
        .iter()
        .position(|operation| operation.starts_with("execute:"))
        .expect("cancel-aware execution recorded");
    assert!(cancel_read < execution);
}

#[tokio::test]
async fn cancellation_reread_error_fails_closed_and_cleans_registration() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["unknown-cancel-state"]));
    deps.fail_cancel_reread();
    let queue = GenerationJobQueue::new(
        "fail-closed-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(2),
            Duration::from_millis(200),
            Duration::from_millis(50),
        )
        .expect("valid fail-closed config"),
    )
    .expect("valid fail-closed queue");
    queue
        .start(deps.clone())
        .await
        .expect("start fail-closed worker");

    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if deps
                .operations()
                .iter()
                .any(|operation| operation == "cancel-observed:unknown-cancel-state:true")
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("unknown durable cancellation state must fail closed");
    assert_eq!(queue.registered_cancellation_count_for_test(), 0);
    queue.shutdown().await;

    assert!(deps
        .operations()
        .iter()
        .any(|operation| { operation == "diagnostic:ReadCancellation" }));
}

#[tokio::test]
async fn executor_error_cleans_registration_reconciles_then_drain_continues() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["fails", "after-error"]));
    deps.fail_next_execution();
    let queue = GenerationJobQueue::new(
        "error-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(2),
            Duration::from_millis(200),
            Duration::from_millis(50),
        )
        .expect("valid error config"),
    )
    .expect("valid error queue");
    queue.start(deps.clone()).await.expect("start error worker");

    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if deps
                .operations()
                .iter()
                .any(|operation| operation == "execute:error-worker:7:after-error")
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("executor error must not stop the drain");
    assert_eq!(queue.registered_cancellation_count_for_test(), 0);
    queue.shutdown().await;
    let operations = deps.operations();
    assert!(operations
        .iter()
        .any(|operation| operation == "diagnostic:ExecuteJob"));
    let failed_execution = operations
        .iter()
        .position(|operation| operation == "execute:error-worker:7:fails")
        .expect("failed execution recorded");
    let release = operations
        .iter()
        .position(|operation| operation == "release:error-worker:7")
        .expect("uncertain session released");
    let second_acquire = operations
        .iter()
        .enumerate()
        .filter(|(_, operation)| operation.starts_with("acquire:error-worker:"))
        .nth(1)
        .map(|(index, _)| index)
        .expect("worker reacquired after uncertain execution");
    let second_reconcile = operations
        .iter()
        .enumerate()
        .filter(|(_, operation)| operation.starts_with("reconcile:error-worker:"))
        .nth(1)
        .map(|(index, _)| index)
        .expect("worker reconciled after reacquiring");
    let next_execution = operations
        .iter()
        .position(|operation| operation == "execute:error-worker:7:after-error")
        .expect("next execution recorded");
    assert!(failed_execution < release);
    assert!(release < second_acquire);
    assert!(second_acquire < second_reconcile);
    assert!(second_reconcile < next_execution);
}

#[tokio::test]
async fn executor_panic_cleans_registration_reconciles_then_drain_continues() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["panics", "after-panic"]));
    deps.panic_next_execution();
    let queue = GenerationJobQueue::new(
        "panic-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(2),
            Duration::from_millis(200),
            Duration::from_millis(50),
        )
        .expect("valid panic config"),
    )
    .expect("valid panic queue");
    queue.start(deps.clone()).await.expect("start panic worker");

    let drain_continued = tokio::time::timeout(Duration::from_millis(300), async {
        loop {
            if deps
                .operations()
                .iter()
                .any(|operation| operation == "execute:panic-worker:7:after-panic")
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    queue.shutdown().await;

    drain_continued.expect("executor panic must be isolated from the worker session");
    assert_eq!(queue.registered_cancellation_count_for_test(), 0);
    let operations = deps.operations();
    assert!(operations
        .iter()
        .any(|operation| operation == "diagnostic:ExecutePanic"));
    assert!(
        operations
            .iter()
            .all(|operation| !operation.contains("panic-secret-sentinel")),
        "panic payloads may contain provider secrets and must never reach diagnostics"
    );
    let panic_execution = operations
        .iter()
        .position(|operation| operation == "execute:panic-worker:7:panics")
        .expect("panicking execution recorded");
    let release = operations
        .iter()
        .position(|operation| operation == "release:panic-worker:7")
        .expect("panic session released");
    let second_reconcile = operations
        .iter()
        .enumerate()
        .filter(|(_, operation)| operation.starts_with("reconcile:panic-worker:"))
        .nth(1)
        .map(|(index, _)| index)
        .expect("worker reconciled after panic");
    let next_execution = operations
        .iter()
        .position(|operation| operation == "execute:panic-worker:7:after-panic")
        .expect("post-panic execution recorded");
    assert!(panic_execution < release);
    assert!(release < second_reconcile);
    assert!(second_reconcile < next_execution);
}

#[tokio::test]
async fn explicit_needs_reconciliation_blocks_the_next_claim_until_reacquired() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["uncertain", "after-reconcile"]));
    deps.require_reconciliation_after_next_execution();
    let queue = GenerationJobQueue::new(
        "outcome-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(2),
            Duration::from_millis(200),
            Duration::from_millis(50),
        )
        .expect("valid outcome config"),
    )
    .expect("valid outcome queue");
    queue
        .start(deps.clone())
        .await
        .expect("start outcome worker");
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if deps
                .operations()
                .iter()
                .any(|operation| operation == "execute:outcome-worker:7:after-reconcile")
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("worker must reconcile and resume");
    queue.shutdown().await;

    let operations = deps.operations();
    let second_reconcile = operations
        .iter()
        .enumerate()
        .filter(|(_, operation)| operation.starts_with("reconcile:outcome-worker:"))
        .nth(1)
        .map(|(index, _)| index)
        .expect("explicit outcome triggers a second reconciliation");
    let next_execution = operations
        .iter()
        .position(|operation| operation == "execute:outcome-worker:7:after-reconcile")
        .expect("next job executed");
    assert!(second_reconcile < next_execution);
}

#[tokio::test(start_paused = true)]
async fn active_job_renews_before_heartbeat_and_heartbeat_error_is_non_fatal() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["active-job"]));
    deps.block_next_execution_until_cancelled();
    deps.fail_next_heartbeat();
    let queue = GenerationJobQueue::new(
        "heartbeat-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(1),
            Duration::from_millis(100),
            Duration::from_secs(5),
        )
        .expect("valid heartbeat config"),
    )
    .expect("valid heartbeat queue");
    queue
        .start(deps.clone())
        .await
        .expect("start heartbeat worker");
    for _ in 0..100 {
        if queue.registered_cancellation_count_for_test() == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(queue.registered_cancellation_count_for_test(), 1);

    tokio::time::advance(Duration::from_millis(100)).await;
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(100)).await;
    for _ in 0..100 {
        if deps
            .operations()
            .iter()
            .filter(|operation| operation.starts_with("renew:"))
            .count()
            >= 2
        {
            break;
        }
        tokio::task::yield_now().await;
    }
    let operations_before_cleanup = deps.operations();
    let renew_positions = operations_before_cleanup
        .iter()
        .enumerate()
        .filter(|(_, operation)| operation.starts_with("renew:"))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let heartbeat_position = operations_before_cleanup
        .iter()
        .position(|operation| operation.starts_with("heartbeat:"));
    let heartbeat_diagnostic = operations_before_cleanup
        .iter()
        .any(|operation| operation == "diagnostic:HeartbeatJob");

    assert!(queue.cancel("active-job"));
    for _ in 0..100 {
        if queue.registered_cancellation_count_for_test() == 0 {
            break;
        }
        tokio::task::yield_now().await;
    }
    queue.shutdown().await;

    assert!(
        renew_positions.len() >= 2,
        "renew must continue after heartbeat error"
    );
    assert!(
        renew_positions[0] < heartbeat_position.expect("active heartbeat recorded"),
        "lease renew must precede job heartbeat"
    );
    assert!(
        heartbeat_diagnostic,
        "heartbeat failure must be diagnostic only"
    );
}

#[tokio::test(start_paused = true)]
async fn active_job_rereads_durable_cancellation_on_heartbeat_cadence() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["later-durable-cancel"]));
    deps.block_next_execution_until_cancelled();
    let queue = GenerationJobQueue::new(
        "durable-cadence-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(1),
            Duration::from_millis(100),
            Duration::from_secs(5),
        )
        .expect("valid durable cadence config"),
    )
    .expect("valid durable cadence queue");
    queue
        .start(deps.clone())
        .await
        .expect("start durable cadence worker");
    for _ in 0..100 {
        if queue.registered_cancellation_count_for_test() == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    deps.request_durable_cancellation();

    tokio::time::advance(Duration::from_millis(100)).await;
    for _ in 0..100 {
        if queue.registered_cancellation_count_for_test() == 0 {
            break;
        }
        tokio::task::yield_now().await;
    }
    let cancelled_by_reread = queue.registered_cancellation_count_for_test() == 0;
    if !cancelled_by_reread {
        queue.cancel("later-durable-cancel");
    }
    queue.shutdown().await;

    assert!(
        cancelled_by_reread,
        "heartbeat cadence must observe durable cancellation"
    );
    assert!(
        deps.operations()
            .iter()
            .filter(|operation| operation.ends_with(":later-durable-cancel")
                && operation.starts_with("cancel-read:"))
            .count()
            >= 2
    );
}

#[tokio::test(start_paused = true)]
async fn idle_acquired_worker_renews_while_waiting_for_wake_or_bounded_poll() {
    let deps = Arc::new(ScriptedDeps::with_jobs([]));
    let queue = GenerationJobQueue::new(
        "idle-renew-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(1),
            Duration::from_millis(100),
            Duration::from_secs(5),
        )
        .expect("valid idle renew config"),
    )
    .expect("valid idle renew queue");
    queue
        .start(deps.clone())
        .await
        .expect("start idle renew worker");
    for _ in 0..100 {
        if deps
            .operations()
            .iter()
            .any(|operation| operation.ends_with(":none"))
        {
            break;
        }
        tokio::task::yield_now().await;
    }

    tokio::time::advance(Duration::from_millis(100)).await;
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(100)).await;
    for _ in 0..100 {
        if deps
            .operations()
            .iter()
            .filter(|operation| operation.starts_with("renew:"))
            .count()
            >= 2
        {
            break;
        }
        tokio::task::yield_now().await;
    }
    let renew_count = deps
        .operations()
        .iter()
        .filter(|operation| operation.starts_with("renew:"))
        .count();
    queue.shutdown().await;

    assert!(
        renew_count >= 2,
        "idle lease must renew independently of polling"
    );
    assert!(!deps
        .operations()
        .iter()
        .any(|operation| operation.starts_with("heartbeat:")));
}

#[tokio::test(start_paused = true)]
async fn active_lease_loss_cancels_child_then_returns_to_passive_acquire() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["lease-lost-job"]));
    deps.block_next_execution_until_cancelled();
    deps.lose_lease_on_next_renew();
    let queue = GenerationJobQueue::new(
        "lease-loss-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(1),
            Duration::from_millis(100),
            Duration::from_secs(5),
        )
        .expect("valid lease-loss config"),
    )
    .expect("valid lease-loss queue");
    queue
        .start(deps.clone())
        .await
        .expect("start lease-loss worker");
    for _ in 0..100 {
        if queue.registered_cancellation_count_for_test() == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(queue.registered_cancellation_count_for_test(), 1);

    tokio::time::advance(Duration::from_millis(100)).await;
    for _ in 0..100 {
        let operations = deps.operations();
        if operations
            .iter()
            .filter(|operation| operation.starts_with("acquire:"))
            .count()
            >= 2
            && operations
                .iter()
                .any(|operation| operation == "executor-cancelled:lease-lost-job")
        {
            break;
        }
        tokio::task::yield_now().await;
    }
    let operations_before_shutdown = deps.operations();
    queue.shutdown().await;

    let second_acquire = operations_before_shutdown
        .iter()
        .enumerate()
        .filter(|(_, operation)| operation.starts_with("acquire:"))
        .nth(1)
        .map(|(index, _)| index)
        .expect("lease loss must return to passive acquisition");
    let child_cleanup = operations_before_shutdown
        .iter()
        .position(|operation| operation == "executor-cancelled:lease-lost-job")
        .expect("lease loss must cancel and join the active child");
    assert!(child_cleanup < second_acquire);
    assert_eq!(queue.registered_cancellation_count_for_test(), 0);
}

#[tokio::test(start_paused = true)]
async fn claim_crossing_known_expiry_never_starts_cancel_read_or_executor() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["expired-after-claim"]));
    deps.advance_clock_after_claim(11_000);
    let queue = GenerationJobQueue::new(
        "claim-expiry-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(1),
            Duration::from_millis(100),
            Duration::from_secs(5),
        )
        .expect("valid claim-expiry config"),
    )
    .expect("valid claim-expiry queue");
    queue
        .start(deps.clone())
        .await
        .expect("start claim-expiry worker");
    for _ in 0..100 {
        if deps
            .operations()
            .iter()
            .filter(|operation| operation.starts_with("acquire:"))
            .count()
            >= 2
        {
            break;
        }
        tokio::task::yield_now().await;
    }
    let operations = deps.operations();
    queue.shutdown().await;

    assert!(
        operations
            .iter()
            .filter(|operation| operation.starts_with("acquire:"))
            .count()
            >= 2
    );
    assert!(!operations.iter().any(
        |operation| operation.contains("cancel-read:claim-expiry-worker:7:expired-after-claim")
    ));
    assert!(!operations
        .iter()
        .any(|operation| operation == "execute:claim-expiry-worker:7:expired-after-claim"));
}

#[tokio::test(start_paused = true)]
async fn cancel_reread_crossing_known_expiry_never_starts_executor() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["expired-after-reread"]));
    deps.advance_clock_after_cancel_read(11_000);
    let queue = GenerationJobQueue::new(
        "reread-expiry-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(1),
            Duration::from_millis(100),
            Duration::from_secs(5),
        )
        .expect("valid reread-expiry config"),
    )
    .expect("valid reread-expiry queue");
    queue
        .start(deps.clone())
        .await
        .expect("start reread-expiry worker");
    for _ in 0..100 {
        if deps
            .operations()
            .iter()
            .filter(|operation| operation.starts_with("acquire:"))
            .count()
            >= 2
        {
            break;
        }
        tokio::task::yield_now().await;
    }
    let operations = deps.operations();
    queue.shutdown().await;

    assert!(operations
        .iter()
        .any(|operation| operation
            .contains("cancel-read:reread-expiry-worker:7:expired-after-reread")));
    assert!(!operations
        .iter()
        .any(|operation| operation == "execute:reread-expiry-worker:7:expired-after-reread"));
}

#[tokio::test]
async fn committed_claim_event_is_emitted_before_execution_and_sink_error_is_non_fatal() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["event-first", "event-after"]));
    deps.fail_next_event_emit();
    let queue = GenerationJobQueue::new(
        "event-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(2),
            Duration::from_millis(200),
            Duration::from_millis(50),
        )
        .expect("valid event config"),
    )
    .expect("valid event queue");
    queue.start(deps.clone()).await.expect("start event worker");
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if deps
                .operations()
                .iter()
                .any(|operation| operation == "execute:event-worker:7:event-after")
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("event sink error must not stop draining");
    queue.shutdown().await;

    let operations = deps.operations();
    for job_id in ["event-first", "event-after"] {
        let event = operations
            .iter()
            .position(|operation| operation == &format!("event:{job_id}:Running:Preparing"))
            .expect("committed event emitted");
        let execution = operations
            .iter()
            .position(|operation| operation == &format!("execute:event-worker:7:{job_id}"))
            .expect("job executed");
        assert!(event < execution);
    }
    assert!(operations
        .iter()
        .any(|operation| operation == "diagnostic:EmitCommittedEvent"));
}

#[tokio::test(start_paused = true)]
async fn short_busy_jobs_share_session_cadence_and_cannot_starve_renewal() {
    let deps = Arc::new(ScriptedDeps::with_jobs([
        "short-1", "short-2", "short-3", "short-4",
    ]));
    deps.delay_each_execution(Duration::from_millis(40));
    let queue = GenerationJobQueue::new(
        "busy-cadence-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(1),
            Duration::from_millis(100),
            Duration::from_secs(5),
        )
        .expect("valid busy cadence config"),
    )
    .expect("valid busy cadence queue");
    queue
        .start(deps.clone())
        .await
        .expect("start busy cadence worker");
    for _ in 0..100 {
        if deps
            .operations()
            .iter()
            .any(|operation| operation == "execute:busy-cadence-worker:7:short-1")
        {
            break;
        }
        tokio::task::yield_now().await;
    }

    for completed_target in 1..=4 {
        tokio::time::advance(Duration::from_millis(40)).await;
        for _ in 0..100 {
            if deps
                .operations()
                .iter()
                .filter(|operation| operation.starts_with("execute-finished:"))
                .count()
                >= completed_target
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    }
    let operations = deps.operations();
    queue.shutdown().await;

    assert_eq!(
        operations
            .iter()
            .filter(|operation| operation.starts_with("execute-finished:"))
            .count(),
        4
    );
    assert!(operations
        .iter()
        .any(|operation| operation.starts_with("renew:busy-cadence-worker:7:")));
}

#[tokio::test]
async fn startup_events_precede_supervised_recovery_jobs_and_normal_claims() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["normal-after-recovery"]));
    deps.install_startup_reconciliation(
        ["recovery-one", "recovery-two"],
        ["recovery-one", "recovery-two"],
    );
    let queue = GenerationJobQueue::new(
        "recovery-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(2),
            Duration::from_millis(200),
            Duration::from_millis(50),
        )
        .expect("valid recovery config"),
    )
    .expect("valid recovery queue");
    queue
        .start(deps.clone())
        .await
        .expect("start recovery worker");
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if deps
                .operations()
                .iter()
                .any(|operation| operation == "execute:recovery-worker:7:normal-after-recovery")
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("startup recovery and normal drain must complete");
    queue.shutdown().await;

    let operations = deps.operations();
    let first_claim = operations
        .iter()
        .position(|operation| operation.starts_with("claim:"))
        .expect("normal claim recorded");
    let recovery_one_event = operations
        .iter()
        .position(|operation| operation == "event:recovery-one:Running:Preparing")
        .expect("first startup event emitted");
    let recovery_two_event = operations
        .iter()
        .position(|operation| operation == "event:recovery-two:Running:Preparing")
        .expect("second startup event emitted");
    let recovery_one_execution = operations
        .iter()
        .position(|operation| operation == "execute:recovery-worker:7:recovery-one")
        .expect("first recovery executed");
    let recovery_two_execution = operations
        .iter()
        .position(|operation| operation == "execute:recovery-worker:7:recovery-two")
        .expect("second recovery executed");
    assert!(recovery_one_event < recovery_one_execution);
    assert!(recovery_two_event < recovery_one_execution);
    assert!(recovery_one_execution < recovery_two_execution);
    assert!(recovery_two_execution < first_claim);
}

#[tokio::test(start_paused = true)]
async fn transient_renewal_failure_cancels_at_known_expiry_before_next_cadence() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["transient-expiry-job"]));
    deps.block_next_execution_until_cancelled();
    deps.fail_next_renew_transiently_and_advance_clock(10_950);
    let queue = GenerationJobQueue::new(
        "transient-expiry-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(1),
            Duration::from_millis(100),
            Duration::from_secs(5),
        )
        .expect("valid transient-expiry config"),
    )
    .expect("valid transient-expiry queue");
    queue
        .start(deps.clone())
        .await
        .expect("start transient-expiry worker");
    for _ in 0..100 {
        if queue.registered_cancellation_count_for_test() == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }

    tokio::time::advance(Duration::from_millis(100)).await;
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_millis(50)).await;
    for _ in 0..100 {
        if deps
            .operations()
            .iter()
            .filter(|operation| operation.starts_with("acquire:"))
            .count()
            >= 2
        {
            break;
        }
        tokio::task::yield_now().await;
    }
    let operations = deps.operations();
    queue.shutdown().await;

    let child_cleanup = operations
        .iter()
        .position(|operation| operation == "executor-cancelled:transient-expiry-job")
        .expect("known expiry cancels active child");
    let second_acquire = operations
        .iter()
        .enumerate()
        .filter(|(_, operation)| operation.starts_with("acquire:"))
        .nth(1)
        .map(|(index, _)| index)
        .expect("known expiry returns to acquisition");
    assert!(child_cleanup < second_acquire);
}

#[tokio::test(start_paused = true)]
async fn renewal_result_expired_by_await_completion_is_lease_lost() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["late-renew-result"]));
    deps.block_next_execution_until_cancelled();
    deps.advance_clock_during_next_successful_renew(11_200);
    let queue = GenerationJobQueue::new(
        "late-renew-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(1),
            Duration::from_millis(100),
            Duration::from_secs(5),
        )
        .expect("valid late-renew config"),
    )
    .expect("valid late-renew queue");
    queue
        .start(deps.clone())
        .await
        .expect("start late-renew worker");
    for _ in 0..100 {
        if queue.registered_cancellation_count_for_test() == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }

    tokio::time::advance(Duration::from_millis(100)).await;
    for _ in 0..100 {
        if deps
            .operations()
            .iter()
            .filter(|operation| operation.starts_with("acquire:"))
            .count()
            >= 2
        {
            break;
        }
        tokio::task::yield_now().await;
    }
    let operations = deps.operations();
    queue.shutdown().await;

    assert!(operations
        .iter()
        .any(|operation| operation == "executor-cancelled:late-renew-result"));
    assert!(
        operations
            .iter()
            .filter(|operation| operation.starts_with("acquire:"))
            .count()
            >= 2
    );
}

#[tokio::test]
async fn active_shutdown_cancels_and_joins_child_before_release_and_return() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["shutdown-active-job"]));
    deps.block_next_execution_until_cancelled();
    let queue = GenerationJobQueue::new(
        "shutdown-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(2),
            Duration::from_millis(200),
            Duration::from_millis(50),
        )
        .expect("valid shutdown config"),
    )
    .expect("valid shutdown queue");
    queue
        .start(deps.clone())
        .await
        .expect("start shutdown worker");
    for _ in 0..100 {
        if queue.registered_cancellation_count_for_test() == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(queue.registered_cancellation_count_for_test(), 1);

    let shutdown_queue = queue.clone();
    let shutdown_task = tokio::spawn(async move { shutdown_queue.shutdown().await });
    for _ in 0..100 {
        if shutdown_task.is_finished() {
            break;
        }
        tokio::task::yield_now().await;
    }
    let joined_without_external_cancel = shutdown_task.is_finished();
    if !joined_without_external_cancel {
        queue.cancel("shutdown-active-job");
    }
    shutdown_task
        .await
        .expect("shutdown coordination task must not panic");

    assert!(
        joined_without_external_cancel,
        "shutdown must signal active cancellation itself"
    );
    assert_eq!(queue.registered_cancellation_count_for_test(), 0);
    assert_eq!(
        queue.lifecycle_state_for_test(),
        QueueLifecycleSnapshot::Stopped
    );
    let operations = deps.operations();
    let child_cleanup = operations
        .iter()
        .position(|operation| operation == "executor-cancelled:shutdown-active-job")
        .expect("active executor acknowledged cancellation");
    let release = operations
        .iter()
        .position(|operation| operation == "release:shutdown-worker:7")
        .expect("lease released after child cleanup");
    assert!(child_cleanup < release);
    assert_eq!(
        operations.last().map(String::as_str),
        Some("release:shutdown-worker:7")
    );
}

#[tokio::test(start_paused = true)]
async fn fixed_wall_clock_cannot_extend_known_expiry_across_transient_renewals() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["fixed-clock-expiry"]));
    deps.block_next_execution_until_cancelled();
    deps.make_renewals_transient();
    let queue = GenerationJobQueue::new(
        "fixed-clock-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(1),
            Duration::from_millis(100),
            Duration::from_secs(5),
        )
        .expect("valid fixed-clock config"),
    )
    .expect("valid fixed-clock queue");
    queue
        .start(deps.clone())
        .await
        .expect("start fixed-clock worker");
    for _ in 0..100 {
        if queue.registered_cancellation_count_for_test() == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }

    for _ in 0..10 {
        tokio::time::advance(Duration::from_millis(100)).await;
        tokio::task::yield_now().await;
    }
    for _ in 0..100 {
        if deps
            .operations()
            .iter()
            .filter(|operation| operation.starts_with("acquire:"))
            .count()
            >= 2
        {
            break;
        }
        tokio::task::yield_now().await;
    }
    let reacquired = deps
        .operations()
        .iter()
        .filter(|operation| operation.starts_with("acquire:"))
        .count()
        >= 2;
    if !reacquired {
        queue.cancel("fixed-clock-expiry");
    }
    queue.shutdown().await;

    assert!(
        reacquired,
        "transient renewals must not move the known lease deadline"
    );
}

#[tokio::test(start_paused = true)]
async fn every_lease_lost_seam_exits_stale_authority_before_reacquiring() {
    for point in [
        LeaseLossPoint::Reconcile,
        LeaseLossPoint::Claim,
        LeaseLossPoint::CancelRead,
        LeaseLossPoint::ExecutionOutcome,
        LeaseLossPoint::Heartbeat,
    ] {
        let (owner_id, job_id) = match point {
            LeaseLossPoint::Reconcile => ("lost-reconcile-worker", "lost-reconcile-job"),
            LeaseLossPoint::Claim => ("lost-claim-worker", "lost-claim-job"),
            LeaseLossPoint::CancelRead => ("lost-reread-worker", "lost-reread-job"),
            LeaseLossPoint::ExecutionOutcome => ("lost-outcome-worker", "lost-outcome-job"),
            LeaseLossPoint::Heartbeat => ("lost-heartbeat-worker", "lost-heartbeat-job"),
        };
        let deps = Arc::new(ScriptedDeps::with_jobs([job_id]));
        deps.lose_lease_at(point);
        if matches!(point, LeaseLossPoint::Heartbeat) {
            deps.block_next_execution_until_cancelled();
        }
        let queue = GenerationJobQueue::new(
            owner_id,
            GenerationJobWorkerConfig::new(
                Duration::from_secs(1),
                Duration::from_millis(100),
                Duration::from_secs(5),
            )
            .expect("valid seam config"),
        )
        .expect("valid seam queue");
        queue.start(deps.clone()).await.expect("start seam worker");
        if matches!(point, LeaseLossPoint::Heartbeat) {
            for _ in 0..100 {
                if queue.registered_cancellation_count_for_test() == 1 {
                    break;
                }
                tokio::task::yield_now().await;
            }
            tokio::time::advance(Duration::from_millis(100)).await;
        }
        for _ in 0..100 {
            if deps
                .operations()
                .iter()
                .filter(|operation| operation.starts_with("acquire:"))
                .count()
                >= 2
            {
                break;
            }
            tokio::task::yield_now().await;
        }
        let before_shutdown = deps.operations();
        let second_acquire = before_shutdown
            .iter()
            .enumerate()
            .filter(|(_, operation)| operation.starts_with("acquire:"))
            .nth(1)
            .map(|(index, _)| index);
        if second_acquire.is_none() {
            queue.cancel(job_id);
        }
        queue.shutdown().await;

        let second_acquire = second_acquire
            .unwrap_or_else(|| panic!("{point:?} lease loss must return to passive acquisition"));
        assert!(
            !before_shutdown[..second_acquire]
                .iter()
                .any(|operation| operation.starts_with("release:")),
            "{point:?} must not release a stale authority"
        );
        if matches!(point, LeaseLossPoint::Heartbeat) {
            let child_cleanup = before_shutdown
                .iter()
                .position(|operation| operation == "executor-cancelled:lost-heartbeat-job")
                .expect("heartbeat lease loss cancels child");
            assert!(child_cleanup < second_acquire);
        }
    }
}

#[tokio::test]
async fn shutdown_during_claim_await_never_starts_the_returned_job() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["claimed-after-shutdown"]));
    deps.block_claim_until_resumed();
    let queue = GenerationJobQueue::new(
        "shutdown-claim-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(2),
            Duration::from_millis(200),
            Duration::from_millis(50),
        )
        .expect("valid shutdown-claim config"),
    )
    .expect("valid shutdown-claim queue");
    queue
        .start(deps.clone())
        .await
        .expect("start shutdown-claim worker");
    tokio::time::timeout(Duration::from_secs(1), async {
        while !deps
            .operations()
            .iter()
            .any(|operation| operation == "claim-await:claimed-after-shutdown")
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("claim must reach its await barrier");

    let shutdown_queue = queue.clone();
    let shutdown = tokio::spawn(async move { shutdown_queue.shutdown().await });
    tokio::time::timeout(Duration::from_secs(1), async {
        while queue.lifecycle_state_for_test() != QueueLifecycleSnapshot::Stopping {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("shutdown must be published before claim resumes");
    deps.resume_claim();
    tokio::time::timeout(Duration::from_secs(1), shutdown)
        .await
        .expect("shutdown must join after claim returns")
        .expect("shutdown task must not panic");

    assert!(!deps
        .operations()
        .iter()
        .any(|operation| operation == "execute:shutdown-claim-worker:7:claimed-after-shutdown"));
}

#[tokio::test]
async fn shutdown_during_cancel_reread_await_never_starts_executor() {
    let deps = Arc::new(ScriptedDeps::with_jobs(["reread-after-shutdown"]));
    deps.block_cancel_read_until_resumed();
    let queue = GenerationJobQueue::new(
        "shutdown-reread-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(2),
            Duration::from_millis(200),
            Duration::from_millis(50),
        )
        .expect("valid shutdown-reread config"),
    )
    .expect("valid shutdown-reread queue");
    queue
        .start(deps.clone())
        .await
        .expect("start shutdown-reread worker");
    tokio::time::timeout(Duration::from_secs(1), async {
        while !deps
            .operations()
            .iter()
            .any(|operation| operation == "cancel-read-await:reread-after-shutdown")
        {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancel reread must reach its await barrier");

    let shutdown_queue = queue.clone();
    let shutdown = tokio::spawn(async move { shutdown_queue.shutdown().await });
    tokio::time::timeout(Duration::from_secs(1), async {
        while queue.lifecycle_state_for_test() != QueueLifecycleSnapshot::Stopping {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("shutdown must be published before reread resumes");
    deps.resume_cancel_read();
    tokio::time::timeout(Duration::from_secs(1), shutdown)
        .await
        .expect("shutdown must join after reread returns")
        .expect("shutdown task must not panic");

    assert!(!deps
        .operations()
        .iter()
        .any(|operation| operation == "execute:shutdown-reread-worker:7:reread-after-shutdown"));
}

#[tokio::test(start_paused = true)]
async fn held_wait_uses_fresh_time_after_acquire_await_returns() {
    let deps = Arc::new(PassiveDeps::hold_for_and_advance_clock(
        Duration::from_millis(100),
        1_090,
    ));
    let queue = GenerationJobQueue::new(
        "held-fresh-time-worker",
        GenerationJobWorkerConfig::new(
            Duration::from_secs(20),
            Duration::from_secs(1),
            Duration::from_secs(5),
        )
        .expect("valid held fresh-time config"),
    )
    .expect("valid held fresh-time queue");
    queue
        .start(deps.clone())
        .await
        .expect("start held fresh-time worker");
    for _ in 0..100 {
        if deps.acquire_calls.load(Ordering::SeqCst) >= 1 {
            break;
        }
        tokio::task::yield_now().await;
    }

    tokio::time::advance(Duration::from_millis(10)).await;
    for _ in 0..100 {
        if deps.acquire_calls.load(Ordering::SeqCst) >= 2 {
            break;
        }
        tokio::task::yield_now().await;
    }
    let calls = deps.acquire_calls.load(Ordering::SeqCst);
    queue.shutdown().await;

    assert!(
        calls >= 2,
        "Held expiry wait must use time sampled after acquire await"
    );
}
