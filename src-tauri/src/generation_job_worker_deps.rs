use crate::db::Database;
use crate::generation_job_execution::{GenerationExecutionClock, GenerationExecutionEventSink};
use crate::generation_job_worker::{
    ClaimedGenerationJob, GenerationJobWorkerDeps, StartupReconciliation, WorkerCoreError,
    WorkerCoreErrorKind, WorkerDiagnostic, WorkerExecutionJob, WorkerExecutionOutcome,
};
use crate::generation_jobs::{
    claim_next_job_fenced_with_event_with_transaction_time,
    heartbeat_running_job_current_stage_fenced_with_transaction_time,
    reread_running_job_cancel_requested_fenced_with_transaction_time, WorkerTransitionError,
};
use crate::generation_worker_lease::{
    acquire_worker_lease_with_transaction_time, release_worker_lease,
    renew_worker_lease_with_transaction_time, WorkerLeaseAcquireOutcome, WorkerLeaseError,
    WorkerTransitionAuthority,
};
use crate::models::{GenerationJobStage, GenerationJobStatus};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

#[async_trait]
pub(crate) trait GenerationJobExecutor: Send + Sync + 'static {
    async fn execute(
        &self,
        authority: &WorkerTransitionAuthority,
        job_id: &str,
        cancellation: watch::Receiver<bool>,
    ) -> Result<WorkerExecutionOutcome, WorkerCoreError>;
}

#[async_trait]
pub(crate) trait GenerationJobStartupReconciler: Send + Sync + 'static {
    async fn reconcile(
        &self,
        authority: &WorkerTransitionAuthority,
        now_ms: i64,
    ) -> Result<StartupReconciliation, WorkerCoreError>;
}

pub(crate) trait GenerationJobWorkerDiagnosticSink: Send + Sync + 'static {
    fn record(&self, diagnostic: WorkerDiagnostic);
}

pub(crate) struct RepositoryGenerationJobWorkerDeps {
    db: Database,
    executor: Arc<dyn GenerationJobExecutor>,
    reconciler: Arc<dyn GenerationJobStartupReconciler>,
    event_sink: Arc<dyn GenerationExecutionEventSink>,
    diagnostic_sink: Arc<dyn GenerationJobWorkerDiagnosticSink>,
    clock: Arc<dyn GenerationExecutionClock>,
}

#[cfg(test)]
impl RepositoryGenerationJobWorkerDeps {
    // Tests exercise the repository boundary with fakes. Production code must
    // not construct this graph until this module can bind a transaction-time
    // executor, reconciler, repository, event sink, and clock in one factory.
    fn new(
        db: Database,
        executor: Arc<dyn GenerationJobExecutor>,
        reconciler: Arc<dyn GenerationJobStartupReconciler>,
        event_sink: Arc<dyn GenerationExecutionEventSink>,
        diagnostic_sink: Arc<dyn GenerationJobWorkerDiagnosticSink>,
        clock: Arc<dyn GenerationExecutionClock>,
    ) -> Self {
        Self {
            db,
            executor,
            reconciler,
            event_sink,
            diagnostic_sink,
            clock,
        }
    }
}

fn worker_error(kind: WorkerCoreErrorKind, message: &'static str) -> WorkerCoreError {
    WorkerCoreError {
        kind,
        message: message.to_string(),
    }
}

fn transient(message: &'static str) -> WorkerCoreError {
    worker_error(WorkerCoreErrorKind::Transient, message)
}

fn map_lease_error(error: WorkerLeaseError, message: &'static str) -> WorkerCoreError {
    worker_error(
        if error == WorkerLeaseError::LeaseLost {
            WorkerCoreErrorKind::LeaseLost
        } else {
            WorkerCoreErrorKind::Transient
        },
        message,
    )
}

fn map_transition_error(error: WorkerTransitionError, message: &'static str) -> WorkerCoreError {
    worker_error(
        if matches!(
            error,
            WorkerTransitionError::Lease(WorkerLeaseError::LeaseLost)
        ) {
            WorkerCoreErrorKind::LeaseLost
        } else {
            WorkerCoreErrorKind::Transient
        },
        message,
    )
}

fn sanitize_delegated_error(error: WorkerCoreError, message: &'static str) -> WorkerCoreError {
    worker_error(error.kind, message)
}

#[async_trait]
impl GenerationJobWorkerDeps for RepositoryGenerationJobWorkerDeps {
    fn now_ms(&self) -> i64 {
        self.clock.now_ms()
    }

    async fn acquire_lease(
        &self,
        owner_id: &str,
        _now_ms: i64,
        ttl: Duration,
    ) -> Result<WorkerLeaseAcquireOutcome, WorkerCoreError> {
        let db = self.db.clone();
        let owner_id = owner_id.to_string();
        let clock = Arc::clone(&self.clock);
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn
                .lock()
                .map_err(|_| transient("generation worker lease database is unavailable"))?;
            acquire_worker_lease_with_transaction_time(&conn, &owner_id, ttl, || clock.now_ms())
                .map_err(|error| {
                    map_lease_error(error, "generation worker lease could not be acquired")
                })
        })
        .await
        .map_err(|_| transient("generation worker lease task failed"))?
    }

    async fn reconcile_startup(
        &self,
        authority: &WorkerTransitionAuthority,
        now_ms: i64,
    ) -> Result<StartupReconciliation, WorkerCoreError> {
        self.reconciler
            .reconcile(authority, now_ms)
            .await
            .map_err(|error| {
                sanitize_delegated_error(error, "generation startup reconciliation failed")
            })
    }

    async fn claim_next(
        &self,
        authority: &WorkerTransitionAuthority,
        _now_ms: i64,
    ) -> Result<Option<ClaimedGenerationJob>, WorkerCoreError> {
        let db = self.db.clone();
        let authority = authority.clone();
        let clock = Arc::clone(&self.clock);
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn
                .lock()
                .map_err(|_| transient("generation job database is unavailable"))?;
            let transition =
                claim_next_job_fenced_with_event_with_transaction_time(&conn, &authority, || {
                    clock.now_ms()
                })
                .map_err(|error| {
                    map_transition_error(error, "generation job could not be claimed")
                })?;
            transition
                .map(|transition| {
                    let event = transition
                        .event
                        .ok_or_else(|| transient("generation claim committed without an event"))?;
                    if transition.value.id != event.job_id
                        || transition.value.generation_id != event.generation_id
                        || transition.value.status != GenerationJobStatus::Running
                        || transition.value.stage != GenerationJobStage::Preparing
                        || event.status != GenerationJobStatus::Running
                        || event.stage != GenerationJobStage::Preparing
                    {
                        return Err(transient("generation claim projection is invalid"));
                    }
                    Ok(ClaimedGenerationJob::from_committed_event(event))
                })
                .transpose()
        })
        .await
        .map_err(|_| transient("generation claim task failed"))?
    }

    fn emit_committed_event(
        &self,
        event: crate::models::GenerationJobEvent,
    ) -> Result<(), WorkerCoreError> {
        self.event_sink
            .emit(event)
            .map_err(|_| transient("committed generation event could not be emitted"))
    }

    async fn reread_cancel_requested(
        &self,
        authority: &WorkerTransitionAuthority,
        job_id: &str,
    ) -> Result<bool, WorkerCoreError> {
        let db = self.db.clone();
        let authority = authority.clone();
        let job_id = job_id.to_string();
        let clock = Arc::clone(&self.clock);
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn
                .lock()
                .map_err(|_| transient("generation job database is unavailable"))?;
            reread_running_job_cancel_requested_fenced_with_transaction_time(
                &conn,
                &job_id,
                &authority,
                || clock.now_ms(),
            )
            .map_err(|error| {
                map_transition_error(error, "generation cancellation could not be read")
            })
        })
        .await
        .map_err(|_| transient("generation cancellation read task failed"))?
    }

    async fn execute_job(
        &self,
        authority: &WorkerTransitionAuthority,
        job: WorkerExecutionJob,
        cancellation: watch::Receiver<bool>,
    ) -> Result<WorkerExecutionOutcome, WorkerCoreError> {
        self.executor
            .execute(authority, job.job_id(), cancellation)
            .await
            .map_err(|error| sanitize_delegated_error(error, "generation job execution failed"))
    }

    async fn renew_lease(
        &self,
        authority: &WorkerTransitionAuthority,
        _now_ms: i64,
        ttl: Duration,
    ) -> Result<i64, WorkerCoreError> {
        let db = self.db.clone();
        let authority = authority.clone();
        let clock = Arc::clone(&self.clock);
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn
                .lock()
                .map_err(|_| transient("generation worker lease database is unavailable"))?;
            renew_worker_lease_with_transaction_time(&conn, &authority, ttl, || clock.now_ms())
                .map_err(|error| {
                    map_lease_error(error, "generation worker lease could not be renewed")
                })
        })
        .await
        .map_err(|_| transient("generation worker renewal task failed"))?
    }

    async fn heartbeat_job(
        &self,
        authority: &WorkerTransitionAuthority,
        job_id: &str,
        _now_ms: i64,
    ) -> Result<(), WorkerCoreError> {
        let db = self.db.clone();
        let authority = authority.clone();
        let job_id = job_id.to_string();
        let clock = Arc::clone(&self.clock);
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn
                .lock()
                .map_err(|_| transient("generation job database is unavailable"))?;
            heartbeat_running_job_current_stage_fenced_with_transaction_time(
                &conn,
                &job_id,
                &authority,
                || clock.now_ms(),
            )
            .map(|_| ())
            .map_err(|error| {
                map_transition_error(error, "generation heartbeat could not be committed")
            })
        })
        .await
        .map_err(|_| transient("generation heartbeat task failed"))?
    }

    async fn release_lease(
        &self,
        authority: &WorkerTransitionAuthority,
    ) -> Result<(), WorkerCoreError> {
        let db = self.db.clone();
        let authority = authority.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db
                .conn
                .lock()
                .map_err(|_| transient("generation worker lease database is unavailable"))?;
            release_worker_lease(&conn, &authority).map_err(|error| {
                map_lease_error(error, "generation worker lease could not be released")
            })
        })
        .await
        .map_err(|_| transient("generation worker release task failed"))?
    }

    fn record_diagnostic(&self, diagnostic: WorkerDiagnostic) {
        self.diagnostic_sink.record(diagnostic);
    }
}

#[cfg(test)]
#[path = "generation_job_worker_deps_tests.rs"]
mod tests;
