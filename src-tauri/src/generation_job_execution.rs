use crate::api_gateway::{ImageEngine, ProviderAttemptBody};
use crate::commands::settings;
use crate::db::Database;
use crate::error::AppError;
use crate::file_manager::{PromotedGenerationFiles, StagedGenerationFiles};
use crate::generation_job_worker::{
    provider_failure_action, AutomaticRetryPolicy, ProviderFailureAction, WorkerCoreError,
    WorkerCoreErrorKind, WorkerExecutionOutcome,
};
use crate::generation_jobs::{
    get_job, get_job_in_transaction, load_generation_execution_snapshot_for_stage_in_transaction,
    reserve_automatic_retry_with_event_with_transaction_time,
    transition_running_job_stage_with_event_with_transaction_time,
    validate_worker_recovery_for_stage, GenerationJobTransition, WorkerStageTransition,
    WorkerTransitionError,
};
use crate::generation_lifecycle::{
    acknowledge_generation_cancellation_fenced_with_transaction_time,
    commit_generation_success_fenced_with_transaction_time,
    finalize_generation_failure_fenced_with_transaction_time, perform_provider_http_attempt,
    persist_provider_attempt_response, prepare_provider_attempt,
    promote_verified_response_fenced_with_transaction_time, resume_verified_response,
    validate_provider_execution_snapshot, CancellationProbe, FencedGenerationSuccessTransition,
    FileResponseArtifactStore, GenerationExecutionError, GenerationExecutionSnapshot,
    GenerationFileStore, GenerationTerminalDisposition, ImageResponseDecoder,
    PreparedProviderAttempt, ProviderAttemptResponse, ProviderExecutionCredentials,
    ResponseArtifactStore,
};
use crate::generation_worker_lease::{WorkerLeaseError, WorkerTransitionAuthority};
use crate::models::{GenerationJob, GenerationJobEvent, GenerationJobStage, GenerationJobStatus};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;

/// A narrow event boundary for the not-yet-wired production worker adapter.
/// Events passed here are owned snapshots returned only after a fenced commit.
pub(crate) trait GenerationExecutionEventSink: Send + Sync + 'static {
    fn emit(&self, event: GenerationJobEvent) -> Result<(), ()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GenerationExecutionDiagnosticKind {
    EventSink,
    UnsupportedStage,
    Repository,
    ArtifactPromotion,
    LocalCleanup,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenerationExecutionDiagnostic {
    pub(crate) kind: GenerationExecutionDiagnosticKind,
    pub(crate) job_id: String,
    pub(crate) message: &'static str,
}

pub(crate) trait GenerationExecutionDiagnosticSink: Send + Sync + 'static {
    fn record(&self, diagnostic: GenerationExecutionDiagnostic);
}

pub(crate) trait GenerationExecutionClock: Send + Sync + 'static {
    /// Must be non-blocking and side-effect free. Fenced repositories invoke
    /// this while holding their database mutex and immediate write transaction.
    fn now_ms(&self) -> i64;
    fn now_utc(&self) -> DateTime<Utc>;
}

#[async_trait]
pub(crate) trait GenerationExecutionSleeper: Send + Sync + 'static {
    async fn sleep(&self, delay: Duration);
}

pub(crate) trait GenerationRetryJitter: Send + Sync + 'static {
    fn jitter(&self, job_id: &str, auto_attempt: i32) -> Duration;
}

pub(crate) struct SystemGenerationExecutionClock;

impl GenerationExecutionClock for SystemGenerationExecutionClock {
    fn now_ms(&self) -> i64 {
        Utc::now().timestamp_millis()
    }

    fn now_utc(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

pub(crate) struct TokioGenerationExecutionSleeper;

#[async_trait]
impl GenerationExecutionSleeper for TokioGenerationExecutionSleeper {
    async fn sleep(&self, delay: Duration) {
        tokio::time::sleep(delay).await;
    }
}

pub(crate) struct ZeroGenerationRetryJitter;

impl GenerationRetryJitter for ZeroGenerationRetryJitter {
    fn jitter(&self, _job_id: &str, _auto_attempt: i32) -> Duration {
        Duration::ZERO
    }
}

/// Unwired execution component used by the next slice's
/// `ProductionGenerationJobWorkerDeps`. It intentionally does not claim jobs,
/// reconcile startup state, emit Tauri events, or own the worker loop.
pub(crate) struct GenerationJobExecutionAdapter {
    db: Database,
    engine: Arc<dyn ImageEngine>,
    artifact_store: FileResponseArtifactStore,
    decoder: Arc<dyn ImageResponseDecoder>,
    file_store: Arc<dyn GenerationFileStore>,
    clock: Arc<dyn GenerationExecutionClock>,
    sleeper: Arc<dyn GenerationExecutionSleeper>,
    jitter: Arc<dyn GenerationRetryJitter>,
    event_sink: Arc<dyn GenerationExecutionEventSink>,
    diagnostic_sink: Arc<dyn GenerationExecutionDiagnosticSink>,
    retry_policy: AutomaticRetryPolicy,
}

impl GenerationJobExecutionAdapter {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        db: Database,
        engine: Arc<dyn ImageEngine>,
        artifact_store: FileResponseArtifactStore,
        decoder: Arc<dyn ImageResponseDecoder>,
        file_store: Arc<dyn GenerationFileStore>,
        clock: Arc<dyn GenerationExecutionClock>,
        sleeper: Arc<dyn GenerationExecutionSleeper>,
        jitter: Arc<dyn GenerationRetryJitter>,
        event_sink: Arc<dyn GenerationExecutionEventSink>,
        diagnostic_sink: Arc<dyn GenerationExecutionDiagnosticSink>,
        retry_policy: AutomaticRetryPolicy,
    ) -> Self {
        Self {
            db,
            engine,
            artifact_store,
            decoder,
            file_store,
            clock,
            sleeper,
            jitter,
            event_sink,
            diagnostic_sink,
            retry_policy,
        }
    }

    pub(crate) async fn execute(
        &self,
        authority: &WorkerTransitionAuthority,
        job_id: &str,
        cancellation: watch::Receiver<bool>,
    ) -> Result<WorkerExecutionOutcome, WorkerCoreError> {
        let job = match self.load_job(job_id).await {
            Ok(job) => job,
            Err(failure) => return Ok(self.persistence_outcome(job_id, failure)),
        };
        let expected_stage = job.stage;
        match expected_stage {
            GenerationJobStage::Preparing
            | GenerationJobStage::ResponseReady
            | GenerationJobStage::LocalProcessing => {}
            _ => {
                self.record(
                    GenerationExecutionDiagnosticKind::UnsupportedStage,
                    job_id,
                    "generation execution requires startup reconciliation",
                );
                return Ok(WorkerExecutionOutcome::NeedsReconciliation);
            }
        }
        let loaded = match self.load_execution_at_stage(job_id, expected_stage).await {
            Ok(loaded) => loaded,
            Err(failure) => return Ok(self.persistence_outcome(job_id, failure)),
        };
        match expected_stage {
            GenerationJobStage::Preparing => {
                self.execute_preparing(authority, loaded, cancellation)
                    .await
            }
            GenerationJobStage::ResponseReady => {
                self.execute_response_ready(authority, loaded, cancellation)
                    .await
            }
            GenerationJobStage::LocalProcessing => {
                self.execute_local_processing(authority, loaded, cancellation)
                    .await
            }
            _ => unreachable!("supported execution stages were checked above"),
        }
    }

    async fn execute_preparing(
        &self,
        authority: &WorkerTransitionAuthority,
        loaded: LoadedExecution,
        mut cancellation: watch::Receiver<bool>,
    ) -> Result<WorkerExecutionOutcome, WorkerCoreError> {
        let job_id = loaded.job.id.clone();
        if loaded.job.cancel_requested_at.is_some() || receiver_cancelled(&cancellation) {
            return self
                .finish_observed_cancellation(authority, &loaded.snapshot)
                .await;
        }

        let mut material = match self.load_provider_material(&loaded.snapshot).await {
            Ok(material) => material,
            Err(ProviderMaterialFailure::Terminal(error)) => {
                return self
                    .finish_failure(authority, &loaded.snapshot, error, false, false)
                    .await;
            }
            Err(ProviderMaterialFailure::Repository) => {
                self.record(
                    GenerationExecutionDiagnosticKind::Repository,
                    &job_id,
                    "generation provider material could not be loaded",
                );
                return Ok(WorkerExecutionOutcome::NeedsReconciliation);
            }
            Err(ProviderMaterialFailure::Join) => return Err(blocking_task_error()),
        };
        if loaded.job.cancel_requested_at.is_some() || receiver_cancelled(&cancellation) {
            return self
                .finish_observed_cancellation(authority, &loaded.snapshot)
                .await;
        }

        let expected_response_file = match self
            .artifact_store
            .expected_response_path(&loaded.snapshot.context)
        {
            Ok(path) => path,
            Err(error) => {
                return self
                    .finish_failure(authority, &loaded.snapshot, error, false, false)
                    .await;
            }
        };
        let begin = self
            .transition_stage(
                authority,
                &job_id,
                GenerationJobStage::Preparing,
                WorkerStageTransition::BeginProviderRequest {
                    expected_response_file,
                },
            )
            .await;
        let begin = match begin {
            Ok(begin) => begin,
            Err(failure) => {
                if receiver_cancelled(&cancellation) {
                    return self
                        .finish_observed_cancellation(authority, &loaded.snapshot)
                        .await;
                }
                return Ok(self.persistence_outcome(&job_id, failure));
            }
        };
        self.emit_transition(begin);

        let mut auto_attempt = loaded.job.auto_attempt;
        let max_auto_attempts = loaded.job.max_auto_attempts;
        loop {
            if receiver_cancelled(&cancellation) {
                return self
                    .finish_observed_cancellation(authority, &loaded.snapshot)
                    .await;
            }
            let provider_result = self
                .await_provider_attempt(&loaded.snapshot, &material, &mut cancellation)
                .await;
            let Some(provider_result) = provider_result else {
                return self
                    .finish_observed_cancellation(authority, &loaded.snapshot)
                    .await;
            };
            match provider_result {
                Ok(body) => {
                    return self
                        .persist_and_resume(authority, loaded.snapshot.clone(), body, cancellation)
                        .await;
                }
                Err(GenerationExecutionError::Engine(engine_error)) => {
                    let action = provider_failure_action(
                        &self.retry_policy,
                        &engine_error,
                        auto_attempt,
                        max_auto_attempts,
                        self.clock.now_utc(),
                        self.jitter.jitter(&job_id, auto_attempt),
                    );
                    match action {
                        ProviderFailureAction::RetryAfter(delay) => {
                            let retry = self
                                .transition_stage(
                                    authority,
                                    &job_id,
                                    GenerationJobStage::ProviderRequest,
                                    WorkerStageTransition::EnterRetryBackoff,
                                )
                                .await;
                            let retry = match retry {
                                Ok(retry) => retry,
                                Err(failure) => {
                                    return Ok(self.persistence_outcome(&job_id, failure));
                                }
                            };
                            self.emit_transition(retry);
                            let cancelled_during_backoff = {
                                let sleep = self.sleeper.sleep(delay);
                                tokio::pin!(sleep);
                                tokio::select! {
                                    _ = &mut sleep => false,
                                    _ = wait_for_cancellation(&mut cancellation) => true,
                                }
                            };
                            if cancelled_during_backoff {
                                return self
                                    .finish_observed_cancellation(authority, &loaded.snapshot)
                                    .await;
                            }
                            let after_sleep = match self.load_job(&job_id).await {
                                Ok(job) => job,
                                Err(failure) => {
                                    return Ok(self.persistence_outcome(&job_id, failure));
                                }
                            };
                            if after_sleep.cancel_requested_at.is_some() {
                                return self
                                    .finish_observed_cancellation(authority, &loaded.snapshot)
                                    .await;
                            }
                            if after_sleep.status != GenerationJobStatus::Running
                                || after_sleep.stage != GenerationJobStage::RetryBackoff
                                || after_sleep.auto_attempt != auto_attempt
                            {
                                self.record(
                                    GenerationExecutionDiagnosticKind::Repository,
                                    &job_id,
                                    "generation retry state changed during backoff",
                                );
                                return Ok(WorkerExecutionOutcome::NeedsReconciliation);
                            }
                            let reserved =
                                self.reserve_retry(authority, &job_id, auto_attempt).await;
                            let reserved = match reserved {
                                Ok(reserved) => reserved,
                                Err(failure) => {
                                    if receiver_cancelled(&cancellation) {
                                        return self
                                            .finish_observed_cancellation(
                                                authority,
                                                &loaded.snapshot,
                                            )
                                            .await;
                                    }
                                    return Ok(self.persistence_outcome(&job_id, failure));
                                }
                            };
                            auto_attempt = reserved.value.auto_attempt;
                            self.emit_transition(reserved);
                            // A retry reservation authorizes exactly one new
                            // attempt. Resolve the exact snapshotted profile's
                            // current secret and re-read edit bytes only after
                            // that reservation and before creating HTTP work.
                            material = match self.load_provider_material(&loaded.snapshot).await {
                                Ok(material) => material,
                                Err(ProviderMaterialFailure::Terminal(error)) => {
                                    return self
                                        .finish_failure(
                                            authority,
                                            &loaded.snapshot,
                                            error,
                                            false,
                                            false,
                                        )
                                        .await;
                                }
                                Err(ProviderMaterialFailure::Repository) => {
                                    self.record(
                                        GenerationExecutionDiagnosticKind::Repository,
                                        &job_id,
                                        "generation retry material could not be loaded",
                                    );
                                    return Ok(WorkerExecutionOutcome::NeedsReconciliation);
                                }
                                Err(ProviderMaterialFailure::Join) => {
                                    return Err(blocking_task_error());
                                }
                            };
                        }
                        ProviderFailureAction::Terminal { status, retryable } => {
                            return self
                                .finish_failure(
                                    authority,
                                    &loaded.snapshot,
                                    GenerationExecutionError::Engine(engine_error),
                                    status == GenerationJobStatus::Interrupted,
                                    retryable,
                                )
                                .await;
                        }
                    }
                }
                Err(error) => {
                    return self
                        .finish_failure(authority, &loaded.snapshot, error, false, false)
                        .await;
                }
            }
        }
    }

    async fn await_provider_attempt(
        &self,
        snapshot: &GenerationExecutionSnapshot,
        material: &ProviderAttemptMaterial,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Option<Result<ProviderAttemptBody, GenerationExecutionError>> {
        let provider = perform_provider_http_attempt(
            self.engine.as_ref(),
            snapshot,
            &material.credentials,
            &material.prepared,
        );
        tokio::pin!(provider);
        tokio::select! {
            biased;
            result = &mut provider => Some(result),
            _ = wait_for_cancellation(cancellation) => None,
        }
    }

    async fn persist_and_resume(
        &self,
        authority: &WorkerTransitionAuthority,
        snapshot: GenerationExecutionSnapshot,
        body: ProviderAttemptBody,
        cancellation: watch::Receiver<bool>,
    ) -> Result<WorkerExecutionOutcome, WorkerCoreError> {
        let job_id = snapshot.context.job_id.clone();
        // Once the provider returns a complete body, any artifact-write or SQL
        // uncertainty must be reconciled from disk. It must never replay the
        // paid request or delete the requesting recovery row.
        let response =
            match persist_provider_attempt_response(&self.artifact_store, &snapshot, body).await {
                Ok(response) => response,
                Err(_) => {
                    self.record(
                        GenerationExecutionDiagnosticKind::ArtifactPromotion,
                        &job_id,
                        "provider response artifact requires reconciliation",
                    );
                    return Ok(WorkerExecutionOutcome::NeedsReconciliation);
                }
            };
        let promoted = self.promote_response(authority, &snapshot, response).await;
        let event = match promoted {
            Ok(event) => event,
            Err(failure) => return Ok(self.persistence_outcome(&job_id, failure)),
        };
        self.emit(event);
        let loaded = match self
            .load_execution_at_stage(&job_id, GenerationJobStage::ResponseReady)
            .await
        {
            Ok(loaded) => loaded,
            Err(failure) => return Ok(self.persistence_outcome(&job_id, failure)),
        };
        if loaded.snapshot.context != snapshot.context
            || loaded.snapshot.request != snapshot.request
        {
            self.record(
                GenerationExecutionDiagnosticKind::Repository,
                &job_id,
                "promoted response snapshot changed unexpectedly",
            );
            return Ok(WorkerExecutionOutcome::NeedsReconciliation);
        }
        self.execute_response_ready(authority, loaded, cancellation)
            .await
    }

    async fn execute_response_ready(
        &self,
        authority: &WorkerTransitionAuthority,
        loaded: LoadedExecution,
        cancellation: watch::Receiver<bool>,
    ) -> Result<WorkerExecutionOutcome, WorkerCoreError> {
        let job_id = loaded.snapshot.context.job_id.clone();
        // Verify the no-follow artifact against the exact committed recovery
        // descriptor while the job is still ResponseReady. A bad artifact is
        // terminalized from ResponseReady and must not be mislabeled as a
        // preservable LocalProcessing failure.
        let response = match self.load_exact_response(&loaded).await {
            Ok(response) => response,
            Err(error) => {
                return self
                    .finish_failure(authority, &loaded.snapshot, error, false, false)
                    .await;
            }
        };
        if loaded.job.cancel_requested_at.is_some() || receiver_cancelled(&cancellation) {
            return self
                .finish_observed_cancellation(authority, &loaded.snapshot)
                .await;
        }
        let transitioned = self
            .transition_stage(
                authority,
                &job_id,
                GenerationJobStage::ResponseReady,
                WorkerStageTransition::EnterLocalProcessing,
            )
            .await;
        let transitioned = match transitioned {
            Ok(transitioned) => transitioned,
            Err(failure) => {
                if failure != PersistenceFailure::LeaseLost {
                    if let Ok(job) = self.load_job(&job_id).await {
                        if job.cancel_requested_at.is_some() {
                            return self
                                .finish_observed_cancellation(authority, &loaded.snapshot)
                                .await;
                        }
                    }
                }
                return Ok(self.persistence_outcome(&job_id, failure));
            }
        };
        self.emit_transition(transitioned);
        self.execute_local_with_response(authority, loaded.snapshot, response, cancellation)
            .await
    }

    async fn execute_local_processing(
        &self,
        authority: &WorkerTransitionAuthority,
        loaded: LoadedExecution,
        cancellation: watch::Receiver<bool>,
    ) -> Result<WorkerExecutionOutcome, WorkerCoreError> {
        if loaded.job.cancel_requested_at.is_some() || receiver_cancelled(&cancellation) {
            return self
                .finish_observed_cancellation(authority, &loaded.snapshot)
                .await;
        }
        let response = match self.load_exact_response(&loaded).await {
            Ok(response) => response,
            Err(error) => {
                if receiver_cancelled(&cancellation) {
                    return self
                        .finish_observed_cancellation(authority, &loaded.snapshot)
                        .await;
                }
                return self
                    .finish_failure(authority, &loaded.snapshot, error, false, false)
                    .await;
            }
        };
        self.execute_local_with_response(authority, loaded.snapshot, response, cancellation)
            .await
    }

    async fn execute_local_with_response(
        &self,
        authority: &WorkerTransitionAuthority,
        snapshot: GenerationExecutionSnapshot,
        response: ProviderAttemptResponse,
        mut cancellation: watch::Receiver<bool>,
    ) -> Result<WorkerExecutionOutcome, WorkerCoreError> {
        let job_id = snapshot.context.job_id.clone();

        let probe = CancellationProbe::new();
        if receiver_cancelled(&cancellation) {
            probe.cancel();
        }
        let local = resume_verified_response(
            self.decoder.as_ref(),
            self.file_store.as_ref(),
            &snapshot,
            &response,
            &probe,
        );
        tokio::pin!(local);
        let local_result = if probe.is_cancelled() {
            local.await
        } else {
            tokio::select! {
                result = &mut local => result,
                _ = wait_for_cancellation(&mut cancellation) => {
                    probe.cancel();
                    local.await
                }
            }
        };
        let staged = match local_result {
            Ok(staged) => staged,
            Err(error) => {
                if probe.is_cancelled() || receiver_cancelled(&cancellation) {
                    return self
                        .finish_observed_cancellation(authority, &snapshot)
                        .await;
                }
                return self
                    .finish_failure(authority, &snapshot, error, false, false)
                    .await;
            }
        };

        if receiver_cancelled(&cancellation) {
            self.drop_staged(job_id.clone(), staged).await?;
            return self
                .finish_observed_cancellation(authority, &snapshot)
                .await;
        }
        let promoted = match self.promote_staged(staged, &mut cancellation).await {
            Ok(Some(promoted)) => promoted,
            Ok(None) => {
                return self
                    .finish_observed_cancellation(authority, &snapshot)
                    .await;
            }
            Err(PromoteStagedFailure::Local(error)) => {
                return self
                    .finish_failure(authority, &snapshot, error, false, false)
                    .await;
            }
            Err(PromoteStagedFailure::Join) => return Err(blocking_task_error()),
        };
        let committed = self.commit_success(authority, &snapshot, promoted).await;
        match committed {
            Ok(FencedGenerationSuccessTransition::Completed { event, .. }) => {
                self.emit(*event);
                Ok(WorkerExecutionOutcome::DurablyFinished)
            }
            Ok(FencedGenerationSuccessTransition::CancelRequested) => {
                self.finish_observed_cancellation(authority, &snapshot)
                    .await
            }
            Err(failure) => Ok(self.persistence_outcome(&job_id, failure)),
        }
    }

    async fn finish_failure(
        &self,
        authority: &WorkerTransitionAuthority,
        snapshot: &GenerationExecutionSnapshot,
        error: GenerationExecutionError,
        interrupted: bool,
        retryable: bool,
    ) -> Result<WorkerExecutionOutcome, WorkerCoreError> {
        let job_id = snapshot.context.job_id.clone();
        let stage = match self.load_job(&job_id).await {
            Ok(job) => job.stage,
            Err(failure) => return Ok(self.persistence_outcome(&job_id, failure)),
        };
        let preserve_response_ready = stage == GenerationJobStage::LocalProcessing;
        let disposition = GenerationTerminalDisposition {
            status: if interrupted {
                GenerationJobStatus::Interrupted
            } else {
                GenerationJobStatus::Failed
            },
            error_code: error.code().to_string(),
            retryable,
            preserve_response_ready,
        };
        let db = self.db.clone();
        let context = snapshot.context.clone();
        let fenced_authority = authority.clone();
        let clock = Arc::clone(&self.clock);
        let transition = tokio::task::spawn_blocking(move || {
            finalize_generation_failure_fenced_with_transaction_time(
                &db,
                &context,
                &error,
                &disposition,
                &fenced_authority,
                || clock.now_ms(),
            )
        })
        .await
        .map_err(|_| blocking_task_error())?;
        match transition {
            Ok(event) => {
                self.emit(event);
                Ok(WorkerExecutionOutcome::DurablyFinished)
            }
            Err(error) => {
                let failure = PersistenceFailure::from_transition(error);
                if failure != PersistenceFailure::LeaseLost {
                    if let Ok(job) = self.load_job(&job_id).await {
                        if job.cancel_requested_at.is_some() {
                            return self.finish_observed_cancellation(authority, snapshot).await;
                        }
                    }
                }
                Ok(self.persistence_outcome(&job_id, failure))
            }
        }
    }

    async fn finish_observed_cancellation(
        &self,
        authority: &WorkerTransitionAuthority,
        snapshot: &GenerationExecutionSnapshot,
    ) -> Result<WorkerExecutionOutcome, WorkerCoreError> {
        let job_id = snapshot.context.job_id.clone();
        let job = match self.load_job(&job_id).await {
            Ok(job) => job,
            Err(failure) => return Ok(self.persistence_outcome(&job_id, failure)),
        };
        // The watch channel also closes for shutdown, lease loss, and
        // fail-closed cancellation reads. Only a durable CancelRequested row
        // authorizes destructive acknowledgement of recovery evidence.
        if job.cancel_requested_at.is_none() {
            return Ok(WorkerExecutionOutcome::NeedsReconciliation);
        }
        let db = self.db.clone();
        let context = snapshot.context.clone();
        let authority = authority.clone();
        let clock = Arc::clone(&self.clock);
        let transition = tokio::task::spawn_blocking(move || {
            acknowledge_generation_cancellation_fenced_with_transaction_time(
                &db,
                &context,
                &authority,
                || clock.now_ms(),
            )
        })
        .await
        .map_err(|_| blocking_task_error())?;
        match transition {
            Ok(event) => {
                self.emit(event);
                Ok(WorkerExecutionOutcome::DurablyFinished)
            }
            Err(error) => {
                Ok(self.persistence_outcome(&job_id, PersistenceFailure::from_transition(error)))
            }
        }
    }

    async fn load_exact_response(
        &self,
        loaded: &LoadedExecution,
    ) -> Result<ProviderAttemptResponse, GenerationExecutionError> {
        let descriptor = loaded
            .response
            .as_ref()
            .ok_or_else(response_artifact_error)?;
        let response = self
            .artifact_store
            .load_verified_response(&loaded.snapshot.context, &descriptor.file)
            .await?;
        if response.response_file != descriptor.file.to_string_lossy()
            || response.response_size != descriptor.size
            || response.response_sha256 != descriptor.sha256
            || response.requested_image_count != loaded.snapshot.runtime_options.image_count
        {
            return Err(response_artifact_error());
        }
        Ok(response)
    }

    async fn load_execution_at_stage(
        &self,
        job_id: &str,
        expected_stage: GenerationJobStage,
    ) -> Result<LoadedExecution, PersistenceFailure> {
        let db = self.db.clone();
        let job_id = job_id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().map_err(|_| PersistenceFailure::Repository)?;
            let tx = conn
                .unchecked_transaction()
                .map_err(|_| PersistenceFailure::Repository)?;
            let job =
                get_job_in_transaction(&tx, &job_id).map_err(|_| PersistenceFailure::Repository)?;
            if job.stage != expected_stage || job.status != GenerationJobStatus::Running {
                return Err(PersistenceFailure::Repository);
            }
            let snapshot = load_generation_execution_snapshot_for_stage_in_transaction(
                &tx,
                &job_id,
                expected_stage,
            )
            .map_err(|_| PersistenceFailure::Repository)?;
            validate_worker_recovery_for_stage(&tx, &job.generation_id, expected_stage)
                .map_err(|_| PersistenceFailure::Repository)?;
            let response = if matches!(
                expected_stage,
                GenerationJobStage::ResponseReady | GenerationJobStage::LocalProcessing
            ) {
                let descriptor = tx
                    .query_row(
                        "SELECT response_file, response_size, response_sha256
                       FROM generation_recoveries WHERE generation_id = ?1",
                        [&job.generation_id],
                        |row| {
                            Ok((
                                row.get::<_, Option<String>>(0)?,
                                row.get::<_, Option<i64>>(1)?,
                                row.get::<_, Option<String>>(2)?,
                            ))
                        },
                    )
                    .optional()
                    .map_err(|_| PersistenceFailure::Repository)?
                    .ok_or(PersistenceFailure::Repository)?;
                let (Some(file), Some(size), Some(sha256)) = descriptor else {
                    return Err(PersistenceFailure::Repository);
                };
                Some(PersistedResponseDescriptor {
                    file: PathBuf::from(file),
                    size: u64::try_from(size).map_err(|_| PersistenceFailure::Repository)?,
                    sha256,
                })
            } else {
                None
            };
            tx.commit().map_err(|_| PersistenceFailure::Repository)?;
            Ok(LoadedExecution {
                job,
                snapshot,
                response,
            })
        })
        .await
        .map_err(|_| PersistenceFailure::Join)?
    }

    async fn load_job(&self, job_id: &str) -> Result<GenerationJob, PersistenceFailure> {
        let db = self.db.clone();
        let job_id = job_id.to_string();
        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().map_err(|_| PersistenceFailure::Repository)?;
            get_job(&conn, &job_id).map_err(|_| PersistenceFailure::Repository)
        })
        .await
        .map_err(|_| PersistenceFailure::Join)?
    }

    async fn load_credentials(
        &self,
        snapshot: &GenerationExecutionSnapshot,
    ) -> Result<Option<ProviderExecutionCredentials>, CredentialFailure> {
        let db = self.db.clone();
        let model = snapshot.context.model.clone();
        let profile_id = snapshot.context.provider_profile_id.clone();
        tokio::task::spawn_blocking(move || {
            match settings::read_model_provider_api_key(&db, &model, &profile_id) {
                Ok(secret) => Ok(secret.map(ProviderExecutionCredentials::new)),
                Err(AppError::ProviderProfileNotFound { .. }) => {
                    Err(CredentialFailure::ProfileMissing)
                }
                Err(_) => Err(CredentialFailure::Repository),
            }
        })
        .await
        .map_err(|_| CredentialFailure::Join)?
    }

    async fn load_provider_material(
        &self,
        snapshot: &GenerationExecutionSnapshot,
    ) -> Result<ProviderAttemptMaterial, ProviderMaterialFailure> {
        validate_provider_execution_snapshot(snapshot)
            .map_err(ProviderMaterialFailure::Terminal)?;
        let credentials = match self.load_credentials(snapshot).await {
            Ok(Some(credentials)) => credentials,
            Ok(None) => {
                return Err(ProviderMaterialFailure::Terminal(local_error(
                    "provider_configuration_invalid",
                    "The image provider configuration is invalid",
                    "provider_credentials",
                )));
            }
            Err(CredentialFailure::ProfileMissing) => {
                return Err(ProviderMaterialFailure::Terminal(local_error(
                    "provider_profile_missing",
                    "The selected provider profile is unavailable",
                    "provider_credentials",
                )));
            }
            Err(CredentialFailure::Repository) => {
                return Err(ProviderMaterialFailure::Repository);
            }
            Err(CredentialFailure::Join) => return Err(ProviderMaterialFailure::Join),
        };
        let prepared = prepare_provider_attempt(snapshot)
            .await
            .map_err(ProviderMaterialFailure::Terminal)?;
        Ok(ProviderAttemptMaterial {
            credentials,
            prepared,
        })
    }

    async fn transition_stage(
        &self,
        authority: &WorkerTransitionAuthority,
        job_id: &str,
        expected_stage: GenerationJobStage,
        transition: WorkerStageTransition,
    ) -> Result<GenerationJobTransition<GenerationJob>, PersistenceFailure> {
        let db = self.db.clone();
        let authority = authority.clone();
        let job_id = job_id.to_string();
        let clock = Arc::clone(&self.clock);
        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().map_err(|_| PersistenceFailure::Repository)?;
            transition_running_job_stage_with_event_with_transaction_time(
                &conn,
                &job_id,
                expected_stage,
                transition,
                &authority,
                || clock.now_ms(),
            )
            .map_err(PersistenceFailure::from_transition)
        })
        .await
        .map_err(|_| PersistenceFailure::Join)?
    }

    async fn reserve_retry(
        &self,
        authority: &WorkerTransitionAuthority,
        job_id: &str,
        expected_auto_attempt: i32,
    ) -> Result<GenerationJobTransition<GenerationJob>, PersistenceFailure> {
        let db = self.db.clone();
        let authority = authority.clone();
        let job_id = job_id.to_string();
        let clock = Arc::clone(&self.clock);
        tokio::task::spawn_blocking(move || {
            let conn = db.conn.lock().map_err(|_| PersistenceFailure::Repository)?;
            reserve_automatic_retry_with_event_with_transaction_time(
                &conn,
                &job_id,
                expected_auto_attempt,
                &authority,
                || clock.now_ms(),
            )
            .map_err(PersistenceFailure::from_transition)
        })
        .await
        .map_err(|_| PersistenceFailure::Join)?
    }

    async fn promote_response(
        &self,
        authority: &WorkerTransitionAuthority,
        snapshot: &GenerationExecutionSnapshot,
        response: ProviderAttemptResponse,
    ) -> Result<GenerationJobEvent, PersistenceFailure> {
        let db = self.db.clone();
        let store = self.artifact_store.clone();
        let context = snapshot.context.clone();
        let authority = authority.clone();
        let clock = Arc::clone(&self.clock);
        tokio::task::spawn_blocking(move || {
            promote_verified_response_fenced_with_transaction_time(
                &db,
                &store,
                &context,
                &response,
                &authority,
                || clock.now_ms(),
            )
            .map_err(PersistenceFailure::from_transition)
        })
        .await
        .map_err(|_| PersistenceFailure::Join)?
    }

    async fn commit_success(
        &self,
        authority: &WorkerTransitionAuthority,
        snapshot: &GenerationExecutionSnapshot,
        promoted: PromotedGenerationFiles,
    ) -> Result<FencedGenerationSuccessTransition, PersistenceFailure> {
        let db = self.db.clone();
        let context = snapshot.context.clone();
        let request = snapshot.request.clone();
        let authority = authority.clone();
        let clock = Arc::clone(&self.clock);
        tokio::task::spawn_blocking(move || {
            commit_generation_success_fenced_with_transaction_time(
                &db,
                &context,
                &request,
                promoted,
                &authority,
                || clock.now_ms(),
            )
            .map_err(PersistenceFailure::from_transition)
        })
        .await
        .map_err(|_| PersistenceFailure::Join)?
    }

    async fn drop_staged(
        &self,
        job_id: String,
        staged: StagedGenerationFiles,
    ) -> Result<(), WorkerCoreError> {
        tokio::task::spawn_blocking(move || drop(staged))
            .await
            .map_err(|_| {
                self.record(
                    GenerationExecutionDiagnosticKind::LocalCleanup,
                    &job_id,
                    "staged generation cleanup task failed",
                );
                blocking_task_error()
            })
    }

    async fn promote_staged(
        &self,
        staged: StagedGenerationFiles,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<Option<PromotedGenerationFiles>, PromoteStagedFailure> {
        let promote = tokio::task::spawn_blocking(move || staged.promote());
        tokio::pin!(promote);
        let (result, cancelled) = tokio::select! {
            result = &mut promote => (result, false),
            _ = wait_for_cancellation(cancellation) => (promote.await, true),
        };
        let promoted = result
            .map_err(|_| PromoteStagedFailure::Join)?
            .map_err(|_| {
                PromoteStagedFailure::Local(local_error(
                    "image_save_failed",
                    "Generated images could not be saved",
                    "image_promotion",
                ))
            })?;
        if cancelled || receiver_cancelled(cancellation) {
            tokio::task::spawn_blocking(move || drop(promoted))
                .await
                .map_err(|_| PromoteStagedFailure::Join)?;
            Ok(None)
        } else {
            Ok(Some(promoted))
        }
    }

    fn emit_transition(&self, transition: GenerationJobTransition<GenerationJob>) {
        if let Some(event) = transition.event {
            self.emit(event);
        }
    }

    fn emit(&self, event: GenerationJobEvent) {
        let job_id = event.job_id.clone();
        if self.event_sink.emit(event).is_err() {
            self.record(
                GenerationExecutionDiagnosticKind::EventSink,
                &job_id,
                "committed generation event could not be emitted",
            );
        }
    }

    fn persistence_outcome(
        &self,
        job_id: &str,
        failure: PersistenceFailure,
    ) -> WorkerExecutionOutcome {
        match failure {
            PersistenceFailure::LeaseLost => WorkerExecutionOutcome::LeaseLost,
            PersistenceFailure::Repository | PersistenceFailure::Join => {
                self.record(
                    GenerationExecutionDiagnosticKind::Repository,
                    job_id,
                    "generation execution state requires reconciliation",
                );
                WorkerExecutionOutcome::NeedsReconciliation
            }
        }
    }

    fn record(&self, kind: GenerationExecutionDiagnosticKind, job_id: &str, message: &'static str) {
        self.diagnostic_sink.record(GenerationExecutionDiagnostic {
            kind,
            job_id: job_id.to_string(),
            message,
        });
    }
}

struct LoadedExecution {
    job: GenerationJob,
    snapshot: GenerationExecutionSnapshot,
    response: Option<PersistedResponseDescriptor>,
}

struct PersistedResponseDescriptor {
    file: PathBuf,
    size: u64,
    sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PersistenceFailure {
    LeaseLost,
    Repository,
    Join,
}

impl PersistenceFailure {
    fn from_transition(error: WorkerTransitionError) -> Self {
        match error {
            WorkerTransitionError::Lease(WorkerLeaseError::LeaseLost) => Self::LeaseLost,
            WorkerTransitionError::Lease(_) | WorkerTransitionError::Repository(_) => {
                Self::Repository
            }
        }
    }
}

enum CredentialFailure {
    ProfileMissing,
    Repository,
    Join,
}

struct ProviderAttemptMaterial {
    credentials: ProviderExecutionCredentials,
    prepared: PreparedProviderAttempt,
}

enum ProviderMaterialFailure {
    Terminal(GenerationExecutionError),
    Repository,
    Join,
}

enum PromoteStagedFailure {
    Local(GenerationExecutionError),
    Join,
}

fn local_error(code: &str, message: &str, stage: &str) -> GenerationExecutionError {
    GenerationExecutionError::Local {
        code: code.to_string(),
        sanitized_message: message.to_string(),
        stage: stage.to_string(),
    }
}

fn response_artifact_error() -> GenerationExecutionError {
    local_error(
        "recovery_failed",
        "The provider response could not be verified",
        "response_artifact",
    )
}

fn blocking_task_error() -> WorkerCoreError {
    WorkerCoreError {
        kind: WorkerCoreErrorKind::Transient,
        message: "generation execution blocking task failed".to_string(),
    }
}

fn receiver_cancelled(cancellation: &watch::Receiver<bool>) -> bool {
    *cancellation.borrow()
}

async fn wait_for_cancellation(cancellation: &mut watch::Receiver<bool>) {
    loop {
        if *cancellation.borrow() {
            return;
        }
        if cancellation.changed().await.is_err() {
            return;
        }
    }
}

#[cfg(test)]
#[path = "generation_job_execution_tests.rs"]
mod tests;
