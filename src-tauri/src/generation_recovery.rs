//! Resumes saved responses only; never submits a provider generation request.
use crate::api_gateway::GptImageEngine;
use crate::db::Database;
use crate::error::AppError;
use crate::generation_jobs::{
    heartbeat_running_job_current_stage_fenced_with_transaction_time,
    prepare_modern_recovery_candidate_with_transaction_time,
    transition_running_job_stage_with_event_with_transaction_time, ModernStartupCandidate,
    ModernStartupRecovery, WorkerStageTransition, WorkerTransitionError,
};
use crate::generation_lifecycle::{
    acknowledge_generation_cancellation_fenced_with_transaction_time,
    commit_generation_success_fenced_with_transaction_time,
    finalize_generation_failure_fenced_with_transaction_time,
    promote_verified_response_fenced_with_transaction_time, resume_verified_response,
    CancellationProbe, EngineImageResponseDecoder, FencedGenerationSuccessTransition,
    FileResponseArtifactStore, GenerationExecutionError, GenerationTerminalDisposition,
    LocalGenerationFileStore,
};
use crate::generation_worker_lease::{
    acquire_worker_lease_with_transaction_time, release_worker_lease,
    renew_worker_lease_with_transaction_time, WorkerLeaseAcquireOutcome, WorkerTransitionAuthority,
};
use crate::models::{GenerationJobEvent, GenerationJobStage, GenerationJobStatus};
use crate::storage::{MaintenanceGate, StoragePaths};
use rusqlite::params;
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager, State};

const LEASE_TTL: Duration = Duration::from_secs(30);

#[derive(Default)]
pub(crate) struct StartupRecoveryReport(std::sync::atomic::AtomicUsize);
impl StartupRecoveryReport {
    fn record(&self, result: &RecoverySummary) {
        self.0
            .fetch_add(result.recovered, std::sync::atomic::Ordering::Relaxed);
    }
    fn merge(&self, mut summary: RecoverySummary) -> RecoverySummary {
        summary.recovered += self.0.load(std::sync::atomic::Ordering::Relaxed);
        summary
    }
}

#[derive(Debug, Default, Serialize)]
pub(crate) struct RecoverySummary {
    pub recovered: usize,
    pub interrupted: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub pending: usize,
    pub entries: Vec<RecoveryEntry>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RecoveryEntry {
    pub generation_id: String,
    pub conversation_id: String,
    pub prompt: String,
    pub status: String,
    pub can_recover: bool,
    pub result_unknown: bool,
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
fn lock_error() -> AppError {
    AppError::Database {
        message: "Recovery database lock unavailable".into(),
    }
}
fn transition_error(error: WorkerTransitionError) -> AppError {
    match error {
        WorkerTransitionError::Repository(error) => error,
        WorkerTransitionError::Lease(_) => AppError::Validation {
            message: "Recovery ownership expired; retry recovery".into(),
        },
    }
}

fn emit(app: Option<&tauri::AppHandle>, event: &GenerationJobEvent) {
    if let Some(app) = app {
        let _ = app.emit("generation:job", event);
        let name = if event.status == GenerationJobStatus::Completed {
            "generation:complete"
        } else {
            "generation:failed"
        };
        let _ = app.emit(name, serde_json::json!({"generation_id":event.generation_id,"status":event.status,"error":event.error_message}));
    }
}

struct RecoveryLease<'a> {
    db: &'a Database,
    authority: WorkerTransitionAuthority,
}
impl Drop for RecoveryLease<'_> {
    fn drop(&mut self) {
        if let Ok(conn) = self.db.conn.lock() {
            let _ = release_worker_lease(&conn, &self.authority);
        }
    }
}

fn pending_ids(db: &Database, generation_id: Option<&str>) -> Result<Vec<String>, AppError> {
    let conn = db.conn.lock().map_err(|_| lock_error())?;
    let mut statement = conn.prepare(
        "SELECT j.id FROM generation_jobs j WHERE (?1 IS NULL OR j.generation_id=?1)
         AND ((?1 IS NULL AND j.status IN ('running','queued'))
              OR (j.status IN ('failed','interrupted') AND EXISTS (
                  SELECT 1 FROM generation_recoveries r WHERE r.generation_id=j.generation_id AND r.request_state='response_ready')))
         ORDER BY j.queued_at, j.rowid"
    )?;
    let ids = statement
        .query_map(params![generation_id], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(ids)
}

fn finish_failure(
    db: &Database,
    candidate: &ModernStartupCandidate,
    authority: &WorkerTransitionAuthority,
    preserve: bool,
    app: Option<&tauri::AppHandle>,
) -> Result<GenerationJobStatus, AppError> {
    // Cancellation can arrive during response decoding or filesystem work.
    let cancelled = {
        let conn = db.conn.lock().map_err(|_| lock_error())?;
        conn.query_row(
            "SELECT cancel_requested_at IS NOT NULL FROM generation_jobs WHERE id=?1",
            [&candidate.snapshot.context.job_id],
            |r| r.get::<_, bool>(0),
        )?
    };
    if cancelled {
        return acknowledge_cancel(db, candidate, authority, app);
    }
    let code = if preserve {
        "recovery_failed"
    } else if candidate.stage == GenerationJobStage::Preparing {
        "app_interrupted"
    } else {
        "provider_outcome_unknown"
    };
    let status = if preserve {
        GenerationJobStatus::Failed
    } else {
        GenerationJobStatus::Interrupted
    };
    let event = finalize_generation_failure_fenced_with_transaction_time(
        db,
        &candidate.snapshot.context,
        &GenerationExecutionError::Local {
            code: code.into(),
            sanitized_message: "The saved result could not be recovered".into(),
            stage: "recovery".into(),
        },
        &GenerationTerminalDisposition {
            status: status.clone(),
            error_code: code.into(),
            retryable: true,
            preserve_response_ready: preserve,
        },
        authority,
        now_ms,
    )
    .map_err(transition_error)?;
    emit(app, &event);
    Ok(status)
}

fn acknowledge_cancel(
    db: &Database,
    candidate: &ModernStartupCandidate,
    authority: &WorkerTransitionAuthority,
    app: Option<&tauri::AppHandle>,
) -> Result<GenerationJobStatus, AppError> {
    let event = acknowledge_generation_cancellation_fenced_with_transaction_time(
        db,
        &candidate.snapshot.context,
        authority,
        now_ms,
    )
    .map_err(transition_error)?;
    emit(app, &event);
    Ok(GenerationJobStatus::Cancelled)
}

async fn recover_candidate(
    db: &Database,
    engine: &GptImageEngine,
    root: &Path,
    mut candidate: ModernStartupCandidate,
    authority: &WorkerTransitionAuthority,
    app: Option<&tauri::AppHandle>,
) -> Result<GenerationJobStatus, AppError> {
    if candidate.cancel_requested {
        return acknowledge_cancel(db, &candidate, authority, app);
    }
    if matches!(candidate.recovery, ModernStartupRecovery::RequestingEmpty) {
        return finish_failure(db, &candidate, authority, false, app);
    }
    let store = FileResponseArtifactStore::new(root.to_path_buf());
    // Enter the existing local-failure preservation state before checking a
    // saved response so a corrupt/missing file does not erase its recovery row.
    if candidate.stage == GenerationJobStage::ResponseReady {
        let conn = db.conn.lock().map_err(|_| lock_error())?;
        transition_running_job_stage_with_event_with_transaction_time(
            &conn,
            &candidate.snapshot.context.job_id,
            GenerationJobStage::ResponseReady,
            WorkerStageTransition::EnterLocalProcessing,
            authority,
            now_ms,
        )
        .map_err(transition_error)?;
        candidate.stage = GenerationJobStage::LocalProcessing;
    }
    let path = match &candidate.recovery {
        ModernStartupRecovery::RequestingBound { expected_file } => expected_file,
        ModernStartupRecovery::ResponseReady { file, .. } => file,
        ModernStartupRecovery::RequestingEmpty => unreachable!(),
    };
    let loaded = store
        .load_verified_response(&candidate.snapshot.context, path)
        .await;
    let response = match loaded {
        Ok(response)
            if response.requested_image_count == candidate.snapshot.runtime_options.image_count
                && match &candidate.recovery {
                    ModernStartupRecovery::ResponseReady { size, sha256, .. } => {
                        response.response_size == *size && response.response_sha256 == *sha256
                    }
                    _ => true,
                } =>
        {
            response
        }
        _ => {
            return finish_failure(
                db,
                &candidate,
                authority,
                candidate.stage == GenerationJobStage::LocalProcessing,
                app,
            )
        }
    };
    if candidate.stage == GenerationJobStage::ProviderRequest {
        promote_verified_response_fenced_with_transaction_time(
            db,
            &store,
            &candidate.snapshot.context,
            &response,
            authority,
            now_ms,
        )
        .map_err(transition_error)?;
        let conn = db.conn.lock().map_err(|_| lock_error())?;
        transition_running_job_stage_with_event_with_transaction_time(
            &conn,
            &candidate.snapshot.context.job_id,
            GenerationJobStage::ResponseReady,
            WorkerStageTransition::EnterLocalProcessing,
            authority,
            now_ms,
        )
        .map_err(transition_error)?;
        candidate.stage = GenerationJobStage::LocalProcessing;
    }
    let decoder = EngineImageResponseDecoder::new(Arc::new(engine.clone()));
    let file_store = LocalGenerationFileStore::new(root.to_path_buf());
    let cancellation = CancellationProbe::new();
    let continuation = resume_verified_response(
        &decoder,
        &file_store,
        &candidate.snapshot,
        &response,
        &cancellation,
    );
    tokio::pin!(continuation);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    let staged = loop {
        tokio::select! {
            result = &mut continuation => break result,
            _ = heartbeat.tick() => {
                let conn = db.conn.lock().map_err(|_| lock_error())?;
                renew_worker_lease_with_transaction_time(&conn, authority, LEASE_TTL, now_ms)
                    .map_err(|_| AppError::GenerationJobInvalidTransition)?;
                let job = heartbeat_running_job_current_stage_fenced_with_transaction_time(&conn, &candidate.snapshot.context.job_id, authority, now_ms).map_err(transition_error)?;
                if job.cancel_requested_at.is_some() { cancellation.cancel(); }
            }
        }
    };
    let staged = match staged {
        Ok(staged) => staged,
        Err(_) => return finish_failure(db, &candidate, authority, true, app),
    };
    let promoted = match staged.promote() {
        Ok(promoted) => promoted,
        Err(_) => return finish_failure(db, &candidate, authority, true, app),
    };
    match commit_generation_success_fenced_with_transaction_time(
        db,
        &candidate.snapshot.context,
        &candidate.snapshot.request,
        promoted,
        authority,
        now_ms,
    ) {
        Ok(FencedGenerationSuccessTransition::Completed { event, .. }) => {
            emit(app, &event);
            Ok(GenerationJobStatus::Completed)
        }
        Ok(FencedGenerationSuccessTransition::CancelRequested) => {
            acknowledge_cancel(db, &candidate, authority, app)
        }
        Err(_) => finish_failure(db, &candidate, authority, true, app),
    }
}

pub(crate) async fn recover_pending(
    db: &Database,
    engine: &GptImageEngine,
    root: &Path,
    generation_id: Option<&str>,
    app: Option<&tauri::AppHandle>,
) -> Result<RecoverySummary, AppError> {
    let ids = pending_ids(db, generation_id)?;
    if generation_id.is_some() && ids.is_empty() {
        return Err(AppError::GenerationJobNotRetryable);
    }
    if ids.is_empty() {
        return Ok(RecoverySummary::default());
    }
    let authority = {
        let conn = db.conn.lock().map_err(|_| lock_error())?;
        match acquire_worker_lease_with_transaction_time(
            &conn,
            &format!("recovery-{}", uuid::Uuid::new_v4()),
            LEASE_TTL,
            now_ms,
        )
        .map_err(|_| AppError::GenerationJobInvalidTransition)?
        {
            WorkerLeaseAcquireOutcome::Acquired { authority, .. } => authority,
            WorkerLeaseAcquireOutcome::Held { .. } => {
                return Err(AppError::Validation {
                    message: "Another recovery is already running".into(),
                })
            }
        }
    };
    let lease = RecoveryLease { db, authority };
    let mut summary = RecoverySummary::default();
    for id in ids {
        let prepared = {
            let conn = db.conn.lock().map_err(|_| lock_error())?;
            renew_worker_lease_with_transaction_time(&conn, &lease.authority, LEASE_TTL, now_ms)
                .map_err(|_| AppError::GenerationJobInvalidTransition)?;
            prepare_modern_recovery_candidate_with_transaction_time(
                &conn,
                &id,
                &lease.authority,
                now_ms,
            )
        };
        let result = match prepared {
            Ok(candidate) => {
                recover_candidate(db, engine, root, candidate, &lease.authority, app).await
            }
            Err(error) => Err(transition_error(error)),
        };
        match result {
            Ok(GenerationJobStatus::Completed) => summary.recovered += 1,
            Ok(GenerationJobStatus::Interrupted) => summary.interrupted += 1,
            Ok(GenerationJobStatus::Cancelled) => summary.cancelled += 1,
            _ => {
                summary.failed += 1;
                log::warn!(
                    "A saved generation could not be recovered; continuing with remaining tasks"
                );
            }
        }
    }
    summary.pending = pending_ids(db, None)?.len();
    Ok(summary)
}

/// Called only while the process holds the exclusive library lock and the
/// startup maintenance permit. No previous process can still own this lease.
pub(crate) async fn startup_recover(
    app: &tauri::AppHandle,
    db: &Database,
    engine: &GptImageEngine,
    root: &Path,
) -> Result<RecoverySummary, AppError> {
    let result = startup_recover_inner(Some(app), db, engine, root).await?;
    if app.try_state::<StartupRecoveryReport>().is_none() {
        app.manage(StartupRecoveryReport::default());
    }
    app.state::<StartupRecoveryReport>().record(&result);
    Ok(result)
}

async fn startup_recover_inner(
    app: Option<&tauri::AppHandle>,
    db: &Database,
    engine: &GptImageEngine,
    root: &Path,
) -> Result<RecoverySummary, AppError> {
    {
        let conn = db.conn.lock().map_err(|_| lock_error())?;
        conn.execute("UPDATE generation_worker_lease SET owner_id=NULL, acquired_at=NULL, heartbeat_at=NULL, expires_at=NULL WHERE id=1 AND owner_id IS NOT NULL", [])?;
    }
    recover_pending(db, engine, root, None, app).await
}

fn legacy_recovery_ready(db: &Database, generation_id: &str) -> Result<bool, AppError> {
    let conn = db.conn.lock().map_err(|_| lock_error())?;
    Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM generation_recoveries r JOIN generations g ON g.id=r.generation_id WHERE r.generation_id=?1 AND r.request_state='response_ready' AND g.status IN ('processing','failed') AND g.deleted_at IS NULL)", [generation_id], |row| row.get(0))?)
}

#[tauri::command]
pub(crate) async fn recover_generation(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    engine: State<'_, GptImageEngine>,
    paths: State<'_, StoragePaths>,
    gate: State<'_, MaintenanceGate>,
    generation_id: String,
) -> Result<RecoverySummary, AppError> {
    let _permit = gate
        .write()
        .map_err(|message| AppError::Validation { message })?;
    let modern = {
        let conn = db.conn.lock().map_err(|_| lock_error())?;
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM generation_jobs WHERE generation_id=?1)",
            [&generation_id],
            |row| row.get::<_, bool>(0),
        )?
    };
    if !modern {
        if !legacy_recovery_ready(&db, &generation_id)? {
            return Err(AppError::GenerationJobNotRetryable);
        }
        crate::recover_legacy_generation(&app, &db, &engine, &generation_id).await?;
        return Ok(RecoverySummary {
            recovered: 1,
            ..Default::default()
        });
    }
    recover_pending(
        &db,
        &engine,
        &paths.assets,
        Some(&generation_id),
        Some(&app),
    )
    .await
}

fn recovery_summary(db: &Database) -> Result<RecoverySummary, AppError> {
    let conn = db.conn.lock().map_err(|_| lock_error())?;
    let mut statement = conn.prepare(
        "SELECT generation_id, conversation_id, prompt, status, can_recover, result_unknown FROM (
         SELECT j.generation_id, COALESCE(g.conversation_id,'') AS conversation_id, substr(g.prompt,1,180) AS prompt, j.status,
            EXISTS(SELECT 1 FROM generation_recoveries r WHERE r.generation_id=j.generation_id AND r.request_state='response_ready') AS can_recover, j.error_code='provider_outcome_unknown' AS result_unknown,
            j.finished_at AS sort_time
         FROM generation_jobs j JOIN generations g ON g.id=j.generation_id
         WHERE j.status IN ('failed','interrupted') AND g.deleted_at IS NULL
         AND (j.error_code IN ('app_interrupted','provider_outcome_unknown') OR EXISTS(SELECT 1 FROM generation_recoveries r WHERE r.generation_id=j.generation_id AND r.request_state='response_ready'))
         AND NOT EXISTS(SELECT 1 FROM generation_jobs child WHERE child.parent_job_id=j.id)
         UNION ALL
         SELECT g.id, COALESCE(g.conversation_id,''), substr(g.prompt,1,180), g.status, 1, 0, g.created_at
         FROM generations g JOIN generation_recoveries r ON r.generation_id=g.id
         WHERE g.status='failed' AND g.deleted_at IS NULL AND r.request_state='response_ready'
         AND NOT EXISTS(SELECT 1 FROM generation_jobs j WHERE j.generation_id=g.id)
         ) ORDER BY sort_time DESC LIMIT 20"
    )?;
    let entries = statement
        .query_map([], |r| {
            Ok(RecoveryEntry {
                generation_id: r.get(0)?,
                conversation_id: r.get(1)?,
                prompt: r.get(2)?,
                status: r.get(3)?,
                can_recover: r.get(4)?,
                result_unknown: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(RecoverySummary {
        failed: entries.iter().filter(|e| e.status == "failed").count(),
        interrupted: entries.iter().filter(|e| e.status == "interrupted").count(),
        pending: entries.iter().filter(|e| e.can_recover).count(),
        entries,
        ..Default::default()
    })
}
#[tauri::command]
pub(crate) fn get_generation_recovery_summary(
    app: tauri::AppHandle,
    db: State<'_, Database>,
) -> Result<RecoverySummary, AppError> {
    let summary = recovery_summary(&db)?;
    Ok(match app.try_state::<StartupRecoveryReport>() {
        Some(report) => report.merge(summary),
        None => summary,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_gateway::ProviderAttemptBody;
    use crate::generation_jobs::{enqueue_job, GenerationJobOptions, PreparedGenerationJob};
    use crate::generation_lifecycle::{FileResponseArtifactStore, ResponseArtifactStore};
    use crate::models::GenerationJobStatus;
    use std::path::PathBuf;

    struct Fixture {
        root: PathBuf,
        db: crate::db::Database,
        job_id: String,
        generation_id: String,
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
            Self {
                root,
                db,
                job_id,
                generation_id,
            }
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
        fn status(&self) -> String {
            self.db
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT status FROM generation_jobs WHERE id=?1",
                    [&self.job_id],
                    |row| row.get(0),
                )
                .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[tokio::test]
    async fn startup_interrupts_unsubmitted_job_without_calling_provider_and_is_idempotent() {
        let fixture = Fixture::new();
        let engine =
            crate::api_gateway::GptImageEngine::new(&crate::config::AppConfig::default().api)
                .unwrap();
        let first = recover_pending(&fixture.db, &engine, &fixture.root, None, None)
            .await
            .unwrap();
        assert_eq!(first.interrupted, 1);
        assert_eq!(fixture.status(), "interrupted");
        let second = recover_pending(&fixture.db, &engine, &fixture.root, None, None)
            .await
            .unwrap();
        assert_eq!(second.interrupted, 0);
    }

    #[tokio::test]
    async fn restart_recovers_response_written_before_database_promotion_exactly_once() {
        let fixture = Fixture::new();
        let path = fixture.save_response(false).await;
        // Close and reopen an actual SQLite connection, as on process restart.
        let restarted = crate::db::Database::open(&fixture.root.join("fixture.db")).unwrap();
        let engine =
            crate::api_gateway::GptImageEngine::new(&crate::config::AppConfig::default().api)
                .unwrap();
        let summary = recover_pending(&restarted, &engine, &fixture.root, None, None)
            .await
            .unwrap();
        assert_eq!(summary.recovered, 1);
        assert_eq!(fixture.status(), "completed");
        let again = recover_pending(&restarted, &engine, &fixture.root, None, None)
            .await
            .unwrap();
        assert_eq!(again.recovered, 0);
        let count: i64 = restarted
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
        assert!(path.exists());
    }

    #[tokio::test]
    async fn corrupt_response_remains_recoverable_after_local_repair() {
        let fixture = Fixture::new();
        let path = fixture.save_response(true).await;
        let saved = std::fs::read(&path).unwrap();
        std::fs::write(&path, b"corrupt").unwrap();
        let engine =
            crate::api_gateway::GptImageEngine::new(&crate::config::AppConfig::default().api)
                .unwrap();
        let first = recover_pending(&fixture.db, &engine, &fixture.root, None, None)
            .await
            .unwrap();
        assert_eq!(first.failed, 1);
        assert_eq!(fixture.status(), "failed");
        assert!(path.exists());
        std::fs::write(&path, saved).unwrap();
        let second = recover_pending(
            &fixture.db,
            &engine,
            &fixture.root,
            Some(&fixture.generation_id),
            None,
        )
        .await
        .unwrap();
        assert_eq!(second.recovered, 1);
        assert_eq!(fixture.status(), "completed");
    }

    #[tokio::test]
    async fn cancellation_before_restart_does_not_publish_saved_image() {
        let fixture = Fixture::new();
        fixture.save_response(true).await;
        crate::generation_jobs::request_cancel(&fixture.db.conn.lock().unwrap(), &fixture.job_id)
            .unwrap();
        let engine =
            crate::api_gateway::GptImageEngine::new(&crate::config::AppConfig::default().api)
                .unwrap();
        let result = recover_pending(&fixture.db, &engine, &fixture.root, None, None)
            .await
            .unwrap();
        assert_eq!(result.cancelled, 1);
        assert_eq!(fixture.status(), "cancelled");
    }

    #[tokio::test]
    async fn corrupt_database_candidate_does_not_abort_valid_neighbour() {
        let broken = Fixture::new();
        broken
            .db
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE generation_jobs SET request_json='broken' WHERE id=?1",
                [&broken.job_id],
            )
            .unwrap();
        let valid = Fixture::at(broken.root.clone());
        valid.save_response(true).await;
        let engine = GptImageEngine::new(&crate::config::ApiConfig::default()).unwrap();
        let summary = recover_pending(&valid.db, &engine, &valid.root, None, None)
            .await
            .unwrap();
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.recovered, 1);
        assert_eq!(valid.status(), "completed");
    }

    #[tokio::test]
    async fn ambiguous_request_without_saved_response_requires_manual_new_retry() {
        let fixture = Fixture::new();
        let path = fixture.save_response(false).await;
        std::fs::remove_file(path).unwrap();
        let engine = GptImageEngine::new(&crate::config::ApiConfig::default()).unwrap();
        let result = recover_pending(&fixture.db, &engine, &fixture.root, None, None)
            .await
            .unwrap();
        assert_eq!(result.interrupted, 1);
        assert_eq!(fixture.status(), "interrupted");
        assert!(recovery_summary(&fixture.db).unwrap().entries[0].result_unknown);
        assert!(recover_pending(
            &fixture.db,
            &engine,
            &fixture.root,
            Some(&fixture.generation_id),
            None
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn sqlite_commit_failure_keeps_paid_response_and_next_restart_completes() {
        let fixture = Fixture::new();
        let path = fixture.save_response(true).await;
        fixture.db.conn.lock().unwrap().execute_batch("CREATE TRIGGER fail_recovered_success BEFORE INSERT ON images BEGIN SELECT RAISE(ABORT, 'injected local storage failure'); END;").unwrap();
        let engine = GptImageEngine::new(&crate::config::ApiConfig::default()).unwrap();
        let failed = recover_pending(&fixture.db, &engine, &fixture.root, None, None)
            .await
            .unwrap();
        assert_eq!(failed.failed, 1);
        assert_eq!(fixture.status(), "failed");
        assert!(path.exists());
        let count: i64 = fixture
            .db
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM images", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
        fixture
            .db
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER fail_recovered_success")
            .unwrap();
        let restarted = crate::db::Database::open(&fixture.root.join("fixture.db")).unwrap();
        let success = recover_pending(&restarted, &engine, &fixture.root, None, None)
            .await
            .unwrap();
        assert_eq!(success.recovered, 1);
        assert_eq!(fixture.status(), "completed");
    }

    #[tokio::test]
    async fn manual_recovery_never_steals_an_active_job() {
        let fixture = Fixture::new();
        fixture.save_response(true).await;
        let engine = GptImageEngine::new(&crate::config::ApiConfig::default()).unwrap();
        assert!(recover_pending(
            &fixture.db,
            &engine,
            &fixture.root,
            Some(&fixture.generation_id),
            None
        )
        .await
        .is_err());
        assert_eq!(fixture.status(), "running");
    }

    #[tokio::test]
    async fn recovery_does_not_take_over_an_unexpired_worker_lease() {
        use crate::generation_worker_lease::{
            acquire_worker_lease, release_worker_lease, WorkerLeaseAcquireOutcome,
        };
        let fixture = Fixture::new();
        let path = fixture.save_response(true).await;
        let authority = {
            let conn = fixture.db.conn.lock().unwrap();
            let WorkerLeaseAcquireOutcome::Acquired { authority, .. } =
                acquire_worker_lease(&conn, "other-live-worker", now_ms(), LEASE_TTL).unwrap()
            else {
                panic!("lease")
            };
            authority
        };
        let engine = GptImageEngine::new(&crate::config::ApiConfig::default()).unwrap();
        assert!(
            recover_pending(&fixture.db, &engine, &fixture.root, None, None)
                .await
                .is_err()
        );
        assert_eq!(fixture.status(), "running");
        assert!(path.exists());
        release_worker_lease(&fixture.db.conn.lock().unwrap(), &authority).unwrap();
        assert_eq!(
            recover_pending(&fixture.db, &engine, &fixture.root, None, None)
                .await
                .unwrap()
                .recovered,
            1
        );
    }

    #[tokio::test]
    async fn notice_summary_lists_actionable_results_not_historical_failure_totals() {
        let fixture = Fixture::new();
        let engine = GptImageEngine::new(&crate::config::ApiConfig::default()).unwrap();
        recover_pending(&fixture.db, &engine, &fixture.root, None, None)
            .await
            .unwrap();
        let summary = recovery_summary(&fixture.db).unwrap();
        assert_eq!(summary.entries.len(), 1);
        assert_eq!(summary.entries[0].generation_id, fixture.generation_id);
        assert!(!summary.entries[0].can_recover);
        fixture
            .db
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE generation_jobs SET status='failed', error_code='api_error' WHERE id=?1",
                [&fixture.job_id],
            )
            .unwrap();
        let summary = recovery_summary(&fixture.db).unwrap();
        assert_eq!(summary.entries.len(), 0);
        assert_eq!(summary.failed, 0);
    }

    // The child exits without dropping its database, lease or fixture guards.
    // It is ignored by normal test discovery and runs only in the isolated
    // subprocess tests below, never against an application library.
    #[tokio::test]
    #[ignore]
    async fn crash_fixture_process() {
        let root =
            std::env::var_os("ASTRO_RECOVERY_CRASH_FIXTURE").expect("isolated crash fixture path");
        let fixture = Fixture::at(PathBuf::from(root));
        fixture.save_response(false).await;
        if std::env::var("ASTRO_RECOVERY_CRASH_AFTER_COMMIT").as_deref() == Ok("1") {
            let engine = GptImageEngine::new(&crate::config::ApiConfig::default()).unwrap();
            assert_eq!(
                recover_pending(&fixture.db, &engine, &fixture.root, None, None)
                    .await
                    .unwrap()
                    .recovered,
                1
            );
            std::process::exit(74);
        }
        std::process::exit(73);
    }

    async fn recover_after_process_exit(after_commit: bool) {
        let root =
            std::env::temp_dir().join(format!("astro-process-crash-{}", uuid::Uuid::new_v4()));
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "generation_recovery::tests::crash_fixture_process",
                "--ignored",
                "--nocapture",
            ])
            .env("ASTRO_RECOVERY_CRASH_FIXTURE", &root)
            .env(
                "ASTRO_RECOVERY_CRASH_AFTER_COMMIT",
                if after_commit { "1" } else { "0" },
            )
            .output()
            .expect("run isolated crash child");
        assert_eq!(
            child.status.code(),
            Some(if after_commit { 74 } else { 73 }),
            "{}",
            String::from_utf8_lossy(&child.stderr)
        );
        let db = Database::open(&root.join("fixture.db")).unwrap();
        let engine = GptImageEngine::new(&crate::config::ApiConfig::default()).unwrap();
        let result = recover_pending(&db, &engine, &root, None, None)
            .await
            .unwrap();
        assert_eq!(result.recovered, usize::from(!after_commit));
        assert_eq!(
            recover_pending(&db, &engine, &root, None, None)
                .await
                .unwrap()
                .recovered,
            0
        );
        let conn = db.conn.lock().unwrap();
        let counts: (i64,i64,i64) = conn.query_row("SELECT (SELECT COUNT(*) FROM generation_jobs), (SELECT COUNT(*) FROM images), (SELECT COUNT(*) FROM generation_jobs WHERE status='completed' AND auto_attempt=0)", [], |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
        assert_eq!(counts, (1, 1, 1));
        drop(conn);
        drop(db);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn process_exit_after_response_before_promotion_recovers_without_resubmission() {
        recover_after_process_exit(false).await;
    }
    #[tokio::test]
    async fn process_exit_after_commit_reopens_without_duplicate_images() {
        recover_after_process_exit(true).await;
    }
    #[tokio::test]
    async fn actual_startup_is_safe_for_empty_lease_and_repeated_boot() {
        let fixture = Fixture::new();
        let engine = GptImageEngine::new(&crate::config::ApiConfig::default()).unwrap();
        let first = startup_recover_inner(None, &fixture.db, &engine, &fixture.root)
            .await
            .unwrap();
        assert_eq!(first.interrupted, 1);
        let second = startup_recover_inner(None, &fixture.db, &engine, &fixture.root)
            .await
            .unwrap();
        assert_eq!(second.interrupted, 0);
    }

    #[tokio::test]
    async fn actual_startup_reclaims_previous_process_lease_without_provider_resend() {
        let fixture = Fixture::new();
        fixture.save_response(false).await;
        {
            let conn = fixture.db.conn.lock().unwrap();
            crate::generation_worker_lease::acquire_worker_lease(
                &conn,
                "dead-process",
                now_ms(),
                LEASE_TTL,
            )
            .unwrap();
        }
        let engine = GptImageEngine::new(&crate::config::ApiConfig::default()).unwrap();
        assert_eq!(
            startup_recover_inner(None, &fixture.db, &engine, &fixture.root)
                .await
                .unwrap()
                .recovered,
            1
        );
        assert_eq!(fixture.status(), "completed");
    }

    #[test]
    fn notice_summary_includes_legacy_saved_failure_and_removes_completed_result() {
        let fixture = Fixture::new();
        let conn = fixture.db.conn.lock().unwrap();
        conn.execute("INSERT INTO generations(id,prompt,engine,size,quality,status,conversation_id) SELECT 'legacy-saved','Legacy recovery','gpt-image-2','1024x1024','high','failed',conversation_id FROM generations WHERE id=?1",[&fixture.generation_id]).unwrap();
        conn.execute("INSERT INTO generation_recoveries(generation_id,request_kind,request_state,output_format,response_file,created_at,updated_at) VALUES('legacy-saved','generate','response_ready','png','/fixture/saved.json','2026-07-10T00:00:00Z','2026-07-10T00:00:00Z')",[]).unwrap();
        drop(conn);
        let summary = recovery_summary(&fixture.db).unwrap();
        assert_eq!(summary.entries.len(), 1);
        assert!(summary.entries[0].can_recover);
        fixture
            .db
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE generations SET status='completed' WHERE id='legacy-saved'",
                [],
            )
            .unwrap();
        assert!(recovery_summary(&fixture.db).unwrap().entries.is_empty());
    }
    #[tokio::test]
    async fn successful_startup_report_remains_visible_after_recovery_rows_are_deleted() {
        let fixture = Fixture::new();
        fixture.save_response(true).await;
        let engine = GptImageEngine::new(&crate::config::ApiConfig::default()).unwrap();
        let finished = startup_recover_inner(None, &fixture.db, &engine, &fixture.root)
            .await
            .unwrap();
        let report = StartupRecoveryReport::default();
        report.record(&finished);
        let current = report.merge(recovery_summary(&fixture.db).unwrap());
        assert_eq!(current.recovered, 1);
        assert!(current.entries.is_empty());
    }
}

#[cfg(test)]
mod legacy_manual_guard_tests {
    use super::*;
    #[test]
    fn manual_legacy_recovery_requires_saved_response_and_incomplete_generation() {
        let path = std::env::temp_dir().join(format!("legacy-manual-{}.db", uuid::Uuid::new_v4()));
        let db = Database::open(&path).unwrap();
        db.run_migrations().unwrap();
        {
            let conn = db.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO generations(id,prompt,status) VALUES('g','fixture','failed')",
                [],
            )
            .unwrap();
            conn.execute("INSERT INTO generation_recoveries(generation_id,request_kind,request_state,output_format,created_at,updated_at) VALUES('g','generate','requesting','png','2026-11-13','2026-11-13')",[]).unwrap();
        }
        assert!(!legacy_recovery_ready(&db, "g").unwrap());
        db.conn.lock().unwrap().execute("UPDATE generation_recoveries SET request_state='response_ready' WHERE generation_id='g'",[]).unwrap();
        assert!(legacy_recovery_ready(&db, "g").unwrap());
        db.conn
            .lock()
            .unwrap()
            .execute("UPDATE generations SET status='completed' WHERE id='g'", [])
            .unwrap();
        assert!(!legacy_recovery_ready(&db, "g").unwrap());
        drop(db);
        std::fs::remove_file(path).unwrap();
    }
}
