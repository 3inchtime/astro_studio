use crate::api_gateway::{EngineCallError, RetryAfterHint};
use crate::generation_worker_lease::{WorkerLeaseAcquireOutcome, WorkerTransitionAuthority};
use crate::models::{GenerationJobEvent, GenerationJobStatus};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::{watch, Mutex, Notify};
use tokio::task::JoinHandle;

const MAX_WORKER_OWNER_ID_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WorkerCoreErrorKind {
    LeaseLost,
    Transient,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{message}")]
pub(crate) struct WorkerCoreError {
    pub(crate) kind: WorkerCoreErrorKind,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ClaimedGenerationJob {
    execution_job: WorkerExecutionJob,
    committed_event: GenerationJobEvent,
}

impl ClaimedGenerationJob {
    pub(crate) fn from_committed_event(committed_event: GenerationJobEvent) -> Self {
        Self {
            execution_job: WorkerExecutionJob::new(committed_event.job_id.clone()),
            committed_event,
        }
    }

    pub(crate) fn job_id(&self) -> &str {
        self.execution_job.job_id()
    }

    pub(crate) fn committed_event(&self) -> &GenerationJobEvent {
        &self.committed_event
    }

    fn into_execution_job(self) -> WorkerExecutionJob {
        self.execution_job
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkerExecutionJob {
    job_id: String,
}

impl WorkerExecutionJob {
    pub(crate) fn new(job_id: impl Into<String>) -> Self {
        Self {
            job_id: job_id.into(),
        }
    }

    pub(crate) fn job_id(&self) -> &str {
        &self.job_id
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StartupReconciliation {
    events: Vec<GenerationJobEvent>,
    recovery_jobs: Vec<WorkerExecutionJob>,
}

impl StartupReconciliation {
    pub(crate) fn new(
        events: Vec<GenerationJobEvent>,
        recovery_jobs: Vec<WorkerExecutionJob>,
    ) -> Self {
        Self {
            events,
            recovery_jobs,
        }
    }

    pub(crate) fn empty() -> Self {
        Self::new(Vec::new(), Vec::new())
    }

    fn into_parts(self) -> (Vec<GenerationJobEvent>, Vec<WorkerExecutionJob>) {
        (self.events, self.recovery_jobs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkerExecutionOutcome {
    DurablyFinished,
    NeedsReconciliation,
    LeaseLost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkerDiagnosticKind {
    AcquireLease,
    ReconcileStartup,
    ClaimNext,
    EmitCommittedEvent,
    ReadCancellation,
    ExecuteJob,
    ExecutePanic,
    CancellationRegistryInvariant,
    RenewLease,
    HeartbeatJob,
    ReleaseLease,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkerDiagnostic {
    pub(crate) kind: WorkerDiagnosticKind,
    pub(crate) job_id: Option<String>,
    pub(crate) message: String,
}

impl WorkerDiagnostic {
    fn from_error(
        kind: WorkerDiagnosticKind,
        job_id: Option<&str>,
        error: &WorkerCoreError,
    ) -> Self {
        Self {
            kind,
            job_id: job_id.map(str::to_owned),
            message: error.message.clone(),
        }
    }
}

#[async_trait]
pub(crate) trait GenerationJobWorkerDeps: Send + Sync + 'static {
    fn now_ms(&self) -> i64;

    async fn acquire_lease(
        &self,
        owner_id: &str,
        now_ms: i64,
        ttl: Duration,
    ) -> Result<WorkerLeaseAcquireOutcome, WorkerCoreError>;

    async fn reconcile_startup(
        &self,
        authority: &WorkerTransitionAuthority,
        now_ms: i64,
    ) -> Result<StartupReconciliation, WorkerCoreError>;

    async fn claim_next(
        &self,
        authority: &WorkerTransitionAuthority,
        now_ms: i64,
    ) -> Result<Option<ClaimedGenerationJob>, WorkerCoreError>;

    fn emit_committed_event(&self, event: GenerationJobEvent) -> Result<(), WorkerCoreError>;

    async fn reread_cancel_requested(
        &self,
        authority: &WorkerTransitionAuthority,
        job_id: &str,
    ) -> Result<bool, WorkerCoreError>;

    /// Executes one supervised job and reports only its durable execution disposition.
    ///
    /// Except for startup reconciliation events returned by `reconcile_startup`, the
    /// implementation must emit every committed stage, retry, cancellation-acknowledgement,
    /// and terminal event through `emit_committed_event` only after the database commit has
    /// completed and its lock has been released. Event sink failures are diagnostic-only and
    /// must not change the returned execution disposition.
    async fn execute_job(
        &self,
        authority: &WorkerTransitionAuthority,
        job: WorkerExecutionJob,
        cancellation: watch::Receiver<bool>,
    ) -> Result<WorkerExecutionOutcome, WorkerCoreError>;

    async fn renew_lease(
        &self,
        authority: &WorkerTransitionAuthority,
        now_ms: i64,
        ttl: Duration,
    ) -> Result<i64, WorkerCoreError>;

    async fn heartbeat_job(
        &self,
        authority: &WorkerTransitionAuthority,
        job_id: &str,
        now_ms: i64,
    ) -> Result<(), WorkerCoreError>;

    async fn release_lease(
        &self,
        authority: &WorkerTransitionAuthority,
    ) -> Result<(), WorkerCoreError>;

    fn record_diagnostic(&self, diagnostic: WorkerDiagnostic);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GenerationJobWorkerConfig {
    lease_ttl: Duration,
    bounded_poll_interval: Duration,
    heartbeat_interval: Duration,
}

impl GenerationJobWorkerConfig {
    pub(crate) fn new(
        lease_ttl: Duration,
        heartbeat_interval: Duration,
        bounded_poll_interval: Duration,
    ) -> Result<Self, GenerationJobWorkerConfigError> {
        if lease_ttl.is_zero()
            || heartbeat_interval.is_zero()
            || bounded_poll_interval.is_zero()
            || heartbeat_interval >= lease_ttl
            || lease_ttl.as_millis() < 1
            || !lease_ttl.subsec_nanos().is_multiple_of(1_000_000)
            || lease_ttl.as_millis() > i64::MAX as u128
            || heartbeat_interval.as_millis() > i64::MAX as u128
            || bounded_poll_interval.as_millis() > i64::MAX as u128
            || tokio::time::Instant::now()
                .checked_add(heartbeat_interval)
                .is_none()
            || tokio::time::Instant::now()
                .checked_add(bounded_poll_interval)
                .is_none()
        {
            return Err(GenerationJobWorkerConfigError::InvalidInterval);
        }
        Ok(Self {
            lease_ttl,
            bounded_poll_interval,
            heartbeat_interval,
        })
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
pub(crate) enum GenerationJobWorkerConfigError {
    #[error("generation worker intervals must be positive")]
    InvalidInterval,
}

#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
pub(crate) enum GenerationJobQueueStartError {
    #[error("generation job queue has already started")]
    AlreadyStarted,
    #[error("generation job queue owner ID is invalid")]
    InvalidOwner,
    #[error("generation job queue has already shut down")]
    Shutdown,
}

#[derive(Clone)]
pub(crate) struct GenerationJobQueue {
    inner: Arc<GenerationJobQueueInner>,
}

struct GenerationJobQueueInner {
    wake: Notify,
    shutdown: watch::Sender<bool>,
    lifecycle: StdMutex<QueueLifecycle>,
    shutdown_gate: Mutex<()>,
    cancellations: StdMutex<HashMap<String, watch::Sender<bool>>>,
    owner_id: String,
    config: GenerationJobWorkerConfig,
}

enum QueueLifecycle {
    Created,
    Running(JoinHandle<()>),
    Stopping,
    Stopped,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QueueLifecycleSnapshot {
    Created,
    Running,
    Stopping,
    Stopped,
}

impl GenerationJobQueue {
    pub(crate) fn new(
        owner_id: impl Into<String>,
        config: GenerationJobWorkerConfig,
    ) -> Result<Self, GenerationJobQueueStartError> {
        let owner_id = owner_id.into();
        if !worker_owner_id_is_canonical(&owner_id) {
            return Err(GenerationJobQueueStartError::InvalidOwner);
        }
        let (shutdown, _) = watch::channel(false);
        Ok(Self {
            inner: Arc::new(GenerationJobQueueInner {
                wake: Notify::new(),
                shutdown,
                lifecycle: StdMutex::new(QueueLifecycle::Created),
                shutdown_gate: Mutex::new(()),
                cancellations: StdMutex::new(HashMap::new()),
                owner_id,
                config,
            }),
        })
    }

    pub(crate) async fn start(
        &self,
        deps: Arc<dyn GenerationJobWorkerDeps>,
    ) -> Result<(), GenerationJobQueueStartError> {
        let mut lifecycle = self
            .inner
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match &*lifecycle {
            QueueLifecycle::Created if !*self.inner.shutdown.borrow() => {
                let inner = Arc::clone(&self.inner);
                let mut shutdown = inner.shutdown.subscribe();
                let task = tokio::spawn(async move {
                    generation_worker_loop(Arc::clone(&inner), deps, &mut shutdown).await;
                });
                *lifecycle = QueueLifecycle::Running(task);
                Ok(())
            }
            QueueLifecycle::Created | QueueLifecycle::Stopped => {
                Err(GenerationJobQueueStartError::Shutdown)
            }
            QueueLifecycle::Running(_) | QueueLifecycle::Stopping => {
                Err(GenerationJobQueueStartError::AlreadyStarted)
            }
        }
    }

    pub(crate) fn wake(&self) {
        self.inner.wake.notify_one();
    }

    pub(crate) fn cancel(&self, job_id: &str) -> bool {
        let cancellation = self
            .inner
            .cancellations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(job_id)
            .cloned();
        let Some(cancellation) = cancellation else {
            return false;
        };
        cancellation.send_replace(true);
        true
    }

    pub(crate) async fn shutdown(&self) {
        let _shutdown_guard = self.inner.shutdown_gate.lock().await;
        self.inner.shutdown.send_replace(true);
        self.inner.wake.notify_waiters();
        let task = {
            let mut lifecycle = self
                .inner
                .lifecycle
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            match std::mem::replace(&mut *lifecycle, QueueLifecycle::Stopping) {
                QueueLifecycle::Created => {
                    *lifecycle = QueueLifecycle::Stopped;
                    None
                }
                QueueLifecycle::Running(task) => Some(task),
                QueueLifecycle::Stopping => None,
                QueueLifecycle::Stopped => {
                    *lifecycle = QueueLifecycle::Stopped;
                    None
                }
            }
        };
        if let Some(task) = task {
            let _ = task.await;
        }
        *self
            .inner
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = QueueLifecycle::Stopped;
    }

    #[cfg(test)]
    pub(crate) fn lifecycle_state_for_test(&self) -> QueueLifecycleSnapshot {
        match &*self
            .inner
            .lifecycle
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
        {
            QueueLifecycle::Created => QueueLifecycleSnapshot::Created,
            QueueLifecycle::Running(_) => QueueLifecycleSnapshot::Running,
            QueueLifecycle::Stopping => QueueLifecycleSnapshot::Stopping,
            QueueLifecycle::Stopped => QueueLifecycleSnapshot::Stopped,
        }
    }

    #[cfg(test)]
    pub(crate) fn registered_cancellation_count_for_test(&self) -> usize {
        self.inner
            .cancellations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }
}

fn worker_owner_id_is_canonical(owner_id: &str) -> bool {
    !owner_id.is_empty()
        && owner_id.len() <= MAX_WORKER_OWNER_ID_BYTES
        && owner_id.trim() == owner_id
        && owner_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

struct CancellationRegistration<'a> {
    cancellations: &'a StdMutex<HashMap<String, watch::Sender<bool>>>,
    job_id: String,
    sender: watch::Sender<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CancellationRegistrationError {
    DuplicateJobId,
}

impl<'a> CancellationRegistration<'a> {
    fn insert(
        cancellations: &'a StdMutex<HashMap<String, watch::Sender<bool>>>,
        job_id: String,
        sender: watch::Sender<bool>,
    ) -> Result<Self, CancellationRegistrationError> {
        let mut registry = cancellations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if registry.contains_key(&job_id) {
            return Err(CancellationRegistrationError::DuplicateJobId);
        }
        registry.insert(job_id.clone(), sender.clone());
        drop(registry);
        Ok(Self {
            cancellations,
            job_id,
            sender,
        })
    }
}

impl Drop for CancellationRegistration<'_> {
    fn drop(&mut self) {
        let mut cancellations = self
            .cancellations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if cancellations
            .get(&self.job_id)
            .is_some_and(|sender| sender.same_channel(&self.sender))
        {
            cancellations.remove(&self.job_id);
        }
    }
}

async fn generation_worker_loop(
    inner: Arc<GenerationJobQueueInner>,
    deps: Arc<dyn GenerationJobWorkerDeps>,
    shutdown: &mut watch::Receiver<bool>,
) {
    loop {
        if *shutdown.borrow() {
            return;
        }
        let now_ms = deps.now_ms();
        let acquired = deps
            .acquire_lease(&inner.owner_id, now_ms, inner.config.lease_ttl)
            .await;
        let mut passive_wait = inner.config.bounded_poll_interval;
        match acquired {
            Ok(WorkerLeaseAcquireOutcome::Acquired { authority, expires }) => {
                if now_ms < 0 || expires <= now_ms {
                    deps.record_diagnostic(WorkerDiagnostic {
                        kind: WorkerDiagnosticKind::AcquireLease,
                        job_id: None,
                        message: "acquired generation worker lease is already expired".to_owned(),
                    });
                } else {
                    match run_acquired_session(
                        &inner,
                        Arc::clone(&deps),
                        &authority,
                        expires,
                        shutdown,
                    )
                    .await
                    {
                        SessionOutcome::Shutdown => return,
                        SessionOutcome::LeaseLost | SessionOutcome::NeedsReconciliation => continue,
                        SessionOutcome::Released => {}
                    }
                }
            }
            Ok(WorkerLeaseAcquireOutcome::Held { expires }) => {
                passive_wait = wait_until_expiry_or_poll(deps.now_ms(), expires, passive_wait);
                if passive_wait.is_zero() {
                    continue;
                }
            }
            Err(error) => deps.record_diagnostic(WorkerDiagnostic::from_error(
                WorkerDiagnosticKind::AcquireLease,
                None,
                &error,
            )),
        }
        tokio::select! {
            _ = inner.wake.notified() => {}
            _ = tokio::time::sleep(passive_wait) => {}
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return;
                }
            }
        }
    }
}

fn wait_until_expiry_or_poll(now_ms: i64, expires: i64, poll: Duration) -> Duration {
    if now_ms < 0 || expires <= now_ms {
        return Duration::ZERO;
    }
    let until_expiry = u64::try_from(expires - now_ms)
        .map(Duration::from_millis)
        .unwrap_or(Duration::ZERO);
    poll.min(until_expiry)
}

#[derive(Debug, Clone, Copy)]
struct KnownLeaseExpiry {
    wall_ms: i64,
    deadline: tokio::time::Instant,
}

impl KnownLeaseExpiry {
    fn from_wall_clock(now_ms: i64, expires: i64, max_ttl: Duration) -> Option<Self> {
        let remaining = lease_expiry_wait(now_ms, expires).min(max_ttl);
        if remaining.is_zero() {
            return None;
        }
        Some(Self {
            wall_ms: expires,
            deadline: tokio::time::Instant::now().checked_add(remaining)?,
        })
    }

    fn is_expired(self, now_ms: i64) -> bool {
        now_ms < 0 || now_ms >= self.wall_ms || tokio::time::Instant::now() >= self.deadline
    }

    fn deadline(self, now_ms: i64) -> tokio::time::Instant {
        let wall_remaining = lease_expiry_wait(now_ms, self.wall_ms);
        let wall_deadline = tokio::time::Instant::now()
            .checked_add(wall_remaining)
            .unwrap_or_else(tokio::time::Instant::now);
        self.deadline.min(wall_deadline)
    }

    fn replace_from_wall_clock(&mut self, now_ms: i64, expires: i64, max_ttl: Duration) -> bool {
        let Some(updated) = Self::from_wall_clock(now_ms, expires, max_ttl) else {
            return false;
        };
        *self = updated;
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionOutcome {
    Shutdown,
    LeaseLost,
    NeedsReconciliation,
    Released,
}

async fn run_acquired_session(
    inner: &GenerationJobQueueInner,
    deps: Arc<dyn GenerationJobWorkerDeps>,
    authority: &WorkerTransitionAuthority,
    initial_expires: i64,
    shutdown: &mut watch::Receiver<bool>,
) -> SessionOutcome {
    let Some(mut known_expiry) =
        KnownLeaseExpiry::from_wall_clock(deps.now_ms(), initial_expires, inner.config.lease_ttl)
    else {
        record_expired_lease(deps.as_ref(), None);
        return SessionOutcome::LeaseLost;
    };
    let first_renewal = tokio::time::Instant::now() + inner.config.heartbeat_interval;
    let mut next_renewal = first_renewal;
    let mut renew_interval =
        tokio::time::interval_at(first_renewal, inner.config.heartbeat_interval);
    renew_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let reconciliation = match deps.reconcile_startup(authority, deps.now_ms()).await {
        Ok(reconciliation) => reconciliation,
        Err(error) => {
            let lease_lost = error.kind == WorkerCoreErrorKind::LeaseLost;
            deps.record_diagnostic(WorkerDiagnostic::from_error(
                WorkerDiagnosticKind::ReconcileStartup,
                None,
                &error,
            ));
            if lease_lost {
                return SessionOutcome::LeaseLost;
            }
            release_session(deps.as_ref(), authority).await;
            return SessionOutcome::Released;
        }
    };
    let (events, recovery_jobs) = reconciliation.into_parts();
    for event in events {
        let job_id = event.job_id.clone();
        emit_committed_event(deps.as_ref(), &job_id, event);
    }
    let mut recovery_jobs = VecDeque::from(recovery_jobs);

    loop {
        if *shutdown.borrow() {
            release_session(deps.as_ref(), authority).await;
            return SessionOutcome::Shutdown;
        }
        if known_expiry.is_expired(deps.now_ms()) {
            record_expired_lease(deps.as_ref(), None);
            return SessionOutcome::LeaseLost;
        }
        if tokio::time::Instant::now() >= next_renewal {
            renew_interval.tick().await;
            schedule_next_renewal(&mut next_renewal, inner.config.heartbeat_interval);
            if renew_for_session(
                deps.as_ref(),
                authority,
                inner.config.lease_ttl,
                &mut known_expiry,
                None,
            )
            .await
                == RenewSessionOutcome::LeaseLost
            {
                return SessionOutcome::LeaseLost;
            }
        }

        if let Some(job) = recovery_jobs.pop_front() {
            match execute_claimed_job(
                inner,
                Arc::clone(&deps),
                authority,
                job,
                inner.config,
                &mut known_expiry,
                &mut renew_interval,
                &mut next_renewal,
                shutdown,
            )
            .await
            {
                ExecuteClaimedOutcome::Completed => continue,
                ExecuteClaimedOutcome::NeedsReconciliation => {
                    release_session(deps.as_ref(), authority).await;
                    return SessionOutcome::NeedsReconciliation;
                }
                ExecuteClaimedOutcome::Shutdown => {
                    release_session(deps.as_ref(), authority).await;
                    return SessionOutcome::Shutdown;
                }
                ExecuteClaimedOutcome::LeaseLost => return SessionOutcome::LeaseLost,
            }
        }

        match deps.claim_next(authority, deps.now_ms()).await {
            Ok(Some(job)) => {
                emit_claimed_event(deps.as_ref(), &job);
                let job = job.into_execution_job();
                match execute_claimed_job(
                    inner,
                    Arc::clone(&deps),
                    authority,
                    job,
                    inner.config,
                    &mut known_expiry,
                    &mut renew_interval,
                    &mut next_renewal,
                    shutdown,
                )
                .await
                {
                    ExecuteClaimedOutcome::Completed => {}
                    ExecuteClaimedOutcome::NeedsReconciliation => {
                        release_session(deps.as_ref(), authority).await;
                        return SessionOutcome::NeedsReconciliation;
                    }
                    ExecuteClaimedOutcome::Shutdown => {
                        release_session(deps.as_ref(), authority).await;
                        return SessionOutcome::Shutdown;
                    }
                    ExecuteClaimedOutcome::LeaseLost => return SessionOutcome::LeaseLost,
                }
            }
            Ok(None) => {
                let notified = inner.wake.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();

                match deps.claim_next(authority, deps.now_ms()).await {
                    Ok(Some(job)) => {
                        emit_claimed_event(deps.as_ref(), &job);
                        let job = job.into_execution_job();
                        match execute_claimed_job(
                            inner,
                            Arc::clone(&deps),
                            authority,
                            job,
                            inner.config,
                            &mut known_expiry,
                            &mut renew_interval,
                            &mut next_renewal,
                            shutdown,
                        )
                        .await
                        {
                            ExecuteClaimedOutcome::Completed => {}
                            ExecuteClaimedOutcome::NeedsReconciliation => {
                                release_session(deps.as_ref(), authority).await;
                                return SessionOutcome::NeedsReconciliation;
                            }
                            ExecuteClaimedOutcome::Shutdown => {
                                release_session(deps.as_ref(), authority).await;
                                return SessionOutcome::Shutdown;
                            }
                            ExecuteClaimedOutcome::LeaseLost => return SessionOutcome::LeaseLost,
                        }
                    }
                    Ok(None) => {
                        let expiry_deadline = known_expiry.deadline(deps.now_ms());
                        tokio::select! {
                            _ = &mut notified => {}
                            _ = tokio::time::sleep(inner.config.bounded_poll_interval) => {}
                            _ = tokio::time::sleep_until(expiry_deadline) => {
                                record_expired_lease(deps.as_ref(), None);
                                return SessionOutcome::LeaseLost;
                            }
                            _ = renew_interval.tick() => {
                                schedule_next_renewal(&mut next_renewal, inner.config.heartbeat_interval);
                                if renew_for_session(
                                    deps.as_ref(),
                                    authority,
                                    inner.config.lease_ttl,
                                    &mut known_expiry,
                                    None,
                                ).await == RenewSessionOutcome::LeaseLost {
                                    return SessionOutcome::LeaseLost;
                                }
                            }
                            changed = shutdown.changed() => {
                                if changed.is_err() || *shutdown.borrow() {
                                    release_session(deps.as_ref(), authority).await;
                                    return SessionOutcome::Shutdown;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        let lease_lost = error.kind == WorkerCoreErrorKind::LeaseLost;
                        deps.record_diagnostic(WorkerDiagnostic::from_error(
                            WorkerDiagnosticKind::ClaimNext,
                            None,
                            &error,
                        ));
                        if lease_lost {
                            return SessionOutcome::LeaseLost;
                        }
                        let expiry_deadline = known_expiry.deadline(deps.now_ms());
                        tokio::select! {
                            _ = inner.wake.notified() => {}
                            _ = tokio::time::sleep(inner.config.bounded_poll_interval) => {}
                            _ = tokio::time::sleep_until(expiry_deadline) => {
                                record_expired_lease(deps.as_ref(), None);
                                return SessionOutcome::LeaseLost;
                            }
                            _ = renew_interval.tick() => {
                                schedule_next_renewal(&mut next_renewal, inner.config.heartbeat_interval);
                                if renew_for_session(
                                    deps.as_ref(),
                                    authority,
                                    inner.config.lease_ttl,
                                    &mut known_expiry,
                                    None,
                                ).await == RenewSessionOutcome::LeaseLost {
                                    return SessionOutcome::LeaseLost;
                                }
                            }
                            changed = shutdown.changed() => {
                                if changed.is_err() || *shutdown.borrow() {
                                    release_session(deps.as_ref(), authority).await;
                                    return SessionOutcome::Shutdown;
                                }
                            }
                        }
                    }
                }
            }
            Err(error) => {
                let lease_lost = error.kind == WorkerCoreErrorKind::LeaseLost;
                deps.record_diagnostic(WorkerDiagnostic::from_error(
                    WorkerDiagnosticKind::ClaimNext,
                    None,
                    &error,
                ));
                if lease_lost {
                    return SessionOutcome::LeaseLost;
                }
                let expiry_deadline = known_expiry.deadline(deps.now_ms());
                tokio::select! {
                    _ = inner.wake.notified() => {}
                    _ = tokio::time::sleep(inner.config.bounded_poll_interval) => {}
                    _ = tokio::time::sleep_until(expiry_deadline) => {
                        record_expired_lease(deps.as_ref(), None);
                        return SessionOutcome::LeaseLost;
                    }
                    _ = renew_interval.tick() => {
                        schedule_next_renewal(&mut next_renewal, inner.config.heartbeat_interval);
                        if renew_for_session(
                            deps.as_ref(),
                            authority,
                            inner.config.lease_ttl,
                            &mut known_expiry,
                            None,
                        ).await == RenewSessionOutcome::LeaseLost {
                            return SessionOutcome::LeaseLost;
                        }
                    }
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            release_session(deps.as_ref(), authority).await;
                            return SessionOutcome::Shutdown;
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenewSessionOutcome {
    Renewed,
    TransientFailure,
    LeaseLost,
}

fn emit_committed_event(
    deps: &dyn GenerationJobWorkerDeps,
    job_id: &str,
    event: GenerationJobEvent,
) {
    if let Err(error) = deps.emit_committed_event(event) {
        deps.record_diagnostic(WorkerDiagnostic::from_error(
            WorkerDiagnosticKind::EmitCommittedEvent,
            Some(job_id),
            &error,
        ));
    }
}

fn emit_claimed_event(deps: &dyn GenerationJobWorkerDeps, job: &ClaimedGenerationJob) {
    emit_committed_event(deps, job.job_id(), job.committed_event().clone());
}

fn lease_expiry_wait(now_ms: i64, expires: i64) -> Duration {
    if now_ms < 0 || expires <= now_ms {
        Duration::ZERO
    } else {
        u64::try_from(expires - now_ms)
            .map(Duration::from_millis)
            .unwrap_or(Duration::ZERO)
    }
}

fn schedule_next_renewal(next_renewal: &mut tokio::time::Instant, interval: Duration) {
    let now = tokio::time::Instant::now();
    loop {
        let Some(next) = next_renewal.checked_add(interval) else {
            *next_renewal = now;
            return;
        };
        *next_renewal = next;
        if *next_renewal > now {
            return;
        }
    }
}

fn record_expired_lease(deps: &dyn GenerationJobWorkerDeps, job_id: Option<&str>) {
    deps.record_diagnostic(WorkerDiagnostic {
        kind: WorkerDiagnosticKind::RenewLease,
        job_id: job_id.map(str::to_owned),
        message: "generation worker lease expired before renewal".to_owned(),
    });
}

async fn renew_for_session(
    deps: &dyn GenerationJobWorkerDeps,
    authority: &WorkerTransitionAuthority,
    lease_ttl: Duration,
    known_expiry: &mut KnownLeaseExpiry,
    job_id: Option<&str>,
) -> RenewSessionOutcome {
    let now_ms = deps.now_ms();
    if known_expiry.is_expired(now_ms) {
        record_expired_lease(deps, job_id);
        return RenewSessionOutcome::LeaseLost;
    }
    match deps.renew_lease(authority, now_ms, lease_ttl).await {
        Ok(expires) => {
            let current_now_ms = deps.now_ms();
            if known_expiry.replace_from_wall_clock(current_now_ms, expires, lease_ttl) {
                RenewSessionOutcome::Renewed
            } else {
                deps.record_diagnostic(WorkerDiagnostic {
                    kind: WorkerDiagnosticKind::RenewLease,
                    job_id: job_id.map(str::to_owned),
                    message: "generation worker renewal returned an expired lease".to_owned(),
                });
                RenewSessionOutcome::LeaseLost
            }
        }
        Err(error) => {
            let lease_lost = error.kind == WorkerCoreErrorKind::LeaseLost
                || known_expiry.is_expired(deps.now_ms());
            deps.record_diagnostic(WorkerDiagnostic::from_error(
                WorkerDiagnosticKind::RenewLease,
                job_id,
                &error,
            ));
            if lease_lost {
                RenewSessionOutcome::LeaseLost
            } else {
                RenewSessionOutcome::TransientFailure
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecuteClaimedOutcome {
    Completed,
    NeedsReconciliation,
    Shutdown,
    LeaseLost,
}

async fn execute_claimed_job(
    inner: &GenerationJobQueueInner,
    deps: Arc<dyn GenerationJobWorkerDeps>,
    authority: &WorkerTransitionAuthority,
    job: WorkerExecutionJob,
    config: GenerationJobWorkerConfig,
    known_expiry: &mut KnownLeaseExpiry,
    renew_interval: &mut tokio::time::Interval,
    next_renewal: &mut tokio::time::Instant,
    shutdown: &mut watch::Receiver<bool>,
) -> ExecuteClaimedOutcome {
    let job_id = job.job_id().to_owned();
    if *shutdown.borrow() {
        return ExecuteClaimedOutcome::Shutdown;
    }
    if known_expiry.is_expired(deps.now_ms()) {
        record_expired_lease(deps.as_ref(), Some(&job_id));
        return ExecuteClaimedOutcome::LeaseLost;
    }
    let (cancellation, cancellation_receiver) = watch::channel(false);
    let _registration = match CancellationRegistration::insert(
        &inner.cancellations,
        job_id.clone(),
        cancellation.clone(),
    ) {
        Ok(registration) => Some(registration),
        Err(CancellationRegistrationError::DuplicateJobId) => {
            cancellation.send_replace(true);
            deps.record_diagnostic(WorkerDiagnostic {
                kind: WorkerDiagnosticKind::CancellationRegistryInvariant,
                job_id: Some(job_id.clone()),
                message: "duplicate active generation job ID".to_owned(),
            });
            return ExecuteClaimedOutcome::NeedsReconciliation;
        }
    };
    match deps.reread_cancel_requested(authority, job.job_id()).await {
        Ok(true) => {
            cancellation.send_replace(true);
        }
        Ok(false) => {}
        Err(error) => {
            let lease_lost = error.kind == WorkerCoreErrorKind::LeaseLost;
            deps.record_diagnostic(WorkerDiagnostic::from_error(
                WorkerDiagnosticKind::ReadCancellation,
                Some(&job_id),
                &error,
            ));
            if lease_lost {
                return ExecuteClaimedOutcome::LeaseLost;
            }
            cancellation.send_replace(true);
        }
    }
    if *shutdown.borrow() {
        return ExecuteClaimedOutcome::Shutdown;
    }
    if known_expiry.is_expired(deps.now_ms()) {
        record_expired_lease(deps.as_ref(), Some(&job_id));
        return ExecuteClaimedOutcome::LeaseLost;
    }
    let execute_deps = Arc::clone(&deps);
    let execute_authority = authority.clone();
    let mut execution = tokio::spawn(async move {
        execute_deps
            .execute_job(&execute_authority, job, cancellation_receiver)
            .await
    });
    let mut shutdown_requested = false;

    loop {
        let expiry_deadline = known_expiry.deadline(deps.now_ms());
        tokio::select! {
            execution_result = &mut execution => {
                return match record_execution_result(deps.as_ref(), &job_id, execution_result) {
                    ExecutionResultOutcome::DurablyFinished if shutdown_requested => {
                        ExecuteClaimedOutcome::Shutdown
                    }
                    ExecutionResultOutcome::DurablyFinished => ExecuteClaimedOutcome::Completed,
                    ExecutionResultOutcome::NeedsReconciliation if shutdown_requested => {
                        ExecuteClaimedOutcome::Shutdown
                    }
                    ExecutionResultOutcome::NeedsReconciliation => {
                        ExecuteClaimedOutcome::NeedsReconciliation
                    }
                    ExecutionResultOutcome::LeaseLost => ExecuteClaimedOutcome::LeaseLost,
                };
            }
            _ = tokio::time::sleep_until(expiry_deadline) => {
                record_expired_lease(deps.as_ref(), Some(&job_id));
                cancellation.send_replace(true);
                let _ = record_execution_result(deps.as_ref(), &job_id, execution.await);
                return ExecuteClaimedOutcome::LeaseLost;
            }
            _ = renew_interval.tick() => {
                schedule_next_renewal(next_renewal, config.heartbeat_interval);
                match renew_for_session(
                    deps.as_ref(),
                    authority,
                    config.lease_ttl,
                    known_expiry,
                    Some(&job_id),
                ).await {
                    RenewSessionOutcome::Renewed => {
                        if let Err(error) = deps
                            .heartbeat_job(authority, &job_id, deps.now_ms())
                            .await
                        {
                            let lease_lost = error.kind == WorkerCoreErrorKind::LeaseLost;
                            deps.record_diagnostic(WorkerDiagnostic::from_error(
                                WorkerDiagnosticKind::HeartbeatJob,
                                Some(&job_id),
                                &error,
                            ));
                            if lease_lost {
                                cancellation.send_replace(true);
                                let _ = record_execution_result(
                                    deps.as_ref(),
                                    &job_id,
                                    execution.await,
                                );
                                return ExecuteClaimedOutcome::LeaseLost;
                            }
                        }
                        match deps.reread_cancel_requested(authority, &job_id).await {
                            Ok(true) => {
                                cancellation.send_replace(true);
                            }
                            Ok(false) => {}
                            Err(error) => {
                                let lease_lost = error.kind == WorkerCoreErrorKind::LeaseLost;
                                deps.record_diagnostic(WorkerDiagnostic::from_error(
                                    WorkerDiagnosticKind::ReadCancellation,
                                    Some(&job_id),
                                    &error,
                                ));
                                cancellation.send_replace(true);
                                if lease_lost {
                                    let _ = record_execution_result(
                                        deps.as_ref(),
                                        &job_id,
                                        execution.await,
                                    );
                                    return ExecuteClaimedOutcome::LeaseLost;
                                }
                            }
                        }
                    }
                    RenewSessionOutcome::TransientFailure => {}
                    RenewSessionOutcome::LeaseLost => {
                        cancellation.send_replace(true);
                        let _ = record_execution_result(deps.as_ref(), &job_id, execution.await);
                        return ExecuteClaimedOutcome::LeaseLost;
                    }
                }
            }
            changed = shutdown.changed(), if !shutdown_requested => {
                if changed.is_err() || *shutdown.borrow() {
                    shutdown_requested = true;
                    cancellation.send_replace(true);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecutionResultOutcome {
    DurablyFinished,
    NeedsReconciliation,
    LeaseLost,
}

fn record_execution_result(
    deps: &dyn GenerationJobWorkerDeps,
    job_id: &str,
    result: Result<Result<WorkerExecutionOutcome, WorkerCoreError>, tokio::task::JoinError>,
) -> ExecutionResultOutcome {
    match result {
        Ok(Ok(WorkerExecutionOutcome::DurablyFinished)) => ExecutionResultOutcome::DurablyFinished,
        Ok(Ok(WorkerExecutionOutcome::NeedsReconciliation)) => {
            ExecutionResultOutcome::NeedsReconciliation
        }
        Ok(Ok(WorkerExecutionOutcome::LeaseLost)) => ExecutionResultOutcome::LeaseLost,
        Ok(Err(error)) => {
            let lease_lost = error.kind == WorkerCoreErrorKind::LeaseLost;
            deps.record_diagnostic(WorkerDiagnostic::from_error(
                WorkerDiagnosticKind::ExecuteJob,
                Some(job_id),
                &error,
            ));
            if lease_lost {
                ExecutionResultOutcome::LeaseLost
            } else {
                ExecutionResultOutcome::NeedsReconciliation
            }
        }
        Err(error) => {
            // A JoinError's display text includes the panic payload. Provider
            // execution may panic while a secret is in scope, so diagnostics
            // must classify the failure without ever formatting that payload.
            let message = if error.is_cancelled() {
                "generation execution task was cancelled"
            } else {
                "generation execution task panicked"
            };
            deps.record_diagnostic(WorkerDiagnostic {
                kind: WorkerDiagnosticKind::ExecutePanic,
                job_id: Some(job_id.to_owned()),
                message: message.to_owned(),
            });
            ExecutionResultOutcome::NeedsReconciliation
        }
    }
}

async fn release_session(
    deps: &dyn GenerationJobWorkerDeps,
    authority: &WorkerTransitionAuthority,
) {
    if let Err(error) = deps.release_lease(authority).await {
        deps.record_diagnostic(WorkerDiagnostic::from_error(
            WorkerDiagnosticKind::ReleaseLease,
            None,
            &error,
        ));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupAction {
    KeepQueued,
    RecoverResponse,
    AcknowledgeCancellation,
    Interrupt,
    IgnoreTerminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartupRecoveryEvidence {
    PreProvider,
    ProviderOutcomeMayExist,
    ResponseReady,
}

pub(crate) fn startup_action(
    status: &GenerationJobStatus,
    evidence: StartupRecoveryEvidence,
    cancel_requested: bool,
) -> StartupAction {
    match status {
        GenerationJobStatus::Queued if cancel_requested => StartupAction::AcknowledgeCancellation,
        GenerationJobStatus::Queued => StartupAction::KeepQueued,
        GenerationJobStatus::Running if evidence == StartupRecoveryEvidence::ResponseReady => {
            StartupAction::RecoverResponse
        }
        GenerationJobStatus::Running
            if evidence == StartupRecoveryEvidence::PreProvider && cancel_requested =>
        {
            StartupAction::AcknowledgeCancellation
        }
        GenerationJobStatus::Running => StartupAction::Interrupt,
        GenerationJobStatus::Completed
        | GenerationJobStatus::Failed
        | GenerationJobStatus::Cancelled
        | GenerationJobStatus::Interrupted => StartupAction::IgnoreTerminal,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AutomaticRetryPolicy {
    base_delay: Duration,
    max_delay: Duration,
}

impl AutomaticRetryPolicy {
    pub(crate) fn new(base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            base_delay,
            max_delay,
        }
    }

    pub(crate) fn delay(
        &self,
        error: &EngineCallError,
        auto_attempt: i32,
        max_auto_attempts: i32,
        now: DateTime<Utc>,
        jitter: Duration,
    ) -> Option<Duration> {
        let code_is_automatically_retryable = matches!(
            error.code.as_str(),
            "rate_limited" | "provider_unavailable" | "network_before_response"
        );
        if !code_is_automatically_retryable
            || !error.safe_to_retry
            || error.outcome_ambiguous
            || auto_attempt < 0
            || max_auto_attempts < 0
            || auto_attempt >= max_auto_attempts
        {
            return None;
        }

        let delay = match error.retry_after.as_ref() {
            Some(RetryAfterHint::DelaySeconds(seconds)) => Duration::from_secs(*seconds),
            Some(RetryAfterHint::HttpDate(retry_at)) if retry_at <= &now => Duration::ZERO,
            Some(RetryAfterHint::HttpDate(retry_at)) => (*retry_at - now).to_std().ok()?,
            Some(RetryAfterHint::Invalid) => return None,
            None => {
                if self.base_delay.is_zero() {
                    jitter
                } else {
                    let mut delay = self.base_delay;
                    let mut remaining = u32::try_from(auto_attempt).ok()?;
                    while remaining > 0 {
                        let step = remaining.min(31);
                        delay = delay.checked_mul(1_u32 << step)?;
                        if delay > self.max_delay {
                            return None;
                        }
                        remaining -= step;
                    }
                    delay.checked_add(jitter)?
                }
            }
        };

        (delay <= self.max_delay).then_some(delay)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProviderFailureAction {
    RetryAfter(Duration),
    Terminal {
        status: GenerationJobStatus,
        retryable: bool,
    },
}

pub(crate) fn provider_failure_action(
    policy: &AutomaticRetryPolicy,
    error: &EngineCallError,
    auto_attempt: i32,
    max_auto_attempts: i32,
    now: DateTime<Utc>,
    jitter: Duration,
) -> ProviderFailureAction {
    if let Some(delay) = policy.delay(error, auto_attempt, max_auto_attempts, now, jitter) {
        return ProviderFailureAction::RetryAfter(delay);
    }

    let retryable = error.safe_to_retry
        || error.outcome_ambiguous
        || matches!(
            error.code.as_str(),
            "rate_limited"
                | "provider_unavailable"
                | "network_before_response"
                | "provider_outcome_unknown"
        );
    ProviderFailureAction::Terminal {
        status: if error.outcome_ambiguous {
            GenerationJobStatus::Interrupted
        } else {
            GenerationJobStatus::Failed
        },
        retryable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn retry_policy() -> AutomaticRetryPolicy {
        AutomaticRetryPolicy::new(Duration::from_secs(2), Duration::from_secs(60))
    }

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, 11, 8, 0, 0)
            .single()
            .expect("valid fixed time")
    }

    #[test]
    fn startup_reconciliation_keeps_recovers_cancels_and_interrupts_exact_states() {
        assert_eq!(
            startup_action(
                &GenerationJobStatus::Queued,
                StartupRecoveryEvidence::PreProvider,
                false,
            ),
            StartupAction::KeepQueued
        );
        assert_eq!(
            startup_action(
                &GenerationJobStatus::Running,
                StartupRecoveryEvidence::ResponseReady,
                true,
            ),
            StartupAction::RecoverResponse,
            "a known provider result wins over cancellation and must finish locally"
        );
        assert_eq!(
            startup_action(
                &GenerationJobStatus::Running,
                StartupRecoveryEvidence::PreProvider,
                true,
            ),
            StartupAction::AcknowledgeCancellation
        );
        assert_eq!(
            startup_action(
                &GenerationJobStatus::Running,
                StartupRecoveryEvidence::ProviderOutcomeMayExist,
                true,
            ),
            StartupAction::Interrupt,
            "a stale cancel cannot discard an unknown provider outcome"
        );
        assert_eq!(
            startup_action(
                &GenerationJobStatus::Running,
                StartupRecoveryEvidence::PreProvider,
                false,
            ),
            StartupAction::Interrupt
        );
        for terminal in [
            GenerationJobStatus::Completed,
            GenerationJobStatus::Failed,
            GenerationJobStatus::Cancelled,
            GenerationJobStatus::Interrupted,
        ] {
            assert_eq!(
                startup_action(&terminal, StartupRecoveryEvidence::ResponseReady, true),
                StartupAction::IgnoreTerminal
            );
        }
    }

    #[test]
    fn automatic_retry_policy_obeys_exact_attempt_boundaries_and_jittered_backoff() {
        let error = EngineCallError::network_before_response();
        let now = fixed_now();
        let jitter = Duration::from_millis(250);

        assert_eq!(retry_policy().delay(&error, 0, 0, now, jitter), None);
        assert_eq!(
            retry_policy().delay(&error, 0, 1, now, jitter),
            Some(Duration::from_millis(2250))
        );
        assert_eq!(retry_policy().delay(&error, 1, 1, now, jitter), None);
        assert_eq!(
            retry_policy().delay(&error, 1, 2, now, jitter),
            Some(Duration::from_millis(4250))
        );
        assert_eq!(retry_policy().delay(&error, 2, 2, now, jitter), None);
        assert_eq!(retry_policy().delay(&error, -1, 2, now, jitter), None);
    }

    #[test]
    fn retry_after_seconds_and_http_dates_are_typed_capped_and_never_jittered() {
        let now = fixed_now();
        let seconds = EngineCallError::from_http_status(429, Some(RetryAfterHint::DelaySeconds(3)));
        assert_eq!(
            retry_policy().delay(&seconds, 0, 2, now, Duration::from_secs(9)),
            Some(Duration::from_secs(3))
        );

        let future = EngineCallError::from_http_status(
            503,
            Some(RetryAfterHint::HttpDate(now + chrono::Duration::seconds(7))),
        );
        assert_eq!(
            retry_policy().delay(&future, 0, 2, now, Duration::ZERO),
            Some(Duration::from_secs(7))
        );
        let past = EngineCallError::from_http_status(
            503,
            Some(RetryAfterHint::HttpDate(now - chrono::Duration::seconds(1))),
        );
        assert_eq!(
            retry_policy().delay(&past, 0, 2, now, Duration::ZERO),
            Some(Duration::ZERO)
        );

        let too_large =
            EngineCallError::from_http_status(429, Some(RetryAfterHint::DelaySeconds(61)));
        assert_eq!(
            retry_policy().delay(&too_large, 0, 2, now, Duration::ZERO),
            None
        );
    }

    #[test]
    fn invalid_retry_after_ambiguous_and_rejected_errors_never_auto_retry() {
        let now = fixed_now();
        let invalid = EngineCallError::from_http_status(429, Some(RetryAfterHint::Invalid));
        assert!(!invalid.safe_to_retry);
        assert_eq!(
            retry_policy().delay(&invalid, 0, 2, now, Duration::ZERO),
            None
        );
        assert_eq!(
            retry_policy().delay(
                &EngineCallError::provider_outcome_unknown("closed after send"),
                0,
                2,
                now,
                Duration::ZERO,
            ),
            None
        );
        assert_eq!(
            retry_policy().delay(
                &EngineCallError::request_rejected(),
                0,
                2,
                now,
                Duration::ZERO,
            ),
            None
        );
        let future_safe_classification = EngineCallError {
            code: "future_safe_classification".to_string(),
            sanitized_message: "A future provider failure".to_string(),
            retry_after: None,
            safe_to_retry: true,
            outcome_ambiguous: false,
        };
        assert_eq!(
            retry_policy().delay(&future_safe_classification, 0, 2, now, Duration::ZERO,),
            None,
            "new error codes require an explicit automatic-retry policy decision"
        );
    }

    #[test]
    fn overflowing_or_over_cap_fallback_backoff_stops_automatic_retry() {
        let representable = AutomaticRetryPolicy::new(Duration::from_nanos(1), Duration::MAX);
        assert_eq!(
            representable.delay(
                &EngineCallError::network_before_response(),
                32,
                33,
                fixed_now(),
                Duration::ZERO,
            ),
            Some(Duration::from_nanos(1_u64 << 32)),
            "attempt ordinals must be limited by Duration/cap, not a u32 multiplier"
        );

        let policy = AutomaticRetryPolicy::new(Duration::MAX, Duration::MAX);
        assert_eq!(
            policy.delay(
                &EngineCallError::network_before_response(),
                1,
                i32::MAX,
                fixed_now(),
                Duration::from_nanos(1),
            ),
            None
        );
    }

    #[test]
    fn provider_failure_policy_separates_auto_retry_from_manual_retryability() {
        let now = fixed_now();
        let automatic =
            EngineCallError::from_http_status(429, Some(RetryAfterHint::DelaySeconds(3)));
        assert_eq!(
            provider_failure_action(&retry_policy(), &automatic, 0, 2, now, Duration::ZERO,),
            ProviderFailureAction::RetryAfter(Duration::from_secs(3))
        );

        let ambiguous = EngineCallError::provider_outcome_unknown("closed after send");
        assert_eq!(
            provider_failure_action(&retry_policy(), &ambiguous, 0, 2, now, Duration::ZERO,),
            ProviderFailureAction::Terminal {
                status: GenerationJobStatus::Interrupted,
                retryable: true,
            }
        );

        for manually_retryable in [
            EngineCallError::from_http_status(429, Some(RetryAfterHint::Invalid)),
            EngineCallError::from_http_status(503, Some(RetryAfterHint::DelaySeconds(61))),
            EngineCallError::network_before_response(),
        ] {
            assert_eq!(
                provider_failure_action(
                    &retry_policy(),
                    &manually_retryable,
                    2,
                    2,
                    now,
                    Duration::ZERO,
                ),
                ProviderFailureAction::Terminal {
                    status: GenerationJobStatus::Failed,
                    retryable: true,
                }
            );
        }

        assert_eq!(
            provider_failure_action(
                &retry_policy(),
                &EngineCallError::request_rejected(),
                0,
                2,
                now,
                Duration::ZERO,
            ),
            ProviderFailureAction::Terminal {
                status: GenerationJobStatus::Failed,
                retryable: false,
            }
        );
    }
}

#[cfg(test)]
#[path = "generation_job_worker_tests.rs"]
mod queue_tests;
