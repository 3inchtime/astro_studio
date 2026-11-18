use crate::db::Database;
use crate::error::AppError;
use crate::generation_jobs::{self, GenerationJobTransition};
use crate::generation_lifecycle::{
    prepare_generation_job, run_claimed_generation, CancellationProbe, GenerationLifecycleKind,
};
use crate::models::*;
use rusqlite::{Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tauri::{Emitter, Manager, State};
use tokio::sync::{watch, Notify};

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GenerationExecutionRequest {
    #[serde(default)]
    pub(crate) provider_override_confirmed: bool,
    pub(crate) provider_profile_id: Option<String>,
    pub(crate) source_generation_id: Option<String>,
    pub(crate) advanced_options: Option<crate::models::ImageAdvancedOptions>,
    pub(crate) aspect_ratio: Option<String>,
    pub(crate) resolution: Option<String>,
    pub(crate) kind: GenerationLifecycleKind,
    pub(crate) prompt: String,
    pub(crate) model: Option<String>,
    pub(crate) client_request_id: Option<String>,
    #[serde(default)]
    pub(crate) source_image_paths: Vec<String>,
    pub(crate) size: Option<String>,
    pub(crate) quality: Option<String>,
    pub(crate) background: Option<String>,
    pub(crate) output_format: Option<String>,
    pub(crate) output_compression: Option<u8>,
    pub(crate) moderation: Option<String>,
    pub(crate) input_fidelity: Option<String>,
    pub(crate) image_count: Option<u8>,
    pub(crate) conversation_id: Option<String>,
    pub(crate) project_id: Option<String>,
    pub(crate) document_id: Option<String>,
}

/// SQLite owns the queue. Notifications are only hints to re-read committed rows.
#[derive(Default)]
pub(crate) struct GenerationRuntime {
    wake_worker: Notify,
    changed: watch::Sender<()>,
    pub(crate) execution: tokio::sync::Mutex<()>,
}

pub(crate) fn publish(app: &tauri::AppHandle<impl tauri::Runtime>, event: &GenerationJobEvent) {
    let _ = app.emit("generation:job", event);
    app.state::<GenerationRuntime>().changed.send_replace(());
}

pub(crate) fn publish_current(app: &tauri::AppHandle<impl tauri::Runtime>, job_id: &str) {
    let db = app.state::<Database>();
    let event = db
        .connection()
        .and_then(|conn| generation_jobs::get_job_event(&conn, job_id));
    if let Ok(event) = event {
        publish(app, &event);
    }
}

pub(crate) enum DeletionScope<'a> {
    Generation(&'a str),
    Conversation(&'a str),
    Project(&'a str),
}

/// Stop work within the same write transaction that hides its parent. The
/// event is captured while the source remains visible, then emitted after commit.
fn cancel_for_deletion(
    tx: &rusqlite::Transaction<'_>,
    scope: DeletionScope<'_>,
) -> Result<Vec<GenerationJobEvent>, AppError> {
    let (column, id) = match scope {
        DeletionScope::Generation(id) => ("g.id", id),
        DeletionScope::Conversation(id) => ("c.id", id),
        DeletionScope::Project(id) => ("c.project_id", id),
    };
    let mut query = tx.prepare(&format!("SELECT j.id FROM generation_jobs j JOIN generations g ON g.id=j.generation_id JOIN conversations c ON c.id=g.conversation_id WHERE {column}=?1 AND g.deleted_at IS NULL AND c.deleted_at IS NULL AND j.status IN ('queued','running')"))?;
    let ids = query
        .query_map([id], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let timestamp = crate::current_timestamp();
    let mut events = Vec::with_capacity(ids.len());
    for id in ids {
        let job = generation_jobs::get_job_in_transaction(tx, &id)?;
        tx.execute("UPDATE generation_jobs SET cancel_requested_at=COALESCE(cancel_requested_at,?1) WHERE id=?2", rusqlite::params![timestamp,id])?;
        tx.execute(
            "DELETE FROM generation_recoveries WHERE generation_id=?1",
            [&job.generation_id],
        )?;
        generation_jobs::finish_job_in_transaction(
            tx,
            &generation_jobs::GenerationJobTerminalUpdate {
                job_id: id.clone(),
                expected_status: job.status,
                status: GenerationJobStatus::Cancelled,
                finished_at: timestamp.clone(),
                error_code: Some("cancelled_by_user".into()),
                error_message: None,
                retryable: false,
            },
        )?;
        events.push(generation_jobs::get_job_event_in_transaction(tx, &id)?);
    }
    Ok(events)
}

pub(crate) fn delete_with_jobs(
    app: &tauri::AppHandle<impl tauri::Runtime>,
    db: &Database,
    scope: DeletionScope<'_>,
    delete: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<(), AppError>,
) -> Result<(), AppError> {
    let _write = app
        .state::<crate::storage::MaintenanceGate>()
        .write()
        .map_err(|message| AppError::Validation { message })?;
    let events = {
        let mut conn = db.connection()?;
        let tx = generation_jobs::begin_generation_job_write_transaction(&mut conn)?;
        let events = cancel_for_deletion(&tx, scope)?;
        delete(&tx)?;
        tx.commit()?;
        events
    };
    for event in events {
        publish(app, &event);
    }
    Ok(())
}

fn publish_transition<T>(
    app: &tauri::AppHandle<impl tauri::Runtime>,
    transition: GenerationJobTransition<T>,
) -> T {
    if let Some(event) = transition.event {
        publish(app, &event);
    }
    transition.value
}

fn request_fingerprint(request: &GenerationExecutionRequest) -> Result<String, AppError> {
    // Compare caller intent before looking up defaults, credentials or external files.
    // Omit absent options so adding a new optional field preserves older fingerprints.
    let mut value = serde_json::to_value(request)?;
    if let Some(fields) = value.as_object_mut() {
        fields.retain(|_, value| !value.is_null());
    }
    value.sort_all_objects();
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(&value)?)))
}

fn replay_receipt(
    conn: &Connection,
    id: Option<&str>,
    fingerprint: &str,
) -> Result<Option<GenerationJobTransition<EnqueueGenerationResult>>, AppError> {
    let Some(id) = id else {
        return Ok(None);
    };
    let Some(job) = generation_jobs::find_job_by_client_request_id(conn, id)? else {
        return Ok(None);
    };
    let Some(original) = job.source_ref["request_fingerprint"].as_str() else {
        // Pre-runtime jobs are compared against the validated historical request below.
        return Ok(None);
    };
    if original != fingerprint {
        return Err(AppError::GenerationJobIdempotencyConflict);
    }
    Ok(Some(GenerationJobTransition {
        value: generation_jobs::enqueue_result_for_validated_job(&job)?,
        event: None,
    }))
}

fn enqueue(
    db: &Database,
    mut request: GenerationExecutionRequest,
    validate_sources: impl FnOnce(&mut GenerationExecutionRequest) -> Result<(), AppError>,
) -> Result<GenerationJobTransition<EnqueueGenerationResult>, AppError> {
    if request.prompt.trim().is_empty()
        || request.client_request_id.as_deref().is_some_and(|id| {
            id.trim().is_empty() || id.len() > 256 || id.chars().any(char::is_control)
        })
    {
        return Err(AppError::Validation {
            message: "A prompt and a valid request identity are required.".into(),
        });
    }
    let fingerprint = request_fingerprint(&request)?;
    if let Some(receipt) = replay_receipt(
        &*db.connection()?,
        request.client_request_id.as_deref(),
        &fingerprint,
    )? {
        return Ok(receipt);
    }
    validate_sources(&mut request)?;
    if let Some(id) = &request.document_id {
        let conn = db.connection()?;
        let project: String = conn
            .query_row(
                "SELECT project_id FROM canvas_documents WHERE id=?1 AND deleted_at IS NULL",
                [id],
                |row| row.get(0),
            )
            .map_err(|_| AppError::Validation {
                message: "Canvas document unavailable.".into(),
            })?;
        if request
            .project_id
            .as_ref()
            .is_some_and(|requested| requested != &project)
        {
            return Err(AppError::Validation {
                message: "Canvas document belongs to another project.".into(),
            });
        }
        request.project_id = Some(project);
    }
    let mut prepared = prepare_generation_job(db, &request)?;
    let mut conn = db.connection()?;
    if let Some(receipt) =
        replay_receipt(&conn, request.client_request_id.as_deref(), &fingerprint)?
    {
        return Ok(receipt);
    }
    prepared.source_ref["request_fingerprint"] = fingerprint.into();
    if let Some(parent) = prepared.parent_job_id.as_deref() {
        let tx = generation_jobs::begin_generation_job_write_transaction(&mut conn)?;
        let exists = generation_jobs::find_job_by_client_request_id_in_transaction(
            &tx,
            &prepared.client_request_id,
        )?
        .is_some();
        let value = generation_jobs::insert_retry_job_in_transaction(
            &tx,
            parent,
            &prepared.client_request_id,
            Some(&prepared.source_ref),
        )?;
        let event = if exists {
            None
        } else {
            Some(generation_jobs::get_job_event_in_transaction(
                &tx,
                &value.job_id,
            )?)
        };
        tx.commit()?;
        Ok(GenerationJobTransition { value, event })
    } else {
        generation_jobs::enqueue_job_with_event(&mut conn, &prepared)
    }
}

pub(crate) fn enqueue_request(
    app: &tauri::AppHandle<impl tauri::Runtime>,
    db: &Database,
    request: GenerationExecutionRequest,
    validate_sources: impl FnOnce(&mut GenerationExecutionRequest) -> Result<(), AppError>,
) -> Result<EnqueueGenerationResult, AppError> {
    let _write = app
        .state::<crate::storage::MaintenanceGate>()
        .write()
        .map_err(|message| AppError::Validation { message })?;
    let result = publish_transition(app, enqueue(db, request, validate_sources)?);
    app.state::<GenerationRuntime>().wake_worker.notify_one();
    Ok(result)
}

/// This loop starts once, after recovery and while the process owns the library
/// lock. A claim and its provider milestone are durable before the first HTTP byte.
pub(crate) fn start_worker(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let runtime = app.state::<GenerationRuntime>();
        loop {
            match process_next(&app).await {
                Ok(true) => continue,
                Ok(false) => runtime.wake_worker.notified().await,
                Err(error) => {
                    log::warn!("Generation worker deferred: {}", error.stable_code());
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_secs(2)) => {},
                        _ = runtime.wake_worker.notified() => {},
                    }
                }
            }
        }
    });
}

struct QueueTransition {
    event: GenerationJobEvent,
    execution: Option<crate::generation_lifecycle::GenerationExecutionSnapshot>,
}

/// Validate and snapshot each candidate inside its claim transaction. An old
/// malformed row cannot block later jobs or cause a second provider submission.
fn claim_next(db: &Database) -> Result<Option<QueueTransition>, AppError> {
    let mut conn = db.connection()?;
    let mut after: Option<(String, i64)> = None;
    loop {
        let tx = generation_jobs::begin_generation_job_write_transaction(&mut conn)?;
        let candidate = tx.query_row(
            "SELECT j.id,j.queued_at,j.rowid FROM generation_jobs j
             JOIN generations g ON g.id=j.generation_id AND g.deleted_at IS NULL
             JOIN conversations c ON c.id=g.conversation_id AND c.deleted_at IS NULL
             WHERE j.status='queued' AND j.stage='queued'
               AND json_extract(CASE WHEN json_valid(j.source_ref_json) THEN j.source_ref_json ELSE '{}' END,'$.execution_version')=1
               AND (?1 IS NULL OR (j.queued_at,j.rowid)>(?1,?2))
             ORDER BY j.queued_at,j.rowid LIMIT 1",
            rusqlite::params![after.as_ref().map(|(time,_)|time),after.as_ref().map(|(_,row)| row)],
            |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?, r.get::<_,i64>(2)?))
        ).optional()?;
        let Some((id, queued_at, rowid)) = candidate else {
            return Ok(None);
        };
        let claimed = (|| {
            let job = generation_jobs::claim_job_in_transaction(&tx, &id)?;
            generation_jobs::validate_worker_recovery_for_stage(
                &tx,
                &job.generation_id,
                GenerationJobStage::Preparing,
            )?;
            let snapshot =
                generation_jobs::load_generation_execution_snapshot_in_transaction(&tx, &id)?;
            let event = generation_jobs::get_job_event_in_transaction(&tx, &id)?;
            Ok((snapshot, event))
        })();
        match claimed {
            Ok(claimed) => {
                tx.commit()?;
                return Ok(Some(QueueTransition {
                    event: claimed.1,
                    execution: Some(claimed.0),
                }));
            }
            Err(
                AppError::GenerationJobCorruptPersistedData
                | AppError::GenerationJobInvalidSnapshot
                | AppError::GenerationJobInvalidTransition,
            ) => {
                tx.rollback()?;
                let tx = generation_jobs::begin_generation_job_write_transaction(&mut conn)?;
                match generation_jobs::retire_invalid_queue_record(&tx, &id) {
                    Ok(event) => {
                        tx.commit()?;
                        return Ok(Some(QueueTransition {
                            event,
                            execution: None,
                        }));
                    }
                    Err(
                        AppError::GenerationJobCorruptPersistedData
                        | AppError::GenerationJobInvalidSnapshot
                        | AppError::GenerationJobInvalidTransition,
                    ) => {
                        // Roll back even a partially attempted retirement. Another
                        // valid job must not depend on repairing this projection.
                        tx.rollback()?;
                        log::warn!("Skipped a queue record with unverifiable identity");
                    }
                    Err(error) => return Err(error),
                }
                after = Some((queued_at, rowid));
            }
            Err(error) => return Err(error),
        }
    }
}

async fn process_next(app: &tauri::AppHandle<impl tauri::Runtime>) -> Result<bool, AppError> {
    let runtime = app.state::<GenerationRuntime>();
    let _worker = runtime.execution.lock().await;
    let _write = app
        .state::<crate::storage::MaintenanceGate>()
        .write()
        .map_err(|message| AppError::Validation { message })?;
    let db = app.state::<Database>();
    let Some(transition) = claim_next(&db)? else {
        // A corrupt, unreadable receipt has no safe event payload. Wake earlier
        // waiters so their next read returns the same integrity error.
        runtime.changed.send_replace(());
        return Ok(false);
    };
    publish(app, &transition.event);
    let Some(snapshot) = transition.execution else {
        return Ok(true);
    };
    let job_id = transition.event.job_id;
    // Provider errors and local save failures are terminalized by the lifecycle.
    let root = app.state::<crate::storage::StoragePaths>();
    let cancellation = CancellationProbe::new();
    let engine = app.state::<crate::api_gateway::GptImageEngine>();
    let mut changes = runtime.changed.subscribe();
    let run = run_claimed_generation(app, &db, &engine, &root.assets, snapshot, &cancellation);
    tokio::pin!(run);
    let outcome = loop {
        if db
            .connection()?
            .query_row(
                "SELECT cancel_requested_at IS NOT NULL FROM generation_jobs WHERE id=?1",
                [&job_id],
                |row| row.get::<_, bool>(0),
            )
            .optional()?
            .unwrap_or(true)
        {
            cancellation.cancel();
        }
        tokio::select! {
            result = &mut run => break result,
            _ = changes.changed() => {},
        }
    };
    if let Err(error) = outcome {
        log::info!("Generation job {} ended: {}", job_id, error.stable_code());
        // A local commit may fail after the response is already durable. The
        // sole executor can reconcile just this job before it claims another.
        let running: bool = db.connection()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM generation_jobs WHERE id=?1 AND status='running')",
            [&job_id],
            |row| row.get(0),
        )?;
        if running {
            crate::generation_recovery::recover_ids(
                &db,
                &engine,
                &root.assets,
                std::slice::from_ref(&job_id),
                Some(app),
            )
            .await?;
            return Ok(true);
        }
    }
    publish_current(app, &job_id);
    Ok(true)
}

#[tauri::command]
pub(crate) fn get_generation_job(
    db: State<'_, Database>,
    job_id: String,
) -> Result<GenerationJob, AppError> {
    generation_jobs::get_job(&*db.connection()?, &job_id)
}

#[tauri::command]
pub(crate) fn list_generation_jobs(
    db: State<'_, Database>,
    filter: Option<GenerationJobFilter>,
) -> Result<GenerationJobPage, AppError> {
    generation_jobs::list_visible_jobs(&*db.connection()?, &filter.unwrap_or_default())
}

#[tauri::command]
pub(crate) fn cancel_generation_job(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    job_id: String,
) -> Result<GenerationJob, AppError> {
    let _write = app
        .state::<crate::storage::MaintenanceGate>()
        .write()
        .map_err(|message| AppError::Validation { message })?;
    let transition = generation_jobs::request_cancel_with_event(&*db.connection()?, &job_id)?;
    Ok(publish_transition(&app, transition))
}

fn read_result(db: &Database, id: &str) -> Result<Option<GenerateResult>, AppError> {
    let conn = db.connection()?;
    // A waiting compatibility caller can outlive a deletion from another window.
    let removed: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM generation_jobs j JOIN generations g ON g.id=j.generation_id LEFT JOIN conversations c ON c.id=g.conversation_id WHERE j.id=?1 AND (g.deleted_at IS NOT NULL OR c.deleted_at IS NOT NULL))", [id], |r| r.get(0))?;
    if removed {
        return Err(AppError::Validation {
            message: "The generation was removed.".into(),
        });
    }
    let job = generation_jobs::get_job(&conn, id)?;
    match job.status {
        GenerationJobStatus::Queued | GenerationJobStatus::Running => Ok(None),
        GenerationJobStatus::Cancelled => Err(AppError::GenerationJobCancelled),
        GenerationJobStatus::Completed => {
            let mut stmt =
                conn.prepare("SELECT * FROM images WHERE generation_id=?1 ORDER BY rowid")?;
            let images = stmt
                .query_map(
                    [&job.generation_id],
                    crate::collections::images::row_to_image,
                )?
                .collect::<rusqlite::Result<_>>()?;
            Ok(Some(GenerateResult {
                generation_id: job.generation_id,
                conversation_id: job.request["conversation_id"]
                    .as_str()
                    .ok_or(AppError::GenerationJobCorruptPersistedData)?
                    .into(),
                images,
            }))
        }
        _ => {
            let model = job.request["model"].as_str().unwrap_or_default().to_owned();
            Err(match job.error_code.as_deref() {
                Some("provider_outcome_unknown") => AppError::ProviderOutcomeUnknown {
                    message: generation_jobs::terminal_message_for_code("provider_outcome_unknown")
                        .into(),
                },
                Some("provider_configuration_invalid")
                    if generation_jobs::executable_provider_snapshot_is_valid(
                        &job.provider_kind,
                        &job.provider_profile_id,
                        &job.endpoint_snapshot,
                    ) =>
                {
                    AppError::ApiKeyNotSet { model }
                }
                Some("provider_profile_missing") => AppError::ProviderProfileNotFound { model },
                _ => AppError::Validation {
                    message: job
                        .error_message
                        .unwrap_or_else(|| "Image generation failed".into()),
                },
            })
        }
    }
}

pub(crate) async fn await_result(
    app: &tauri::AppHandle<impl tauri::Runtime>,
    db: &Database,
    job_id: &str,
) -> Result<GenerateResult, AppError> {
    let mut changes = app.state::<GenerationRuntime>().changed.subscribe();
    loop {
        if let Some(result) = read_result(db, job_id)? {
            return Ok(result);
        }
        changes
            .changed()
            .await
            .map_err(|_| AppError::GenerationJobInvalidTransition)?;
    }
}

#[tauri::command]
pub(crate) async fn wait_generation_result(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    job_id: String,
) -> Result<GenerateResult, AppError> {
    await_result(&app, &db, &job_id).await
}

#[cfg(test)]
#[path = "generation_execution_tests.rs"]
mod tests;
