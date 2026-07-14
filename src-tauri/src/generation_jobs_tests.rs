use super::*;
use crate::db::Database;
use crate::error::AppError;
use crate::generation_worker_lease::{
    acquire_worker_lease, WorkerLeaseAcquireOutcome, WorkerLeaseError, WorkerTransitionAuthority,
};
use crate::models::{GenerationJobFilter, GenerationJobStage, GenerationJobStatus};
use rusqlite::{params, Connection};
use serde_json::json;
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::time::Duration;

const WORKER_NOW_MS: i64 = 1_783_872_000_000;
const WORKER_NOW_TIMESTAMP: &str = "2026-07-12T16:00:00Z";

fn acquire_test_worker(
    conn: &Connection,
    owner_id: &str,
    now_ms: i64,
    ttl: Duration,
) -> WorkerTransitionAuthority {
    match acquire_worker_lease(conn, owner_id, now_ms, ttl).expect("acquire test worker lease") {
        WorkerLeaseAcquireOutcome::Acquired { authority, .. } => authority,
        WorkerLeaseAcquireOutcome::Held { expires } => {
            panic!("expected worker lease acquisition, held until {expires}")
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct WorkerDatabaseSnapshot {
    jobs: Vec<WorkerJobSnapshot>,
    generations: Vec<(String, String)>,
    recoveries: Vec<WorkerRecoverySnapshot>,
}

#[derive(Debug, PartialEq, Eq)]
struct WorkerJobSnapshot {
    id: String,
    status: String,
    stage: String,
    auto_attempt: i32,
    started_at: Option<String>,
    last_heartbeat_at: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct WorkerRecoverySnapshot {
    generation_id: String,
    request_state: String,
    expected_response_file: Option<String>,
    response_file: Option<String>,
    response_size: Option<i64>,
    response_sha256: Option<String>,
}

fn worker_database_snapshot(conn: &Connection) -> WorkerDatabaseSnapshot {
    let jobs = conn
        .prepare(
            "SELECT id, status, stage, auto_attempt, started_at, last_heartbeat_at
               FROM generation_jobs ORDER BY id",
        )
        .expect("prepare worker job snapshot")
        .query_map([], |row| {
            Ok(WorkerJobSnapshot {
                id: row.get(0)?,
                status: row.get(1)?,
                stage: row.get(2)?,
                auto_attempt: row.get(3)?,
                started_at: row.get(4)?,
                last_heartbeat_at: row.get(5)?,
            })
        })
        .expect("query worker job snapshot")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("decode worker job snapshot");
    let generations = conn
        .prepare("SELECT id, status FROM generations ORDER BY id")
        .expect("prepare worker generation snapshot")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query worker generation snapshot")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("decode worker generation snapshot");
    let recoveries = conn
        .prepare(
            "SELECT generation_id, request_state, expected_response_file, response_file,
                    response_size, response_sha256
               FROM generation_recoveries ORDER BY generation_id",
        )
        .expect("prepare worker recovery snapshot")
        .query_map([], |row| {
            Ok(WorkerRecoverySnapshot {
                generation_id: row.get(0)?,
                request_state: row.get(1)?,
                expected_response_file: row.get(2)?,
                response_file: row.get(3)?,
                response_size: row.get(4)?,
                response_sha256: row.get(5)?,
            })
        })
        .expect("query worker recovery snapshot")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("decode worker recovery snapshot");
    WorkerDatabaseSnapshot {
        jobs,
        generations,
        recoveries,
    }
}

fn assert_worker_transition_lease_lost<T>(result: Result<T, WorkerTransitionError>) {
    assert!(matches!(
        result,
        Err(WorkerTransitionError::Lease(WorkerLeaseError::LeaseLost))
    ));
}

fn prepare_fenced_retry_backoff(
    conn: &Connection,
    job: &crate::models::GenerationJob,
    authority: &WorkerTransitionAuthority,
) {
    let claimed = claim_next_job_fenced_with_event(conn, authority, WORKER_NOW_MS)
        .expect("claim retry fixture")
        .expect("retry fixture exists");
    assert_eq!(claimed.value.id, job.id);
    let expected_path = std::env::temp_dir()
        .join("astro-studio-responses")
        .join(format!("{}.response.json", job.generation_id));
    transition_running_job_stage_with_event(
        conn,
        &job.id,
        GenerationJobStage::Preparing,
        WorkerStageTransition::BeginProviderRequest {
            expected_response_file: expected_path,
        },
        authority,
        WORKER_NOW_MS + 1_000,
    )
    .expect("begin provider request for retry fixture");
    transition_running_job_stage_with_event(
        conn,
        &job.id,
        GenerationJobStage::ProviderRequest,
        WorkerStageTransition::EnterRetryBackoff,
        authority,
        WORKER_NOW_MS + 2_000,
    )
    .expect("enter retry backoff fixture");
}

struct TestDatabaseDirectory(PathBuf);

impl Drop for TestDatabaseDirectory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).ok();
    }
}

struct JobFixture {
    database: Database,
    path: PathBuf,
    _directory: TestDatabaseDirectory,
}

impl JobFixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "astro-studio-generation-job-repository-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&directory).expect("create test database directory");
        let path = directory.join("astro_studio.db");
        let database = Database::open(&path).expect("open test database");
        database.run_migrations().expect("run v16 migrations");
        Self {
            database,
            path,
            _directory: TestDatabaseDirectory(directory),
        }
    }

    fn open_connection(&self) -> Connection {
        let conn = Connection::open(&self.path).expect("open fixture connection");
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .expect("enable foreign keys");
        conn
    }

    fn prepared(&self, client_request_id: &str, operation: &str) -> PreparedGenerationJob {
        self.prepared_at(client_request_id, operation, "2026-07-10T00:00:00Z")
    }

    fn prepared_at(
        &self,
        client_request_id: &str,
        operation: &str,
        queued_at: &str,
    ) -> PreparedGenerationJob {
        PreparedGenerationJob {
            job_id: uuid::Uuid::new_v4().to_string(),
            client_request_id: client_request_id.to_string(),
            generation_id: uuid::Uuid::new_v4().to_string(),
            requested_conversation_id: None,
            requested_project_id: Some("default".to_string()),
            prompt: format!("prompt for {operation}"),
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
            source_ref: json!({ "id": operation }),
            provider_kind: "openai".to_string(),
            provider_profile_id: "profile-1".to_string(),
            endpoint_snapshot: "https://api.example.test/v1/images/generations".to_string(),
            status: GenerationJobStatus::Queued,
            chain_attempt: 1,
            auto_attempt: 0,
            max_auto_attempts: 2,
            queued_at: queued_at.to_string(),
            finished_at: None,
            error_code: None,
            error_message: None,
            retryable: false,
        }
    }

    fn enqueue(&self, client_request_id: &str, operation: &str) -> crate::models::GenerationJob {
        let request = self.prepared(client_request_id, operation);
        let result = {
            let mut conn = self.database.conn.lock().expect("lock database");
            enqueue_job(&mut conn, &request).expect("enqueue job")
        };
        self.get(&result.job_id)
    }

    fn enqueue_prepared(
        &self,
        request: &PreparedGenerationJob,
    ) -> Result<crate::models::EnqueueGenerationResult, AppError> {
        let mut conn = self.database.conn.lock().expect("lock database");
        enqueue_job(&mut conn, request)
    }

    fn get(&self, id: &str) -> crate::models::GenerationJob {
        let conn = self.database.conn.lock().expect("lock database");
        get_job(&conn, id).expect("get job")
    }

    fn get_result(
        &self,
        client_request_id: &str,
    ) -> Option<crate::models::EnqueueGenerationResult> {
        let conn = self.database.conn.lock().expect("lock database");
        find_enqueue_result_by_client_request_id(&conn, client_request_id)
            .expect("find enqueue result")
    }

    fn claim(&self) -> Option<crate::models::GenerationJob> {
        let mut conn = self.database.conn.lock().expect("lock database");
        claim_next_job(&mut conn).expect("claim next job")
    }

    fn fail_retryable(
        &self,
        client_request_id: &str,
        operation: &str,
    ) -> crate::models::GenerationJob {
        let queued = self.enqueue(client_request_id, operation);
        let claimed = self.claim().expect("claim queued job");
        assert_eq!(claimed.id, queued.id);
        let update = GenerationJobTerminalUpdate {
            job_id: queued.id.clone(),
            expected_status: GenerationJobStatus::Running,
            status: GenerationJobStatus::Failed,
            finished_at: "2026-07-10T00:00:02Z".to_string(),
            error_code: Some("provider_unavailable".to_string()),
            error_message: Some("The provider is temporarily unavailable".to_string()),
            retryable: true,
        };
        let conn = self.database.conn.lock().expect("lock database");
        finish_job(&conn, &update).expect("finish failed job")
    }

    fn count(&self, table: &str) -> i64 {
        let conn = self.database.conn.lock().expect("lock database");
        count_table(&conn, table)
    }

    fn generation_status(&self, generation_id: &str) -> String {
        let conn = self.database.conn.lock().expect("lock database");
        conn.query_row(
            "SELECT status FROM generations WHERE id = ?1",
            params![generation_id],
            |row| row.get(0),
        )
        .expect("read generation status")
    }

    fn list(&self, filter: &GenerationJobFilter) -> crate::models::GenerationJobPage {
        let conn = self.database.conn.lock().expect("lock database");
        list_jobs(&conn, filter).expect("list jobs")
    }
}

fn count_table(conn: &Connection, table: &str) -> i64 {
    assert!(matches!(
        table,
        "conversations"
            | "generations"
            | "generation_jobs"
            | "generation_recoveries"
            | "images"
            | "logs"
    ));
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .expect("count fixture table")
}

fn move_generation_conversation(
    fixture: &JobFixture,
    generation_id: &str,
    moved_project_id: &str,
    delete_original_project: bool,
) -> String {
    let conn = fixture.database.conn.lock().expect("lock database");
    conn.execute(
        "INSERT INTO projects (id, name) VALUES (?1, 'Moved Project')",
        params![moved_project_id],
    )
    .expect("insert moved project");
    let conversation_id = conn
        .query_row(
            "SELECT conversation_id FROM generations WHERE id = ?1",
            params![generation_id],
            |row| row.get::<_, String>(0),
        )
        .expect("read generation conversation");
    conn.execute(
        "UPDATE conversations SET project_id = ?1, updated_at = '2026-07-10T00:00:01Z'
         WHERE id = ?2 AND deleted_at IS NULL",
        params![moved_project_id, conversation_id],
    )
    .expect("move conversation");
    if delete_original_project {
        conn.execute(
            "UPDATE projects SET deleted_at = '2026-07-10T00:00:01Z'
             WHERE id = 'default'",
            [],
        )
        .expect("soft-delete original project");
    }
    conversation_id
}

fn stable_code(error: &AppError) -> &'static str {
    error.stable_code()
}

fn set_actual_image_count(conn: &Connection, generation_id: &str, actual_image_count: Option<u8>) {
    let raw: String = conn
        .query_row(
            "SELECT request_metadata FROM generations WHERE id = ?1",
            params![generation_id],
            |row| row.get(0),
        )
        .expect("read canonical generation metadata");
    let mut metadata: serde_json::Value =
        serde_json::from_str(&raw).expect("parse canonical generation metadata");
    let object = metadata.as_object_mut().expect("canonical metadata object");
    match actual_image_count {
        Some(count) => {
            object.insert("actual_image_count".to_string(), json!(count));
        }
        None => {
            object.remove("actual_image_count");
        }
    }
    conn.execute(
        "UPDATE generations SET request_metadata = ?1 WHERE id = ?2",
        params![
            serde_json::to_string(&metadata).expect("serialize canonical metadata"),
            generation_id
        ],
    )
    .expect("update canonical generation metadata");
}

fn insert_generation_images(conn: &Connection, generation_id: &str, count: u8) {
    for index in 0..count {
        conn.execute(
            "INSERT INTO images (
                id, generation_id, file_path, thumbnail_path, width, height, file_size, created_at
             ) VALUES (?1, ?2, ?3, ?4, 16, 16, 256, '2026-07-10T00:00:00Z')",
            params![
                format!("{generation_id}-image-{index}"),
                generation_id,
                format!("/managed/images/{generation_id}-{index}.png"),
                format!("/managed/thumbnails/{generation_id}-{index}.png"),
            ],
        )
        .expect("insert generation image projection");
    }
}

fn prepare_completed_projection(conn: &Connection, generation_id: &str, actual_count: u8) {
    insert_generation_images(conn, generation_id, actual_count);
    set_actual_image_count(conn, generation_id, Some(actual_count));
    assert_eq!(
        conn.execute(
            "DELETE FROM generation_recoveries WHERE generation_id = ?1",
            params![generation_id],
        )
        .expect("delete completed generation recovery"),
        1
    );
}

fn transition_fixture_job(
    fixture: &JobFixture,
    status: GenerationJobStatus,
    suffix: &str,
) -> crate::models::GenerationJob {
    let queued = fixture.enqueue(&format!("request-{suffix}"), suffix);
    match status {
        GenerationJobStatus::Queued => queued,
        GenerationJobStatus::Running => fixture.claim().expect("claim running fixture"),
        GenerationJobStatus::Failed | GenerationJobStatus::Interrupted => {
            fixture.claim().expect("claim terminal fixture");
            let conn = fixture.database.conn.lock().expect("lock database");
            finish_job(
                &conn,
                &GenerationJobTerminalUpdate {
                    job_id: queued.id,
                    expected_status: GenerationJobStatus::Running,
                    status,
                    finished_at: "2026-07-10T00:00:02Z".to_string(),
                    error_code: Some("provider_unavailable".to_string()),
                    error_message: None,
                    retryable: true,
                },
            )
            .expect("finish non-completed fixture")
        }
        GenerationJobStatus::Cancelled => {
            let conn = fixture.database.conn.lock().expect("lock database");
            request_cancel(&conn, &queued.id).expect("cancel queued fixture")
        }
        GenerationJobStatus::Completed => panic!("completed fixtures require image projection"),
    }
}

fn completed_projection_fixture(
    requested_count: u8,
    actual_count: u8,
    suffix: &str,
) -> (JobFixture, crate::models::GenerationJob) {
    let fixture = JobFixture::new();
    let mut request = fixture.prepared(&format!("request-{suffix}"), suffix);
    request.image_count = i32::from(requested_count);
    request.request_options.image_count = Some(requested_count);
    let queued = fixture
        .enqueue_prepared(&request)
        .expect("enqueue completed projection fixture");
    fixture.claim().expect("claim completed projection fixture");
    let completed = {
        let conn = fixture.database.conn.lock().expect("lock database");
        let tx = conn
            .unchecked_transaction()
            .expect("begin completed projection transaction");
        prepare_completed_projection(&tx, &queued.generation_id, actual_count);
        let completed = finish_job_in_transaction(
            &tx,
            &GenerationJobTerminalUpdate {
                job_id: queued.job_id,
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Completed,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: None,
                error_message: None,
                retryable: false,
            },
        )
        .expect("finish completed projection fixture");
        tx.commit().expect("commit completed projection fixture");
        completed
    };
    (fixture, completed)
}

#[test]
fn job_transition_matrix_allows_only_documented_edges() {
    use GenerationJobStatus::*;
    let statuses = [Queued, Running, Completed, Failed, Cancelled, Interrupted];

    for from in &statuses {
        for to in &statuses {
            let expected = matches!(
                (from, to),
                (Queued, Running)
                    | (Queued, Cancelled)
                    | (Running, Completed)
                    | (Running, Failed)
                    | (Running, Cancelled)
                    | (Running, Interrupted)
            );
            assert_eq!(
                can_transition(from.clone(), to.clone()),
                expected,
                "unexpected transition decision for {from:?} -> {to:?}"
            );
        }
    }
}

#[test]
fn repeated_client_request_returns_existing_before_any_duplicate_side_effect() {
    let fixture = JobFixture::new();
    let first_request = fixture.prepared("request-1", "same-operation");
    let first = fixture
        .enqueue_prepared(&first_request)
        .expect("first enqueue");
    let counts_after_first = [
        fixture.count("conversations"),
        fixture.count("generations"),
        fixture.count("generation_jobs"),
        fixture.count("generation_recoveries"),
        fixture.count("logs"),
    ];

    let repeated_request = fixture.prepared("request-1", "same-operation");
    let second = fixture
        .enqueue_prepared(&repeated_request)
        .expect("idempotent enqueue");

    assert_eq!(first.job_id, second.job_id);
    assert_eq!(first.generation_id, second.generation_id);
    assert_eq!(first.conversation_id, second.conversation_id);
    assert_eq!(
        counts_after_first,
        [
            fixture.count("conversations"),
            fixture.count("generations"),
            fixture.count("generation_jobs"),
            fixture.count("generation_recoveries"),
            fixture.count("logs"),
        ]
    );
}

#[test]
fn concurrent_root_enqueue_and_retry_wait_for_writer_then_return_existing_job() {
    let fixture = JobFixture::new();
    let enqueue_request = fixture.prepared("concurrent-enqueue", "concurrent-enqueue");
    let mut first_conn = fixture.open_connection();
    let first_tx = begin_generation_job_write_transaction(&mut first_conn)
        .expect("begin first immediate enqueue");
    let first = insert_job_in_transaction(&first_tx, &enqueue_request)
        .expect("insert first concurrent enqueue");

    let enqueue_path = fixture.path.clone();
    let competing_request = enqueue_request.clone();
    let competing_enqueue = std::thread::spawn(move || {
        let mut conn = Connection::open(enqueue_path).expect("open competing enqueue connection");
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .expect("configure competing enqueue connection");
        enqueue_job(&mut conn, &competing_request)
    });
    std::thread::sleep(std::time::Duration::from_millis(100));
    first_tx.commit().expect("commit first concurrent enqueue");
    let replay = competing_enqueue
        .join()
        .expect("join competing enqueue")
        .expect("concurrent enqueue must converge");
    assert_eq!(replay.job_id, first.job_id);
    assert_eq!(replay.generation_id, first.generation_id);
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        request_cancel(&conn, &first.job_id).expect("clear first queued fixture job");
    }

    let parent = fixture.fail_retryable("retry-parent", "retry-parent");
    let mut retry_conn = fixture.open_connection();
    let retry_tx = begin_generation_job_write_transaction(&mut retry_conn)
        .expect("begin first immediate retry");
    let first_retry =
        insert_retry_job_in_transaction(&retry_tx, &parent.id, "concurrent-retry", None)
            .expect("insert first concurrent retry");

    let retry_path = fixture.path.clone();
    let parent_id = parent.id.clone();
    let competing_retry = std::thread::spawn(move || {
        let mut conn = Connection::open(retry_path).expect("open competing retry connection");
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .expect("configure competing retry connection");
        create_retry_job(&mut conn, &parent_id, "concurrent-retry")
    });
    std::thread::sleep(std::time::Duration::from_millis(100));
    retry_tx.commit().expect("commit first concurrent retry");
    let retry_replay = competing_retry
        .join()
        .expect("join competing retry")
        .expect("concurrent retry must converge");
    assert_eq!(retry_replay.job_id, first_retry.job_id);
    assert_eq!(retry_replay.generation_id, first_retry.generation_id);
}

#[test]
fn duplicate_recheck_inside_outer_transaction_precedes_conversation_resolution() {
    let fixture = JobFixture::new();
    let first = fixture.enqueue("request-1", "same-operation");
    let repeated = fixture.prepared("request-1", "same-operation");

    let mut conn = fixture.open_connection();
    let tx = conn.transaction().expect("begin outer transaction");
    let result = insert_job_in_transaction(&tx, &repeated).expect("idempotent insert");
    assert_eq!(result.job_id, first.id);
    assert_eq!(count_table(&tx, "conversations"), 1);
    assert_eq!(count_table(&tx, "generations"), 1);
    tx.commit().expect("commit duplicate transaction");
}

#[test]
fn root_idempotency_compares_original_requested_conversation_and_project_identity() {
    let fixture = JobFixture::new();
    let first = fixture.enqueue("request-1", "same-operation");
    let counts = (
        fixture.count("conversations"),
        fixture.count("generations"),
        fixture.count("generation_jobs"),
        fixture.count("generation_recoveries"),
    );

    for conflicting in [
        {
            let mut request = fixture.prepared("request-1", "same-operation");
            request.requested_conversation_id = Some("different-conversation".to_string());
            request
        },
        {
            let mut request = fixture.prepared("request-1", "same-operation");
            request.requested_project_id = Some("different-project".to_string());
            request
        },
    ] {
        let error = fixture
            .enqueue_prepared(&conflicting)
            .expect_err("different requested identity must conflict");
        assert_eq!(stable_code(&error), "generation_job_idempotency_conflict");
    }

    assert_eq!(fixture.get(&first.id).id, first.id);
    assert_eq!(
        counts,
        (
            fixture.count("conversations"),
            fixture.count("generations"),
            fixture.count("generation_jobs"),
            fixture.count("generation_recoveries"),
        )
    );
}

#[test]
fn root_idempotency_replays_matching_absent_requested_identity() {
    let fixture = JobFixture::new();
    let mut first_request = fixture.prepared("request-1", "absent-identity");
    first_request.requested_conversation_id = None;
    first_request.requested_project_id = None;
    let first = fixture
        .enqueue_prepared(&first_request)
        .expect("enqueue absent identity");

    let mut replay = fixture.prepared("request-1", "absent-identity");
    replay.requested_conversation_id = None;
    replay.requested_project_id = None;
    let second = fixture
        .enqueue_prepared(&replay)
        .expect("replay matching absent identity");

    assert_eq!(first.job_id, second.job_id);
    assert_eq!(fixture.count("generation_jobs"), 1);
    let job = fixture.get(&first.job_id);
    assert!(job.request.get("requested_conversation_id").is_none());
    assert!(job.request.get("requested_project_id").is_none());
}

#[test]
fn resolved_default_project_rewrites_request_metadata_and_all_source_refs() {
    let fixture = JobFixture::new();

    for (index, (source_kind, request_kind, source_ref)) in [
        ("generate", "generate", json!({ "id": "generate-source" })),
        ("edit", "edit", json!({ "id": "edit-source" })),
        (
            "canvas",
            "generate",
            json!({
                "id": "canvas-source",
                "round_id": "round-1",
                "document_id": "document-1",
                "revision_id": "revision-1",
                "conversation_id": "untrusted-conversation",
                "project_id": "untrusted-project"
            }),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut request = fixture.prepared(&format!("request-{index}"), source_kind);
        request.requested_project_id = Some("missing-project".to_string());
        request.request_kind = request_kind.to_string();
        request.source_kind = source_kind.to_string();
        request.source_ref = source_ref;

        let result = fixture.enqueue_prepared(&request).expect("enqueue job");
        let job = fixture.get(&result.job_id);
        assert_eq!(
            job.request["conversation_id"],
            json!(result.conversation_id)
        );
        assert_eq!(job.request["project_id"], json!("default"));
        assert_eq!(
            job.request["requested_conversation_id"],
            serde_json::Value::Null
        );
        assert_eq!(
            job.request["requested_project_id"],
            json!("missing-project")
        );
        assert_eq!(
            job.source_ref["conversation_id"],
            json!(result.conversation_id)
        );
        assert_eq!(job.source_ref["project_id"], json!("default"));
        if source_kind == "canvas" {
            assert_eq!(job.source_ref["round_id"], json!("round-1"));
            assert_eq!(job.source_ref["document_id"], json!("document-1"));
            assert_eq!(job.source_ref["revision_id"], json!("revision-1"));
        }

        let conn = fixture.database.conn.lock().expect("lock database");
        let identities: (String, String, String) = conn
            .query_row(
                "SELECT c.project_id, g.conversation_id, g.request_metadata
                 FROM generations g
                 JOIN conversations c ON c.id = g.conversation_id
                 WHERE g.id = ?1",
                params![result.generation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("read persisted identities");
        let metadata: Value =
            serde_json::from_str(&identities.2).expect("parse canonical metadata");
        assert_eq!(identities.0, "default");
        assert_eq!(identities.1, result.conversation_id);
        assert_eq!(metadata["conversation_id"], json!(result.conversation_id));
        assert_eq!(metadata["project_id"], json!("default"));
    }
}

#[test]
fn existing_conversation_project_overrides_mismatched_requested_project_everywhere() {
    let fixture = JobFixture::new();
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            "INSERT INTO projects (id, name) VALUES ('actual-project', 'Actual')",
            [],
        )
        .expect("insert actual project");
        conn.execute(
            "INSERT INTO projects (id, name) VALUES ('requested-project', 'Requested')",
            [],
        )
        .expect("insert requested project");
        conn.execute(
            "INSERT INTO conversations (id, project_id, title)
             VALUES ('conversation-1', 'actual-project', 'Existing')",
            [],
        )
        .expect("insert conversation");
        conn.execute(
            "INSERT INTO generations (id, prompt, engine, status, conversation_id)
             VALUES ('existing-generation', 'existing', 'gpt-image-2', 'completed', 'conversation-1')",
            [],
        )
        .expect("insert existing generation");
    }

    let mut request = fixture.prepared("request-1", "identity-mismatch");
    request.requested_conversation_id = Some("conversation-1".to_string());
    request.requested_project_id = Some("requested-project".to_string());
    request.source_ref = json!({
        "id": "source-1",
        "conversation_id": "untrusted-conversation",
        "project_id": "untrusted-project"
    });
    let result = fixture.enqueue_prepared(&request).expect("enqueue job");
    let job = fixture.get(&result.job_id);

    assert_eq!(result.conversation_id, "conversation-1");
    assert_eq!(job.request["conversation_id"], json!("conversation-1"));
    assert_eq!(job.request["project_id"], json!("actual-project"));
    assert_eq!(
        job.request["requested_conversation_id"],
        json!("conversation-1")
    );
    assert_eq!(
        job.request["requested_project_id"],
        json!("requested-project")
    );
    assert_eq!(job.source_ref["conversation_id"], json!("conversation-1"));
    assert_eq!(job.source_ref["project_id"], json!("actual-project"));

    let conn = fixture.database.conn.lock().expect("lock database");
    let metadata_json: String = conn
        .query_row(
            "SELECT request_metadata FROM generations WHERE id = ?1",
            params![result.generation_id],
            |row| row.get(0),
        )
        .expect("read metadata");
    let metadata: Value = serde_json::from_str(&metadata_json).expect("parse metadata");
    assert_eq!(metadata["conversation_id"], json!("conversation-1"));
    assert_eq!(metadata["project_id"], json!("actual-project"));
}

#[test]
fn moving_conversation_preserves_immutable_job_snapshot_across_repository_operations() {
    let queued_fixture = JobFixture::new();
    let queued_request = queued_fixture.prepared("move-queued", "move-queued");
    let queued_result = queued_fixture
        .enqueue_prepared(&queued_request)
        .expect("enqueue queued move fixture");
    let conversation_id = move_generation_conversation(
        &queued_fixture,
        &queued_result.generation_id,
        "moved-project",
        true,
    );

    {
        let conn = queued_fixture.database.conn.lock().expect("lock database");
        let job = get_job(&conn, &queued_result.job_id).expect("get moved queued job");
        assert_eq!(job.request["conversation_id"], json!(conversation_id));
        assert_eq!(job.request["project_id"], json!("default"));
        assert_eq!(job.source_ref["project_id"], json!("default"));
        assert_eq!(
            list_jobs(
                &conn,
                &GenerationJobFilter {
                    generation_id: Some(queued_result.generation_id.clone()),
                    ..GenerationJobFilter::default()
                },
            )
            .expect("list moved queued job")
            .items
            .len(),
            1
        );
        assert_eq!(
            find_enqueue_result_by_client_request_id(&conn, "move-queued")
                .expect("ack moved queued job")
                .expect("moved enqueue result")
                .conversation_id,
            conversation_id
        );
    }

    let replay = queued_fixture
        .enqueue_prepared(&queued_request)
        .expect("idempotently replay moved queued job");
    assert_eq!(replay.job_id, queued_result.job_id);
    assert_eq!(
        queued_fixture.claim().expect("claim moved queued job").id,
        queued_result.job_id
    );

    let cancelled_fixture = JobFixture::new();
    let cancelled = cancelled_fixture.enqueue("move-cancel", "move-cancel");
    move_generation_conversation(
        &cancelled_fixture,
        &cancelled.generation_id,
        "cancelled-moved-project",
        false,
    );
    let cancelled_job = {
        let conn = cancelled_fixture
            .database
            .conn
            .lock()
            .expect("lock database");
        request_cancel(&conn, &cancelled.id).expect("cancel moved queued job")
    };
    assert_eq!(cancelled_job.status, GenerationJobStatus::Cancelled);

    let running_fixture = JobFixture::new();
    let running = running_fixture.enqueue("move-running", "move-running");
    running_fixture.claim().expect("claim running move fixture");
    move_generation_conversation(
        &running_fixture,
        &running.generation_id,
        "running-moved-project",
        false,
    );
    let finished = {
        let conn = running_fixture.database.conn.lock().expect("lock database");
        finish_job(
            &conn,
            &GenerationJobTerminalUpdate {
                job_id: running.id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Failed,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: Some("provider_unavailable".to_string()),
                error_message: Some("raw provider detail must be ignored".to_string()),
                retryable: true,
            },
        )
        .expect("finish moved running job")
    };
    assert_eq!(finished.status, GenerationJobStatus::Failed);

    let retry = {
        let mut conn = running_fixture.database.conn.lock().expect("lock database");
        create_retry_job(&mut conn, &finished.id, "move-running-retry")
            .expect("retry moved terminal job")
    };
    let retry_job = running_fixture.get(&retry.job_id);
    assert_eq!(retry_job.request["project_id"], json!("default"));
    assert_eq!(retry_job.source_ref["project_id"], json!("default"));

    let running_cancel_fixture = JobFixture::new();
    let running_cancel =
        running_cancel_fixture.enqueue("move-running-cancel", "move-running-cancel");
    running_cancel_fixture
        .claim()
        .expect("claim running cancellation fixture");
    move_generation_conversation(
        &running_cancel_fixture,
        &running_cancel.generation_id,
        "running-cancel-moved-project",
        false,
    );
    let cancel_requested = {
        let conn = running_cancel_fixture
            .database
            .conn
            .lock()
            .expect("lock database");
        request_cancel(&conn, &running_cancel.id).expect("request moved running cancellation")
    };
    assert_eq!(cancel_requested.status, GenerationJobStatus::Running);
    assert!(cancel_requested.cancel_requested_at.is_some());
    let cancelled = {
        let conn = running_cancel_fixture
            .database
            .conn
            .lock()
            .expect("lock database");
        finish_job(
            &conn,
            &GenerationJobTerminalUpdate {
                job_id: running_cancel.id,
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Cancelled,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: None,
                error_message: None,
                retryable: false,
            },
        )
        .expect("acknowledge moved running cancellation")
    };
    assert_eq!(cancelled.status, GenerationJobStatus::Cancelled);

    let deleted_conversation_fixture = JobFixture::new();
    let deleted_conversation =
        deleted_conversation_fixture.enqueue("deleted-conversation", "deleted-conversation");
    {
        let conn = deleted_conversation_fixture
            .database
            .conn
            .lock()
            .expect("lock database");
        conn.execute(
            "UPDATE conversations SET deleted_at = '2026-07-10T00:00:03Z'
             WHERE id = (SELECT conversation_id FROM generations WHERE id = ?1)",
            params![deleted_conversation.generation_id],
        )
        .expect("soft-delete linked conversation");
        let error = get_job(&conn, &deleted_conversation.id)
            .expect_err("deleted linked conversation remains corrupt");
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    }
}

#[test]
fn composable_enqueue_outer_rollback_removes_conversation_generation_recovery_and_job() {
    let fixture = JobFixture::new();
    let mut conn = fixture.open_connection();
    let tx = conn.transaction().expect("begin outer transaction");
    insert_job_in_transaction(&tx, &fixture.prepared("request-1", "rollback"))
        .expect("insert in outer transaction");
    assert_eq!(count_table(&tx, "conversations"), 1);
    assert_eq!(count_table(&tx, "generations"), 1);
    assert_eq!(count_table(&tx, "generation_recoveries"), 1);
    assert_eq!(count_table(&tx, "generation_jobs"), 1);
    tx.rollback().expect("rollback outer transaction");

    assert_eq!(fixture.count("conversations"), 0);
    assert_eq!(fixture.count("generations"), 0);
    assert_eq!(fixture.count("generation_recoveries"), 0);
    assert_eq!(fixture.count("generation_jobs"), 0);
}

#[test]
fn composable_enqueue_outer_rollback_restores_updated_conversation() {
    let fixture = JobFixture::new();
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            "INSERT INTO conversations (id, project_id, title) VALUES ('conversation-1', 'default', 'Original title')",
            [],
        )
        .expect("insert conversation");
    }
    let mut request = fixture.prepared("request-1", "updated-conversation");
    request.requested_conversation_id = Some("conversation-1".to_string());

    let mut conn = fixture.open_connection();
    let tx = conn.transaction().expect("begin outer transaction");
    let result = insert_job_in_transaction(&tx, &request).expect("insert job");
    assert_eq!(result.conversation_id, "conversation-1");
    let changed_title: String = tx
        .query_row(
            "SELECT title FROM conversations WHERE id = 'conversation-1'",
            [],
            |row| row.get(0),
        )
        .expect("read updated title");
    assert_ne!(changed_title, "Original title");
    tx.rollback().expect("rollback outer transaction");

    let conn = fixture.database.conn.lock().expect("lock database");
    let restored_title: String = conn
        .query_row(
            "SELECT title FROM conversations WHERE id = 'conversation-1'",
            [],
            |row| row.get(0),
        )
        .expect("read restored title");
    assert_eq!(restored_title, "Original title");
}

#[test]
fn initial_provider_configuration_failure_is_inserted_atomically_as_terminal() {
    let fixture = JobFixture::new();
    let mut request = fixture.prepared("request-1", "missing-profile");
    request.status = GenerationJobStatus::Failed;
    request.provider_kind = "unresolved".to_string();
    request.provider_profile_id = "unresolved".to_string();
    request.endpoint_snapshot.clear();
    request.finished_at = Some(request.queued_at.clone());
    request.error_code = Some("provider_profile_missing".to_string());
    request.error_message = Some("Bearer sk-secret from a provider response".to_string());

    let result = fixture
        .enqueue_prepared(&request)
        .expect("persist failed enqueue");
    let job = fixture.get(&result.job_id);
    assert_eq!(result.status, GenerationJobStatus::Failed);
    assert_eq!(job.status, GenerationJobStatus::Failed);
    assert_eq!(job.stage, GenerationJobStage::Terminal);
    assert_eq!(job.finished_at.as_deref(), Some(request.queued_at.as_str()));
    assert_eq!(job.error_code.as_deref(), Some("provider_profile_missing"));
    assert_eq!(
        job.error_message.as_deref(),
        Some("The selected provider profile is unavailable")
    );
    assert!(!serde_json::to_string(&job)
        .expect("serialize failed job")
        .contains("sk-secret"));
    assert!(!job.retryable);
    assert!(!result.retryable);
    assert_eq!(result.error_code, job.error_code);
    assert_eq!(result.error_message, job.error_message);
    assert_eq!(result.queued_at, job.queued_at);
    assert_eq!(result.finished_at, job.finished_at);
    assert_eq!(fixture.generation_status(&job.generation_id), "failed");
    assert_eq!(fixture.count("generation_recoveries"), 0);
    assert!(fixture.claim().is_none());
}

#[test]
fn secret_bearing_endpoint_snapshot_is_rejected_without_persistence_or_leakage() {
    let fixture = JobFixture::new();
    for (index, endpoint) in [
        "https://user:secret-key@example.test/images",
        "https://example.test/images?x%2Dapi%2Dkey=secret-key",
        "https://example.test/images?client_secret=secret-key",
        "https://example.test/images?routing=sk-secret",
        "https://example.test/images#access_token=secret-key",
    ]
    .into_iter()
    .enumerate()
    {
        let mut request = fixture.prepared(&format!("endpoint-request-{index}"), "secret-endpoint");
        request.endpoint_snapshot = endpoint.to_string();
        let error = fixture
            .enqueue_prepared(&request)
            .expect_err("secret-bearing endpoint snapshots must fail");
        assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
        assert!(!error.to_string().contains("secret-key"));
    }
    assert_eq!(fixture.count("conversations"), 0);
    assert_eq!(fixture.count("generations"), 0);
    assert_eq!(fixture.count("generation_jobs"), 0);
}

#[test]
fn unknown_source_reference_keys_are_rejected_before_persistence() {
    let fixture = JobFixture::new();
    for (index, source_ref) in [
        json!({ "unknown_id": "value" }),
        json!({ "client_secret": "secret-key" }),
        json!({ "id": { "nested": "value" } }),
    ]
    .into_iter()
    .enumerate()
    {
        let mut request = fixture.prepared(&format!("request-{index}"), "bad-source-ref");
        request.source_ref = source_ref;
        let error = fixture
            .enqueue_prepared(&request)
            .expect_err("unknown source reference fields must fail");
        assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
        assert!(!error.to_string().contains("secret-key"));
    }
    assert_eq!(fixture.count("generation_jobs"), 0);
}

#[test]
fn safe_custom_endpoint_queries_and_ordinary_prose_are_preserved() {
    let fixture = JobFixture::new();
    for (index, (endpoint, prompt)) in [
        (
            "https://api.example.test/images?api-version=2026-01-01",
            "Paint a ring bearer standing in a garden",
        ),
        (
            "https://api.example.test/images?routing=primary&region=west",
            "Illustrate Aizawa-kun reading under a tree",
        ),
        (
            "https://api.example.test/images?routing=secondary",
            "Place Aizawa-kun-reading-under-a-tree.png in a collage",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut request = fixture.prepared(&format!("request-{index}"), "safe-query");
        request.endpoint_snapshot = endpoint.to_string();
        request.prompt = prompt.to_string();
        let result = fixture.enqueue_prepared(&request).expect("safe snapshot");
        let job = fixture.get(&result.job_id);
        assert_eq!(job.endpoint_snapshot, endpoint);
        assert_eq!(job.request["prompt"], json!(request.prompt));
    }
}

#[test]
fn credential_token_patterns_are_rejected_from_all_public_snapshot_channels() {
    let fixture = JobFixture::new();
    let mut requests = Vec::new();

    let mut prompt = fixture.prepared("request-prompt", "secret-prompt");
    prompt.prompt = "Render the literal header Bearer sk-example".to_string();
    requests.push(prompt);

    let mut opaque_bearer = fixture.prepared("request-bearer", "secret-bearer");
    opaque_bearer.prompt = "Authorization: Bearer abcdefgh1234".to_string();
    requests.push(opaque_bearer);

    let mut rfc_bearer = fixture.prepared("request-rfc-bearer", "secret-rfc-bearer");
    rfc_bearer.prompt = "Authorization: Bearer abc/defghijklmnop==".to_string();
    requests.push(rfc_bearer);

    let mut model = fixture.prepared("request-model", "secret-model");
    model.model = "sk_model-secret".to_string();
    requests.push(model);

    let mut profile = fixture.prepared("request-profile", "secret-profile");
    profile.provider_profile_id = "ghp_secret-profile".to_string();
    requests.push(profile);

    let mut path = fixture.prepared("request-path", "secret-path");
    path.source_image_paths = vec!["/tmp/github_pat_secret-source.png".to_string()];
    requests.push(path);

    let mut source_ref = fixture.prepared("request-source-ref", "secret-source-ref");
    source_ref.source_ref = json!({ "id": format!("AIza{}", "A".repeat(35)) });
    requests.push(source_ref);

    let mut client_request = fixture.prepared("request-client", "secret-client-request");
    client_request.client_request_id = "xoxb-secret-request".to_string();
    requests.push(client_request);

    let mut job_id = fixture.prepared("request-job-id", "secret-job-id");
    job_id.job_id = "eyJhbGciOiJIUzI1NiJ9.payload.signature".to_string();
    requests.push(job_id);

    let mut generation_id = fixture.prepared("request-generation-id", "secret-generation-id");
    generation_id.generation_id = "sk-secret-generation".to_string();
    requests.push(generation_id);

    let mut conversation_id = fixture.prepared("request-conversation-id", "secret-conversation-id");
    conversation_id.requested_conversation_id = Some("ghp_secret-conversation".to_string());
    requests.push(conversation_id);

    let mut project_id = fixture.prepared("request-project-id", "secret-project-id");
    project_id.requested_project_id = Some("github_pat_secret-project".to_string());
    requests.push(project_id);

    let mut endpoint = fixture.prepared("request-endpoint", "secret-endpoint-path");
    endpoint.endpoint_snapshot = "https://api.example.test/xoxp-secret-token/images".to_string();
    requests.push(endpoint);

    for request in requests {
        let error = fixture
            .enqueue_prepared(&request)
            .expect_err("credential-shaped public text must be rejected");
        assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
    }

    assert_eq!(fixture.count("conversations"), 0);
    assert_eq!(fixture.count("generations"), 0);
    assert_eq!(fixture.count("generation_jobs"), 0);
}

#[test]
fn credential_tokens_in_injected_persisted_public_fields_are_reported_as_corruption() {
    for case in 0..4 {
        let fixture = JobFixture::new();
        let queued = fixture.enqueue("request-1", &format!("persisted-token-{case}"));
        let conn = fixture.database.conn.lock().expect("lock database");
        match case {
            0 => {
                conn.execute(
                    "UPDATE generation_jobs SET provider_profile_id = 'ghp_secret-profile'
                     WHERE id = ?1",
                    params![queued.id],
                )
                .expect("inject profile token");
            }
            1 => {
                conn.execute(
                    "UPDATE generations SET prompt = 'Authorization: Bearer abcdefgh1234' WHERE id = ?1",
                    params![queued.generation_id],
                )
                .expect("inject generation prompt token");
            }
            2 => {
                conn.execute(
                    "UPDATE generation_jobs
                     SET source_ref_json = '{\"id\":\"xoxb-secret-source\",\"conversation_id\":\"safe-conversation\",\"project_id\":\"default\"}'
                     WHERE id = ?1",
                    params![queued.id],
                )
                .expect("inject source reference token");
            }
            3 => {
                let mut request = queued.request.clone();
                request["requested_project_id"] = json!("github_pat_secret-project");
                conn.execute(
                    "UPDATE generation_jobs SET request_json = ?1 WHERE id = ?2",
                    params![
                        serde_json::to_string(&request).expect("serialize corrupt request"),
                        queued.id
                    ],
                )
                .expect("inject requested identity token");
            }
            _ => unreachable!(),
        }
        let error = get_job(&conn, &queued.id).expect_err("persisted token must not project");
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        assert!(!error.to_string().contains("secret"));
    }
}

#[test]
fn persisted_snapshot_and_public_row_are_secret_free() {
    let fixture = JobFixture::new();
    let job = fixture.enqueue("request-1", "public-snapshot");
    let serialized = serde_json::to_string(&job).expect("serialize job");
    assert!(!serialized.contains("api_key"));
    assert!(!serialized.contains("secret-key"));
    assert!(job.request["conversation_id"].as_str().is_some());
    let mut request_keys = job
        .request
        .as_object()
        .expect("canonical request object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    request_keys.sort_unstable();
    assert_eq!(
        request_keys,
        [
            "conversation_id",
            "kind",
            "model",
            "options",
            "project_id",
            "prompt",
            "requested_project_id",
            "source_image_paths",
        ]
    );
    let mut option_keys = job.request["options"]
        .as_object()
        .expect("canonical options object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    option_keys.sort_unstable();
    assert_eq!(
        option_keys,
        [
            "background",
            "image_count",
            "input_fidelity",
            "moderation",
            "output_compression",
            "output_format",
            "partial_images",
            "quality",
            "size",
            "stream",
        ]
    );
    assert_eq!(
        job.source_ref["conversation_id"],
        job.request["conversation_id"]
    );
}

#[test]
fn prepared_snapshot_rejects_inconsistent_initial_status_fields() {
    let fixture = JobFixture::new();

    let mut queued_with_terminal = fixture.prepared("request-1", "queued-terminal");
    queued_with_terminal.finished_at = Some(queued_with_terminal.queued_at.clone());
    queued_with_terminal.error_code = Some("unexpected".to_string());

    let mut queued_unresolved = fixture.prepared("request-2", "queued-unresolved");
    queued_unresolved.provider_kind = "unresolved".to_string();
    queued_unresolved.provider_profile_id = "unresolved".to_string();
    queued_unresolved.endpoint_snapshot.clear();

    let mut failed_without_terminal = fixture.prepared("request-3", "failed-incomplete");
    failed_without_terminal.status = GenerationJobStatus::Failed;
    failed_without_terminal.provider_kind = "unresolved".to_string();
    failed_without_terminal.provider_profile_id = "unresolved".to_string();
    failed_without_terminal.endpoint_snapshot.clear();

    for invalid in [
        queued_with_terminal,
        queued_unresolved,
        failed_without_terminal,
    ] {
        let error = fixture
            .enqueue_prepared(&invalid)
            .expect_err("inconsistent initial snapshot must fail");
        assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
    }
    assert_eq!(fixture.count("conversations"), 0);
    assert_eq!(fixture.count("generations"), 0);
    assert_eq!(fixture.count("generation_jobs"), 0);
}

#[test]
fn root_enqueue_rejects_fabricated_retry_lineage_and_attempts() {
    let fixture = JobFixture::new();
    let mut request = fixture.prepared("request-1", "fabricated-lineage");
    request.parent_job_id = Some("fabricated-parent".to_string());
    request.chain_attempt = 2;
    request.auto_attempt = 1;

    let error = fixture
        .enqueue_prepared(&request)
        .expect_err("root enqueue cannot fabricate retry state");
    assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
    assert_eq!(fixture.count("conversations"), 0);
    assert_eq!(fixture.count("generations"), 0);
    assert_eq!(fixture.count("generation_jobs"), 0);
}

#[test]
fn initial_failed_snapshot_accepts_only_configuration_error_codes() {
    let fixture = JobFixture::new();
    let mut request = fixture.prepared("request-1", "invalid-initial-failure");
    request.status = GenerationJobStatus::Failed;
    request.finished_at = Some(request.queued_at.clone());
    request.error_code = Some("provider_unavailable".to_string());
    request.error_message = Some("The provider is unavailable".to_string());

    let error = fixture
        .enqueue_prepared(&request)
        .expect_err("runtime failures cannot bypass the running state");
    assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
    assert_eq!(fixture.count("generations"), 0);
    assert_eq!(fixture.count("generation_jobs"), 0);
}

#[test]
fn initial_configuration_failure_requires_complete_sentinel_or_public_provider_snapshot() {
    let fixture = JobFixture::new();
    let mut request = fixture.prepared("request-1", "incomplete-provider-snapshot");
    request.status = GenerationJobStatus::Failed;
    request.finished_at = Some(request.queued_at.clone());
    request.error_code = Some("provider_configuration_invalid".to_string());
    request.endpoint_snapshot.clear();

    let error = fixture
        .enqueue_prepared(&request)
        .expect_err("known provider identity requires a nonempty endpoint");
    assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
    assert_eq!(fixture.count("conversations"), 0);
    assert_eq!(fixture.count("generations"), 0);
    assert_eq!(fixture.count("generation_jobs"), 0);
}

#[test]
fn persisted_generation_matches_normalized_snapshot_and_job_timestamp() {
    let fixture = JobFixture::new();
    let request = fixture.prepared("request-1", "normalized-generation");
    let result = fixture
        .enqueue_prepared(&request)
        .expect("enqueue normalized job");
    let conn = fixture.database.conn.lock().expect("lock database");
    let persisted = conn
        .query_row(
            "SELECT prompt, engine, request_kind, size, quality, background, output_format,
                    output_compression, moderation, input_fidelity, image_count,
                    source_image_count, source_image_paths, request_metadata, status,
                    conversation_id, created_at
             FROM generations WHERE id = ?1",
            params![result.generation_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i32>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, i32>(10)?,
                    row.get::<_, i32>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, Option<String>>(13)?,
                    row.get::<_, String>(14)?,
                    row.get::<_, Option<String>>(15)?,
                    row.get::<_, String>(16)?,
                ))
            },
        )
        .expect("read persisted generation");

    assert_eq!(persisted.0, request.prompt);
    assert_eq!(persisted.1, request.model);
    assert_eq!(persisted.2, request.request_kind);
    assert_eq!(persisted.3, request.size);
    assert_eq!(persisted.4, request.quality);
    assert_eq!(persisted.5, request.background);
    assert_eq!(persisted.6, request.output_format);
    assert_eq!(persisted.7, request.output_compression);
    assert_eq!(persisted.8, request.moderation);
    assert_eq!(persisted.9, request.input_fidelity);
    assert_eq!(persisted.10, request.image_count);
    assert_eq!(persisted.11, request.source_image_paths.len() as i32);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&persisted.12).expect("valid source paths"),
        json!(request.source_image_paths)
    );
    assert!(serde_json::from_str::<serde_json::Value>(
        persisted.13.as_deref().expect("request metadata")
    )
    .is_ok());
    assert_eq!(persisted.14, "queued");
    assert_eq!(
        persisted.15.as_deref(),
        Some(result.conversation_id.as_str())
    );
    assert_eq!(persisted.16, request.queued_at);
    assert_eq!(result.queued_at, request.queued_at);
}

#[test]
fn fenced_claim_uses_explicit_time_and_returns_the_committed_fifo_event() {
    let fixture = JobFixture::new();
    let first = fixture.enqueue("fenced-claim-first", "fenced-claim-first");
    let second = fixture.enqueue("fenced-claim-second", "fenced-claim-second");
    let conn = fixture.database.conn.lock().expect("lock database");
    let authority = acquire_test_worker(
        &conn,
        "fenced-claim-worker",
        WORKER_NOW_MS,
        Duration::from_secs(60),
    );

    let transition = claim_next_job_fenced_with_event(&conn, &authority, WORKER_NOW_MS)
        .expect("claim with current fenced authority")
        .expect("queued job must be claimed");

    assert_eq!(transition.value.id, first.id);
    assert_eq!(transition.value.status, GenerationJobStatus::Running);
    assert_eq!(transition.value.stage, GenerationJobStage::Preparing);
    assert_eq!(
        transition.value.started_at.as_deref(),
        Some(WORKER_NOW_TIMESTAMP)
    );
    assert_eq!(
        transition.value.last_heartbeat_at.as_deref(),
        Some(WORKER_NOW_TIMESTAMP)
    );
    let event = transition
        .event
        .expect("claim must return a committed event");
    assert_eq!(event.job_id, first.id);
    assert_eq!(event.status, GenerationJobStatus::Running);
    assert_eq!(event.stage, GenerationJobStage::Preparing);
    assert_eq!(event.started_at.as_deref(), Some(WORKER_NOW_TIMESTAMP));
    drop(conn);
    assert_eq!(fixture.generation_status(&first.generation_id), "running");
    assert_eq!(fixture.get(&second.id).status, GenerationJobStatus::Queued);
}

#[test]
fn fenced_claim_rejects_a_worker_time_before_the_queued_timestamp_without_mutation() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("claim-time-regression", "claim-time-regression");
    let before_queue_ms = chrono::DateTime::parse_from_rfc3339("2026-07-09T23:59:59Z")
        .expect("parse before-queue timestamp")
        .timestamp_millis();
    let conn = fixture.database.conn.lock().expect("lock database");
    let authority = acquire_test_worker(
        &conn,
        "claim-time-worker",
        before_queue_ms,
        Duration::from_secs(60),
    );

    let error = claim_next_job_fenced_with_event(&conn, &authority, before_queue_ms)
        .expect_err("claim time before queue must fail");
    assert!(matches!(
        error,
        WorkerTransitionError::Repository(AppError::GenerationJobInvalidTransition)
    ));
    let persisted: (String, String, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT j.status, g.status, j.started_at, j.last_heartbeat_at
               FROM generation_jobs j
               JOIN generations g ON g.id = j.generation_id
              WHERE j.id = ?1",
            params![queued.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .expect("read claim time-regression state");
    assert_eq!(persisted, ("queued".into(), "queued".into(), None, None));
}

#[test]
fn fenced_transition_time_rejects_negative_and_out_of_chrono_range_values_fail_closed() {
    let fixture = JobFixture::new();
    fixture.enqueue("invalid-worker-time", "invalid-worker-time");
    let conn = fixture.database.conn.lock().expect("lock database");
    let authority = acquire_test_worker(
        &conn,
        "invalid-time-worker",
        WORKER_NOW_MS,
        Duration::from_secs(60),
    );
    let baseline = worker_database_snapshot(&conn);

    assert!(matches!(
        claim_next_job_fenced_with_event(&conn, &authority, -1),
        Err(WorkerTransitionError::Lease(
            WorkerLeaseError::InvalidTiming
        ))
    ));
    assert_eq!(worker_database_snapshot(&conn), baseline);
    assert!(matches!(
        claim_next_job_fenced_with_event(&conn, &authority, i64::MAX),
        Err(WorkerTransitionError::Lease(WorkerLeaseError::TimeOverflow))
    ));
    assert_eq!(worker_database_snapshot(&conn), baseline);
}

#[test]
fn begin_provider_request_atomically_persists_expected_response_path_and_stage_event() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("provider-stage", "provider-stage");
    let conn = fixture.database.conn.lock().expect("lock database");
    let authority = acquire_test_worker(
        &conn,
        "provider-stage-worker",
        WORKER_NOW_MS,
        Duration::from_secs(60),
    );
    claim_next_job_fenced_with_event(&conn, &authority, WORKER_NOW_MS)
        .expect("claim provider-stage job")
        .expect("provider-stage job exists");
    let expected_path = std::env::temp_dir()
        .join("astro-studio-responses")
        .join(format!("{}.response.json", queued.generation_id));

    let transition = transition_running_job_stage_with_event(
        &conn,
        &queued.id,
        GenerationJobStage::Preparing,
        WorkerStageTransition::BeginProviderRequest {
            expected_response_file: expected_path.clone(),
        },
        &authority,
        WORKER_NOW_MS + 1_000,
    )
    .expect("begin fenced provider request");

    assert_eq!(transition.value.stage, GenerationJobStage::ProviderRequest);
    assert_eq!(
        transition.value.last_heartbeat_at.as_deref(),
        Some("2026-07-12T16:00:01Z")
    );
    let event = transition
        .event
        .expect("provider stage change must return an event");
    assert_eq!(event.stage, GenerationJobStage::ProviderRequest);
    let recovery: (
        String,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT request_state, expected_response_file, response_file,
                    response_size, response_sha256
               FROM generation_recoveries WHERE generation_id = ?1",
            params![queued.generation_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("read provider recovery metadata");
    assert_eq!(recovery.0, "requesting");
    assert_eq!(recovery.1.as_deref(), expected_path.to_str());
    assert_eq!((recovery.2, recovery.3, recovery.4), (None, None, None));
}

#[test]
fn fenced_claim_and_provider_begin_require_empty_requesting_recovery_metadata() {
    let claim_fixture = JobFixture::new();
    let queued = claim_fixture.enqueue("claim-recovery-empty", "claim-recovery-empty");
    let claim_conn = claim_fixture
        .database
        .conn
        .lock()
        .expect("lock claim recovery database");
    let injected_path = std::env::temp_dir()
        .join("astro-studio-responses")
        .join(format!("{}.response.json", queued.generation_id));
    claim_conn
        .execute(
            "UPDATE generation_recoveries SET expected_response_file = ?1
              WHERE generation_id = ?2",
            params![injected_path.to_str(), queued.generation_id],
        )
        .expect("inject queued expected path");
    let claim_authority = acquire_test_worker(
        &claim_conn,
        "claim-recovery-worker",
        WORKER_NOW_MS,
        Duration::from_secs(60),
    );
    let claim_baseline = worker_database_snapshot(&claim_conn);
    let claim_error =
        claim_next_job_fenced_with_event(&claim_conn, &claim_authority, WORKER_NOW_MS)
            .expect_err("queued recovery metadata must be empty before fenced claim");
    assert!(matches!(
        claim_error,
        WorkerTransitionError::Repository(AppError::GenerationJobCorruptPersistedData)
    ));
    assert_eq!(worker_database_snapshot(&claim_conn), claim_baseline);

    let provider_fixture = JobFixture::new();
    let provider_job = provider_fixture.enqueue("provider-recovery-empty", "provider-empty");
    let provider_conn = provider_fixture
        .database
        .conn
        .lock()
        .expect("lock provider recovery database");
    let provider_authority = acquire_test_worker(
        &provider_conn,
        "provider-recovery-empty-worker",
        WORKER_NOW_MS,
        Duration::from_secs(60),
    );
    claim_next_job_fenced_with_event(&provider_conn, &provider_authority, WORKER_NOW_MS)
        .expect("claim provider-empty fixture")
        .expect("provider-empty fixture exists");
    provider_conn
        .execute(
            "UPDATE generation_recoveries SET response_size = 1 WHERE generation_id = ?1",
            params![provider_job.generation_id],
        )
        .expect("inject requesting response metadata");
    let provider_baseline = worker_database_snapshot(&provider_conn);
    let expected_path = std::env::temp_dir()
        .join("astro-studio-responses")
        .join(format!("{}.response.json", provider_job.generation_id));
    let provider_error = transition_running_job_stage_with_event(
        &provider_conn,
        &provider_job.id,
        GenerationJobStage::Preparing,
        WorkerStageTransition::BeginProviderRequest {
            expected_response_file: expected_path,
        },
        &provider_authority,
        WORKER_NOW_MS + 1_000,
    )
    .expect_err("requesting recovery metadata must be empty before provider begin");
    assert!(matches!(
        provider_error,
        WorkerTransitionError::Repository(AppError::GenerationJobCorruptPersistedData)
    ));
    assert_eq!(worker_database_snapshot(&provider_conn), provider_baseline);
}

#[test]
fn fenced_stage_transition_rejects_time_before_the_last_job_heartbeat_and_rolls_back_recovery() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("stage-time-regression", "stage-time-regression");
    let conn = fixture.database.conn.lock().expect("lock database");
    let authority = acquire_test_worker(
        &conn,
        "stage-time-worker",
        WORKER_NOW_MS,
        Duration::from_secs(60),
    );
    claim_next_job_fenced_with_event(&conn, &authority, WORKER_NOW_MS + 5_000)
        .expect("claim stage-time job")
        .expect("stage-time job exists");
    let expected_path = std::env::temp_dir()
        .join("astro-studio-responses")
        .join(format!("{}.response.json", queued.generation_id));

    let error = transition_running_job_stage_with_event(
        &conn,
        &queued.id,
        GenerationJobStage::Preparing,
        WorkerStageTransition::BeginProviderRequest {
            expected_response_file: expected_path,
        },
        &authority,
        WORKER_NOW_MS + 4_000,
    )
    .expect_err("stage time before heartbeat must fail");
    assert!(matches!(
        error,
        WorkerTransitionError::Repository(AppError::GenerationJobInvalidTransition)
    ));
    let persisted: (String, Option<String>, String) = conn
        .query_row(
            "SELECT j.stage, r.expected_response_file, j.last_heartbeat_at
               FROM generation_jobs j
               JOIN generation_recoveries r ON r.generation_id = j.generation_id
              WHERE j.id = ?1",
            params![queued.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("read stage-time rollback state");
    assert_eq!(
        persisted,
        (
            "preparing".to_string(),
            None,
            "2026-07-12T16:00:05Z".to_string()
        )
    );
}

#[test]
fn begin_provider_request_rejects_noncanonical_paths_without_mutation() {
    for index in 0..6 {
        let fixture = JobFixture::new();
        let queued = fixture.enqueue(
            &format!("invalid-provider-path-{index}"),
            "invalid-provider-path",
        );
        let conn = fixture.database.conn.lock().expect("lock database");
        let authority = acquire_test_worker(
            &conn,
            &format!("invalid-path-worker-{index}"),
            WORKER_NOW_MS,
            Duration::from_secs(60),
        );
        claim_next_job_fenced_with_event(&conn, &authority, WORKER_NOW_MS)
            .expect("claim invalid-path fixture")
            .expect("invalid-path fixture exists");
        let expected_filename = format!("{}.response.json", queued.generation_id);
        let invalid_path = match index {
            0 => PathBuf::from(&expected_filename),
            1 => PathBuf::from(format!("/tmp/./{expected_filename}")),
            2 => PathBuf::from(format!("/tmp/../{expected_filename}")),
            3 => PathBuf::from(format!("/tmp/control\n/{expected_filename}")),
            4 => PathBuf::from("/tmp/wrong-generation.response.json"),
            5 => PathBuf::from(format!("/{}/{expected_filename}", "a".repeat(32_768))),
            _ => unreachable!(),
        };
        let before: (
            String,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT j.stage, r.expected_response_file, r.response_file,
                        r.response_size, r.response_sha256
                   FROM generation_jobs j
                   JOIN generation_recoveries r ON r.generation_id = j.generation_id
                  WHERE j.id = ?1",
                params![queued.id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("read invalid-path state before transition");

        let error = transition_running_job_stage_with_event(
            &conn,
            &queued.id,
            GenerationJobStage::Preparing,
            WorkerStageTransition::BeginProviderRequest {
                expected_response_file: invalid_path,
            },
            &authority,
            WORKER_NOW_MS + 1_000,
        )
        .expect_err("noncanonical response path must fail closed");
        assert!(matches!(
            error,
            WorkerTransitionError::Repository(AppError::GenerationJobInvalidSnapshot)
        ));
        let after: (
            String,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<String>,
        ) = conn
            .query_row(
                "SELECT j.stage, r.expected_response_file, r.response_file,
                        r.response_size, r.response_sha256
                   FROM generation_jobs j
                   JOIN generation_recoveries r ON r.generation_id = j.generation_id
                  WHERE j.id = ?1",
                params![queued.id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .expect("read invalid-path state after transition");
        assert_eq!(after, before, "invalid path case {index} mutated state");
    }
}

#[test]
fn fenced_stage_transitions_allow_only_retry_backoff_and_known_response_local_processing_edges() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("stage-edges", "stage-edges");
    let conn = fixture.database.conn.lock().expect("lock database");
    let authority = acquire_test_worker(
        &conn,
        "stage-edge-worker",
        WORKER_NOW_MS,
        Duration::from_secs(60),
    );
    claim_next_job_fenced_with_event(&conn, &authority, WORKER_NOW_MS)
        .expect("claim stage-edge job")
        .expect("stage-edge job exists");
    let expected_path = std::env::temp_dir()
        .join("astro-studio-responses")
        .join(format!("{}.response.json", queued.generation_id));
    transition_running_job_stage_with_event(
        &conn,
        &queued.id,
        GenerationJobStage::Preparing,
        WorkerStageTransition::BeginProviderRequest {
            expected_response_file: expected_path,
        },
        &authority,
        WORKER_NOW_MS + 1_000,
    )
    .expect("enter provider request");

    let wrong_edge = transition_running_job_stage_with_event(
        &conn,
        &queued.id,
        GenerationJobStage::ProviderRequest,
        WorkerStageTransition::EnterLocalProcessing,
        &authority,
        WORKER_NOW_MS + 2_000,
    )
    .expect_err("provider request cannot skip to local processing");
    assert!(matches!(
        wrong_edge,
        WorkerTransitionError::Repository(AppError::GenerationJobInvalidTransition)
    ));
    let backoff = transition_running_job_stage_with_event(
        &conn,
        &queued.id,
        GenerationJobStage::ProviderRequest,
        WorkerStageTransition::EnterRetryBackoff,
        &authority,
        WORKER_NOW_MS + 2_000,
    )
    .expect("provider request may enter retry backoff");
    assert_eq!(backoff.value.stage, GenerationJobStage::RetryBackoff);
    assert_eq!(
        backoff.event.expect("backoff stage event").stage,
        GenerationJobStage::RetryBackoff
    );

    let recovery_fixture = JobFixture::new();
    let response_ready = recovery_fixture.enqueue("local-stage", "local-stage");
    let recovery_conn = recovery_fixture
        .database
        .conn
        .lock()
        .expect("lock recovery database");
    let recovery_authority = acquire_test_worker(
        &recovery_conn,
        "local-stage-worker",
        WORKER_NOW_MS,
        Duration::from_secs(60),
    );
    claim_next_job_fenced_with_event(&recovery_conn, &recovery_authority, WORKER_NOW_MS)
        .expect("claim local-stage job")
        .expect("local-stage job exists");
    let response_path = std::env::temp_dir()
        .join("astro-studio-responses")
        .join(format!("{}.response.json", response_ready.generation_id));
    recovery_conn
        .execute(
            "UPDATE generation_recoveries
                SET request_state = 'response_ready', expected_response_file = ?1,
                    response_file = ?1, response_size = 12, response_sha256 = ?2
              WHERE generation_id = ?3",
            params![
                response_path.to_str(),
                "a".repeat(64),
                response_ready.generation_id
            ],
        )
        .expect("prepare response-ready recovery");
    recovery_conn
        .execute(
            "UPDATE generation_jobs SET stage = 'response_ready' WHERE id = ?1",
            params![response_ready.id],
        )
        .expect("prepare response-ready job");
    request_cancel(&recovery_conn, &response_ready.id).expect("persist late cancellation");

    let local = transition_running_job_stage_with_event(
        &recovery_conn,
        &response_ready.id,
        GenerationJobStage::ResponseReady,
        WorkerStageTransition::EnterLocalProcessing,
        &recovery_authority,
        WORKER_NOW_MS + 2_000,
    )
    .expect("known response must win over late cancellation");
    assert_eq!(local.value.stage, GenerationJobStage::LocalProcessing);
    assert!(local.value.cancel_requested_at.is_some());
}

#[test]
fn retry_backoff_stage_rejects_a_durable_cancellation_request_without_mutation() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("cancelled-backoff", "cancelled-backoff");
    let conn = fixture.database.conn.lock().expect("lock database");
    let authority = acquire_test_worker(
        &conn,
        "cancelled-backoff-worker",
        WORKER_NOW_MS,
        Duration::from_secs(60),
    );
    claim_next_job_fenced_with_event(&conn, &authority, WORKER_NOW_MS)
        .expect("claim cancelled-backoff job")
        .expect("cancelled-backoff job exists");
    let expected_path = std::env::temp_dir()
        .join("astro-studio-responses")
        .join(format!("{}.response.json", queued.generation_id));
    transition_running_job_stage_with_event(
        &conn,
        &queued.id,
        GenerationJobStage::Preparing,
        WorkerStageTransition::BeginProviderRequest {
            expected_response_file: expected_path,
        },
        &authority,
        WORKER_NOW_MS + 1_000,
    )
    .expect("begin provider request before cancellation");
    request_cancel(&conn, &queued.id).expect("persist provider cancellation");
    let baseline = worker_database_snapshot(&conn);

    let error = transition_running_job_stage_with_event(
        &conn,
        &queued.id,
        GenerationJobStage::ProviderRequest,
        WorkerStageTransition::EnterRetryBackoff,
        &authority,
        WORKER_NOW_MS + 2_000,
    )
    .expect_err("cancelled provider request cannot enter retry backoff");
    assert!(matches!(
        error,
        WorkerTransitionError::Repository(AppError::GenerationJobInvalidTransition)
    ));
    assert_eq!(worker_database_snapshot(&conn), baseline);
}

#[test]
fn local_processing_rejects_incomplete_or_unbound_response_ready_metadata_even_with_late_cancel() {
    for (index, mutation) in [
        "UPDATE generation_recoveries SET expected_response_file = NULL WHERE generation_id = ?1",
        "UPDATE generation_recoveries SET expected_response_file = '/tmp/wrong.response.json' WHERE generation_id = ?1",
        "UPDATE generation_recoveries SET response_file = '/tmp/wrong.response.json' WHERE generation_id = ?1",
        "UPDATE generation_recoveries SET response_size = 67108865 WHERE generation_id = ?1",
        "UPDATE generation_recoveries SET response_sha256 = 'g' || substr(printf('%064d', 0), 2) WHERE generation_id = ?1",
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = JobFixture::new();
        let queued = fixture.enqueue(
            &format!("invalid-response-ready-{index}"),
            "invalid-response-ready",
        );
        let conn = fixture.database.conn.lock().expect("lock database");
        let authority = acquire_test_worker(
            &conn,
            &format!("invalid-response-worker-{index}"),
            WORKER_NOW_MS,
            Duration::from_secs(60),
        );
        claim_next_job_fenced_with_event(&conn, &authority, WORKER_NOW_MS)
            .expect("claim invalid response fixture")
            .expect("invalid response fixture exists");
        let response_path = std::env::temp_dir()
            .join("astro-studio-responses")
            .join(format!("{}.response.json", queued.generation_id));
        conn.execute(
            "UPDATE generation_recoveries
                SET request_state = 'response_ready', expected_response_file = ?1,
                    response_file = ?1, response_size = 12, response_sha256 = ?2
              WHERE generation_id = ?3",
            params![response_path.to_str(), "a".repeat(64), queued.generation_id],
        )
        .expect("prepare response metadata");
        conn.execute(
            "UPDATE generation_jobs SET stage = 'response_ready' WHERE id = ?1",
            params![queued.id],
        )
        .expect("prepare response stage");
        conn.execute(mutation, params![queued.generation_id])
            .expect("invalidate response metadata");
        request_cancel(&conn, &queued.id).expect("persist late cancellation");

        let error = transition_running_job_stage_with_event(
            &conn,
            &queued.id,
            GenerationJobStage::ResponseReady,
            WorkerStageTransition::EnterLocalProcessing,
            &authority,
            WORKER_NOW_MS + 1_000,
        )
        .expect_err("unverified response metadata must block local processing");
        assert!(matches!(
            error,
            WorkerTransitionError::Repository(
                AppError::GenerationJobCorruptPersistedData
                    | AppError::GenerationJobInvalidTransition
            )
        ));
        let stage: String = conn
            .query_row(
                "SELECT stage FROM generation_jobs WHERE id = ?1",
                params![queued.id],
                |row| row.get(0),
            )
            .expect("read unchanged response stage");
        assert_eq!(stage, "response_ready", "invalid case {index} advanced");
    }
}

#[test]
fn provider_and_retry_transitions_recheck_their_stage_aware_requesting_recovery() {
    for reserve_retry in [false, true] {
        let fixture = JobFixture::new();
        let queued = fixture.enqueue(
            if reserve_retry {
                "retry-recovery-recheck"
            } else {
                "provider-recovery-recheck"
            },
            "recovery-recheck",
        );
        let conn = fixture.database.conn.lock().expect("lock database");
        let authority = acquire_test_worker(
            &conn,
            if reserve_retry {
                "retry-recovery-worker"
            } else {
                "provider-recovery-worker"
            },
            WORKER_NOW_MS,
            Duration::from_secs(60),
        );
        claim_next_job_fenced_with_event(&conn, &authority, WORKER_NOW_MS)
            .expect("claim recovery recheck fixture")
            .expect("recovery recheck fixture exists");
        let expected_path = std::env::temp_dir()
            .join("astro-studio-responses")
            .join(format!("{}.response.json", queued.generation_id));
        transition_running_job_stage_with_event(
            &conn,
            &queued.id,
            GenerationJobStage::Preparing,
            WorkerStageTransition::BeginProviderRequest {
                expected_response_file: expected_path,
            },
            &authority,
            WORKER_NOW_MS + 1_000,
        )
        .expect("begin provider request before recovery corruption");
        if reserve_retry {
            transition_running_job_stage_with_event(
                &conn,
                &queued.id,
                GenerationJobStage::ProviderRequest,
                WorkerStageTransition::EnterRetryBackoff,
                &authority,
                WORKER_NOW_MS + 2_000,
            )
            .expect("enter retry backoff before recovery corruption");
        }
        conn.execute(
            "UPDATE generation_recoveries SET expected_response_file = NULL
              WHERE generation_id = ?1",
            params![queued.generation_id],
        )
        .expect("remove expected response binding");

        let error = if reserve_retry {
            reserve_automatic_retry_with_event(
                &conn,
                &queued.id,
                0,
                &authority,
                WORKER_NOW_MS + 3_000,
            )
            .map(|_| ())
            .expect_err("retry reservation requires bound requesting recovery")
        } else {
            transition_running_job_stage_with_event(
                &conn,
                &queued.id,
                GenerationJobStage::ProviderRequest,
                WorkerStageTransition::EnterRetryBackoff,
                &authority,
                WORKER_NOW_MS + 2_000,
            )
            .map(|_| ())
            .expect_err("retry backoff requires bound requesting recovery")
        };
        assert!(matches!(
            error,
            WorkerTransitionError::Repository(AppError::GenerationJobCorruptPersistedData)
        ));
        let persisted = get_job(&conn, &queued.id).expect("reload recovery recheck job");
        assert_eq!(persisted.auto_attempt, 0);
        assert_eq!(
            persisted.stage,
            if reserve_retry {
                GenerationJobStage::RetryBackoff
            } else {
                GenerationJobStage::ProviderRequest
            }
        );
    }
}

#[test]
fn fenced_heartbeat_updates_only_the_exact_running_stage_without_time_regression_or_event() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("heartbeat-job", "heartbeat-job");
    let conn = fixture.database.conn.lock().expect("lock database");
    let authority = acquire_test_worker(
        &conn,
        "heartbeat-worker",
        WORKER_NOW_MS,
        Duration::from_secs(60),
    );
    claim_next_job_fenced_with_event(&conn, &authority, WORKER_NOW_MS)
        .expect("claim heartbeat job")
        .expect("heartbeat job exists");

    let heartbeat: crate::models::GenerationJob = heartbeat_running_job(
        &conn,
        &queued.id,
        GenerationJobStage::Preparing,
        &authority,
        WORKER_NOW_MS + 2_000,
    )
    .expect("write fenced heartbeat");
    assert_eq!(heartbeat.stage, GenerationJobStage::Preparing);
    assert_eq!(
        heartbeat.last_heartbeat_at.as_deref(),
        Some("2026-07-12T16:00:02Z")
    );

    for (stage, now_ms) in [
        (GenerationJobStage::ProviderRequest, WORKER_NOW_MS + 3_000),
        (GenerationJobStage::Preparing, WORKER_NOW_MS + 1_000),
    ] {
        let error = heartbeat_running_job(&conn, &queued.id, stage, &authority, now_ms)
            .expect_err("wrong stage or regressing heartbeat must fail");
        assert!(matches!(
            error,
            WorkerTransitionError::Repository(AppError::GenerationJobInvalidTransition)
        ));
        let persisted = get_job(&conn, &queued.id).expect("reload heartbeat job");
        assert_eq!(
            persisted.last_heartbeat_at.as_deref(),
            Some("2026-07-12T16:00:02Z")
        );
    }
}

#[test]
fn fenced_retry_reservation_increments_exact_ordinal_after_backoff_and_returns_event_snapshot() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("retry-reservation", "retry-reservation");
    let conn = fixture.database.conn.lock().expect("lock database");
    let authority = acquire_test_worker(
        &conn,
        "retry-reservation-worker",
        WORKER_NOW_MS,
        Duration::from_secs(60),
    );
    claim_next_job_fenced_with_event(&conn, &authority, WORKER_NOW_MS)
        .expect("claim retry reservation job")
        .expect("retry reservation job exists");
    let expected_path = std::env::temp_dir()
        .join("astro-studio-responses")
        .join(format!("{}.response.json", queued.generation_id));
    transition_running_job_stage_with_event(
        &conn,
        &queued.id,
        GenerationJobStage::Preparing,
        WorkerStageTransition::BeginProviderRequest {
            expected_response_file: expected_path,
        },
        &authority,
        WORKER_NOW_MS + 1_000,
    )
    .expect("begin retry provider request");
    transition_running_job_stage_with_event(
        &conn,
        &queued.id,
        GenerationJobStage::ProviderRequest,
        WorkerStageTransition::EnterRetryBackoff,
        &authority,
        WORKER_NOW_MS + 2_000,
    )
    .expect("enter retry backoff");

    let reserved =
        reserve_automatic_retry_with_event(&conn, &queued.id, 0, &authority, WORKER_NOW_MS + 3_000)
            .expect("reserve first automatic retry");

    assert_eq!(reserved.value.auto_attempt, 1);
    assert_eq!(reserved.value.stage, GenerationJobStage::ProviderRequest);
    assert_eq!(
        reserved.value.last_heartbeat_at.as_deref(),
        Some("2026-07-12T16:00:03Z")
    );
    let event = reserved.event.expect("retry reservation event");
    assert_eq!(event.auto_attempt, 1);
    assert_eq!(event.stage, GenerationJobStage::ProviderRequest);
    transition_running_job_stage_with_event(
        &conn,
        &queued.id,
        GenerationJobStage::ProviderRequest,
        WorkerStageTransition::EnterRetryBackoff,
        &authority,
        WORKER_NOW_MS + 4_000,
    )
    .expect("advance persisted state after event snapshot");
    assert_eq!(event.stage, GenerationJobStage::ProviderRequest);
    assert_eq!(event.auto_attempt, 1);
}

#[test]
fn retry_reservation_failures_never_change_attempt_stage_or_heartbeat() {
    let fixture = JobFixture::new();
    let retry = fixture.enqueue("retry-guards", "retry-guards");
    let conn = fixture.database.conn.lock().expect("lock database");
    let authority = acquire_test_worker(
        &conn,
        "retry-guard-worker",
        WORKER_NOW_MS,
        Duration::from_secs(60),
    );
    prepare_fenced_retry_backoff(&conn, &retry, &authority);
    let baseline = worker_database_snapshot(&conn);

    for (expected_attempt, now_ms) in [(1, WORKER_NOW_MS + 3_000), (0, WORKER_NOW_MS + 1_000)] {
        let error = reserve_automatic_retry_with_event(
            &conn,
            &retry.id,
            expected_attempt,
            &authority,
            now_ms,
        )
        .expect_err("wrong ordinal or regressing time must reject retry reservation");
        assert!(matches!(
            error,
            WorkerTransitionError::Repository(AppError::GenerationJobInvalidTransition)
        ));
        assert_eq!(worker_database_snapshot(&conn), baseline);
    }
    request_cancel(&conn, &retry.id).expect("persist retry cancellation");
    let cancelled_baseline = worker_database_snapshot(&conn);
    let cancelled_error =
        reserve_automatic_retry_with_event(&conn, &retry.id, 0, &authority, WORKER_NOW_MS + 3_000)
            .expect_err("cancelled backoff must not reserve retry");
    assert!(matches!(
        cancelled_error,
        WorkerTransitionError::Repository(AppError::GenerationJobInvalidTransition)
    ));
    assert_eq!(worker_database_snapshot(&conn), cancelled_baseline);
    drop(conn);

    let max_fixture = JobFixture::new();
    let mut max_request = max_fixture.prepared("retry-max", "retry-max");
    max_request.max_auto_attempts = 0;
    let max_result = max_fixture
        .enqueue_prepared(&max_request)
        .expect("enqueue max retry fixture");
    let max_job = max_fixture.get(&max_result.job_id);
    let max_conn = max_fixture.database.conn.lock().expect("lock max database");
    let max_authority = acquire_test_worker(
        &max_conn,
        "retry-max-worker",
        WORKER_NOW_MS,
        Duration::from_secs(60),
    );
    prepare_fenced_retry_backoff(&max_conn, &max_job, &max_authority);
    let max_baseline = worker_database_snapshot(&max_conn);
    let max_error = reserve_automatic_retry_with_event(
        &max_conn,
        &max_job.id,
        0,
        &max_authority,
        WORKER_NOW_MS + 3_000,
    )
    .expect_err("reached retry maximum must fail");
    assert!(matches!(
        max_error,
        WorkerTransitionError::Repository(AppError::GenerationJobInvalidTransition)
    ));
    assert_eq!(worker_database_snapshot(&max_conn), max_baseline);
    drop(max_conn);

    let overflow_fixture = JobFixture::new();
    let overflow_job = overflow_fixture.enqueue("retry-overflow", "retry-overflow");
    let overflow_conn = overflow_fixture
        .database
        .conn
        .lock()
        .expect("lock overflow database");
    let overflow_authority = acquire_test_worker(
        &overflow_conn,
        "retry-overflow-worker",
        WORKER_NOW_MS,
        Duration::from_secs(60),
    );
    prepare_fenced_retry_backoff(&overflow_conn, &overflow_job, &overflow_authority);
    overflow_conn
        .execute(
            "UPDATE generation_jobs
                SET auto_attempt = 2147483647, max_auto_attempts = 2147483647
              WHERE id = ?1",
            params![overflow_job.id],
        )
        .expect("prepare checked-add overflow row");
    let overflow_baseline = worker_database_snapshot(&overflow_conn);
    let overflow_error = reserve_automatic_retry_with_event(
        &overflow_conn,
        &overflow_job.id,
        i32::MAX,
        &overflow_authority,
        WORKER_NOW_MS + 3_000,
    )
    .expect_err("retry ordinal overflow must fail closed");
    assert!(matches!(
        overflow_error,
        WorkerTransitionError::Repository(AppError::GenerationJobInvalidTransition)
    ));
    assert_eq!(worker_database_snapshot(&overflow_conn), overflow_baseline);
}

#[test]
fn lease_takeover_fences_claim_path_stage_retry_and_heartbeat_before_any_state_mutation() {
    let fixture = JobFixture::new();
    let path_job = fixture.enqueue("stale-path", "stale-path");
    let stage_job = fixture.enqueue("stale-stage", "stale-stage");
    let retry_job = fixture.enqueue("stale-retry", "stale-retry");
    let heartbeat_job = fixture.enqueue("stale-heartbeat", "stale-heartbeat");
    let queued_for_claim = fixture.enqueue("stale-claim", "stale-claim");
    let a = fixture
        .database
        .conn
        .lock()
        .expect("lock worker A database");
    let b = fixture.open_connection();
    let authority_a = acquire_test_worker(
        &a,
        "transition-worker-a",
        WORKER_NOW_MS,
        Duration::from_secs(10),
    );

    let claimed_path = claim_next_job_fenced_with_event(&a, &authority_a, WORKER_NOW_MS + 1_000)
        .expect("worker A claim path job")
        .expect("path job exists");
    assert_eq!(claimed_path.value.id, path_job.id);
    let claimed_stage = claim_next_job_fenced_with_event(&a, &authority_a, WORKER_NOW_MS + 2_000)
        .expect("worker A claim stage job")
        .expect("stage job exists");
    assert_eq!(claimed_stage.value.id, stage_job.id);
    let stage_path = std::env::temp_dir()
        .join("astro-studio-responses")
        .join(format!("{}.response.json", stage_job.generation_id));
    transition_running_job_stage_with_event(
        &a,
        &stage_job.id,
        GenerationJobStage::Preparing,
        WorkerStageTransition::BeginProviderRequest {
            expected_response_file: stage_path,
        },
        &authority_a,
        WORKER_NOW_MS + 3_000,
    )
    .expect("worker A prepare stage job");
    let claimed_retry = claim_next_job_fenced_with_event(&a, &authority_a, WORKER_NOW_MS + 4_000)
        .expect("worker A claim retry job")
        .expect("retry job exists");
    assert_eq!(claimed_retry.value.id, retry_job.id);
    let retry_path = std::env::temp_dir()
        .join("astro-studio-responses")
        .join(format!("{}.response.json", retry_job.generation_id));
    transition_running_job_stage_with_event(
        &a,
        &retry_job.id,
        GenerationJobStage::Preparing,
        WorkerStageTransition::BeginProviderRequest {
            expected_response_file: retry_path,
        },
        &authority_a,
        WORKER_NOW_MS + 5_000,
    )
    .expect("worker A prepare retry job");
    transition_running_job_stage_with_event(
        &a,
        &retry_job.id,
        GenerationJobStage::ProviderRequest,
        WorkerStageTransition::EnterRetryBackoff,
        &authority_a,
        WORKER_NOW_MS + 6_000,
    )
    .expect("worker A back off retry job");
    let claimed_heartbeat =
        claim_next_job_fenced_with_event(&a, &authority_a, WORKER_NOW_MS + 7_000)
            .expect("worker A claim heartbeat job")
            .expect("heartbeat job exists");
    assert_eq!(claimed_heartbeat.value.id, heartbeat_job.id);

    let expires_a = WORKER_NOW_MS + 10_000;
    let before_expiry_rejection = worker_database_snapshot(&a);
    assert_worker_transition_lease_lost(heartbeat_running_job(
        &a,
        &heartbeat_job.id,
        GenerationJobStage::Preparing,
        &authority_a,
        expires_a,
    ));
    assert_eq!(worker_database_snapshot(&a), before_expiry_rejection);

    let authority_b = acquire_test_worker(
        &b,
        "transition-worker-b",
        expires_a,
        Duration::from_secs(60),
    );
    let stale_baseline = worker_database_snapshot(&a);
    let stale_path = std::env::temp_dir()
        .join("astro-studio-responses")
        .join(format!("{}.response.json", path_job.generation_id));

    assert_worker_transition_lease_lost(claim_next_job_fenced_with_event(
        &a,
        &authority_a,
        expires_a,
    ));
    assert_eq!(worker_database_snapshot(&a), stale_baseline);
    assert_worker_transition_lease_lost(transition_running_job_stage_with_event(
        &a,
        &path_job.id,
        GenerationJobStage::Preparing,
        WorkerStageTransition::BeginProviderRequest {
            expected_response_file: stale_path.clone(),
        },
        &authority_a,
        expires_a,
    ));
    assert_eq!(worker_database_snapshot(&a), stale_baseline);
    assert_worker_transition_lease_lost(transition_running_job_stage_with_event(
        &a,
        &stage_job.id,
        GenerationJobStage::ProviderRequest,
        WorkerStageTransition::EnterRetryBackoff,
        &authority_a,
        expires_a,
    ));
    assert_eq!(worker_database_snapshot(&a), stale_baseline);
    assert_worker_transition_lease_lost(reserve_automatic_retry_with_event(
        &a,
        &retry_job.id,
        0,
        &authority_a,
        expires_a,
    ));
    assert_eq!(worker_database_snapshot(&a), stale_baseline);
    assert_worker_transition_lease_lost(heartbeat_running_job(
        &a,
        &heartbeat_job.id,
        GenerationJobStage::Preparing,
        &authority_a,
        expires_a,
    ));
    assert_eq!(worker_database_snapshot(&a), stale_baseline);

    let claimed_by_b = claim_next_job_fenced_with_event(&b, &authority_b, expires_a)
        .expect("worker B claim succeeds")
        .expect("queued claim job remains");
    assert_eq!(claimed_by_b.value.id, queued_for_claim.id);
    transition_running_job_stage_with_event(
        &b,
        &path_job.id,
        GenerationJobStage::Preparing,
        WorkerStageTransition::BeginProviderRequest {
            expected_response_file: stale_path,
        },
        &authority_b,
        expires_a + 1_000,
    )
    .expect("worker B expected-path transition succeeds");
    transition_running_job_stage_with_event(
        &b,
        &stage_job.id,
        GenerationJobStage::ProviderRequest,
        WorkerStageTransition::EnterRetryBackoff,
        &authority_b,
        expires_a + 2_000,
    )
    .expect("worker B stage transition succeeds");
    reserve_automatic_retry_with_event(&b, &retry_job.id, 0, &authority_b, expires_a + 3_000)
        .expect("worker B retry reservation succeeds");
    heartbeat_running_job(
        &b,
        &heartbeat_job.id,
        GenerationJobStage::Preparing,
        &authority_b,
        expires_a + 4_000,
    )
    .expect("worker B heartbeat succeeds");
}

#[test]
fn fenced_stage_commit_failure_returns_no_event_and_rolls_back_job_and_recovery() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("fenced-event-rollback", "fenced-event-rollback");
    let conn = fixture.database.conn.lock().expect("lock database");
    let authority = acquire_test_worker(
        &conn,
        "fenced-event-rollback-worker",
        WORKER_NOW_MS,
        Duration::from_secs(60),
    );
    claim_next_job_fenced_with_event(&conn, &authority, WORKER_NOW_MS)
        .expect("claim fenced rollback fixture")
        .expect("fenced rollback fixture exists");
    conn.execute_batch(
        "CREATE TRIGGER fail_fenced_stage_at_commit
         AFTER UPDATE OF stage ON generation_jobs
         WHEN NEW.stage = 'provider_request'
         BEGIN
             INSERT INTO images (id, generation_id, file_path)
             VALUES ('fenced-event-rollback-image', 'missing-generation', '/tmp/rollback.png');
         END;
         PRAGMA defer_foreign_keys=ON;",
    )
    .expect("install fenced deferred commit failure");
    let baseline = worker_database_snapshot(&conn);
    let expected_path = std::env::temp_dir()
        .join("astro-studio-responses")
        .join(format!("{}.response.json", queued.generation_id));

    let result = transition_running_job_stage_with_event(
        &conn,
        &queued.id,
        GenerationJobStage::Preparing,
        WorkerStageTransition::BeginProviderRequest {
            expected_response_file: expected_path,
        },
        &authority,
        WORKER_NOW_MS + 1_000,
    );

    assert!(matches!(
        result,
        Err(WorkerTransitionError::Repository(AppError::Database { .. }))
    ));
    assert_eq!(worker_database_snapshot(&conn), baseline);
    assert_eq!(count_table(&conn, "images"), 0);
}

#[test]
fn claim_and_list_use_stable_same_second_fifo_and_update_generation() {
    let fixture = JobFixture::new();
    let first = fixture.enqueue("request-1", "first");
    let second = fixture.enqueue("request-2", "second");
    assert_eq!(first.queued_at, second.queued_at);
    assert_eq!(first.stage, GenerationJobStage::Queued);
    assert_eq!(second.stage, GenerationJobStage::Queued);

    let listed = fixture.list(&GenerationJobFilter::default());
    assert_eq!(
        listed
            .items
            .iter()
            .map(|job| job.id.as_str())
            .collect::<Vec<_>>(),
        [first.id.as_str(), second.id.as_str()]
    );

    let claimed = fixture.claim().expect("claim first job");
    assert_eq!(claimed.id, first.id);
    assert_eq!(claimed.status, GenerationJobStatus::Running);
    assert_eq!(claimed.stage, GenerationJobStage::Preparing);
    assert_eq!(fixture.generation_status(&first.generation_id), "running");

    let claimed_second = fixture.claim().expect("claim second job");
    assert_eq!(claimed_second.id, second.id);
}

#[test]
fn generic_reads_and_claim_fail_closed_for_unknown_or_mismatched_stages() {
    for (index, stage) in ["migration_unknown", "future_stage", "terminal"]
        .into_iter()
        .enumerate()
    {
        let fixture = JobFixture::new();
        let queued = fixture.enqueue(&format!("request-{index}"), "stage-sentinel");
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            "UPDATE generation_jobs SET stage = ?1 WHERE id = ?2",
            params![stage, queued.id],
        )
        .expect("mutate persisted stage");

        let get_error = get_job(&conn, &queued.id).expect_err("generic get must reject stage");
        assert_eq!(
            stable_code(&get_error),
            "generation_job_corrupt_persisted_data"
        );
        let list_error = list_jobs(&conn, &GenerationJobFilter::default())
            .expect_err("generic list must reject stage");
        assert_eq!(
            stable_code(&list_error),
            "generation_job_corrupt_persisted_data"
        );
        drop(conn);

        let mut conn = fixture.database.conn.lock().expect("lock database");
        assert!(claim_next_job(&mut conn)
            .expect("non-claimable stage must remain untouched")
            .is_none());
        let tx = conn.transaction().expect("begin exact claim transaction");
        let claim_error = claim_job_in_transaction(&tx, &queued.id)
            .expect_err("exact claim must reject non-queued stage");
        assert!(matches!(
            stable_code(&claim_error),
            "generation_job_corrupt_persisted_data" | "generation_job_invalid_transition"
        ));
        tx.rollback().expect("rollback rejected exact claim");
    }
}

#[test]
fn legacy_response_recovery_is_reserved_and_invalid_for_every_generic_status_projection() {
    for (index, status) in [
        GenerationJobStatus::Queued,
        GenerationJobStatus::Running,
        GenerationJobStatus::Failed,
        GenerationJobStatus::Cancelled,
        GenerationJobStatus::Interrupted,
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = JobFixture::new();
        let job = transition_fixture_job(&fixture, status, &format!("legacy-reserved-{index}"));
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            "UPDATE generation_jobs SET stage = 'legacy_response_recovery' WHERE id = ?1",
            params![job.id],
        )
        .expect("inject reserved legacy stage");

        for error in [
            get_job(&conn, &job.id).expect_err("generic get must reject reserved stage"),
            list_jobs(&conn, &GenerationJobFilter::default())
                .expect_err("generic list must reject reserved stage"),
            get_job_event(&conn, &job.id).expect_err("generic event must reject reserved stage"),
        ] {
            assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        }
        let execution_error = load_generation_execution_snapshot(&conn, &job.id)
            .expect_err("generic execution must reject reserved stage");
        assert!(matches!(
            stable_code(&execution_error),
            "generation_job_corrupt_persisted_data" | "generation_job_invalid_transition"
        ));
    }

    let (fixture, completed) = completed_projection_fixture(1, 1, "legacy-reserved-completed");
    let conn = fixture.database.conn.lock().expect("lock database");
    conn.execute(
        "UPDATE generation_jobs SET stage = 'legacy_response_recovery' WHERE id = ?1",
        params![completed.id],
    )
    .expect("inject completed reserved legacy stage");
    for error in [
        get_job(&conn, &completed.id).expect_err("completed get must reject reserved stage"),
        list_jobs(&conn, &GenerationJobFilter::default())
            .expect_err("completed list must reject reserved stage"),
        get_job_event(&conn, &completed.id)
            .expect_err("completed event must reject reserved stage"),
    ] {
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    }
    let execution_error = load_generation_execution_snapshot(&conn, &completed.id)
        .expect_err("completed execution must reject reserved stage");
    assert!(matches!(
        stable_code(&execution_error),
        "generation_job_corrupt_persisted_data" | "generation_job_invalid_transition"
    ));
}

#[test]
fn only_preparing_stage_can_use_generic_execution_snapshot() {
    for (index, (persisted_stage, expected_stage)) in [
        ("provider_request", GenerationJobStage::ProviderRequest),
        ("retry_backoff", GenerationJobStage::RetryBackoff),
        ("response_ready", GenerationJobStage::ResponseReady),
        ("local_processing", GenerationJobStage::LocalProcessing),
        (
            "startup_reconciliation",
            GenerationJobStage::StartupReconciliation,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = JobFixture::new();
        let queued = fixture.enqueue(&format!("request-stage-{index}"), "non-provider-stage");
        fixture.claim().expect("claim stage fixture");
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            "UPDATE generation_jobs SET stage = ?1 WHERE id = ?2",
            params![persisted_stage, queued.id],
        )
        .expect("mark non-preparing stage");
        assert_eq!(
            get_job(&conn, &queued.id)
                .expect("read non-preparing job")
                .stage,
            expected_stage
        );

        let error = load_generation_execution_snapshot(&conn, &queued.id)
            .expect_err("generic provider snapshot must reject every non-preparing stage");
        assert_eq!(stable_code(&error), "generation_job_invalid_transition");
    }
}

#[test]
fn job_event_projects_committed_conversation_and_secret_free_public_fields() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("event-request", "decoy-source-id");
    let conn = fixture.database.conn.lock().expect("lock database");
    let conversation_id = conn
        .query_row(
            "SELECT conversation_id FROM generations WHERE id = ?1",
            params![queued.generation_id],
            |row| row.get::<_, String>(0),
        )
        .expect("read committed generation conversation");
    assert_ne!(
        queued.source_ref["id"].as_str(),
        Some(conversation_id.as_str()),
        "fixture must distinguish source JSON from committed conversation"
    );

    let event = get_job_event(&conn, &queued.id).expect("project committed job event");
    assert_eq!(event.job_id, queued.id);
    assert_eq!(event.generation_id, queued.generation_id);
    assert_eq!(event.conversation_id, conversation_id);
    assert_eq!(event.source_kind, queued.source_kind);
    assert_eq!(event.source_ref, queued.source_ref);
    assert_eq!(event.status, GenerationJobStatus::Queued);
    assert_eq!(event.stage, GenerationJobStage::Queued);
    assert_eq!(event.queue_position, Some(1));
    assert_eq!(event.chain_attempt, queued.chain_attempt);
    assert_eq!(event.auto_attempt, queued.auto_attempt);
    assert_eq!(event.max_auto_attempts, queued.max_auto_attempts);
    assert_eq!(event.cancel_requested_at, queued.cancel_requested_at);
    assert_eq!(event.error_code, queued.error_code);
    assert_eq!(event.error_message, queued.error_message);
    assert_eq!(event.retryable, queued.retryable);
    assert_eq!(event.queued_at, queued.queued_at);
    assert_eq!(event.started_at, queued.started_at);
    assert_eq!(event.finished_at, queued.finished_at);

    let encoded = serde_json::to_string(&event).expect("serialize committed event");
    for private in [
        queued.request["prompt"].as_str().expect("queued prompt"),
        queued.endpoint_snapshot.as_str(),
        "request_json",
        "endpoint_snapshot",
        "api_key",
        "secret-key",
    ] {
        assert!(!encoded.contains(private), "event leaked {private}");
    }
}

#[test]
fn queued_event_position_uses_queued_at_then_rowid_and_reflows_after_claim() {
    let fixture = JobFixture::new();
    let first = fixture.enqueue("event-position-1", "event-first");
    let second = fixture.enqueue("event-position-2", "event-second");
    assert_eq!(first.queued_at, second.queued_at);
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        assert_eq!(
            get_job_event(&conn, &first.id)
                .expect("project first queued event")
                .queue_position,
            Some(1)
        );
        assert_eq!(
            get_job_event(&conn, &second.id)
                .expect("project second queued event")
                .queue_position,
            Some(2)
        );
    }

    fixture.claim().expect("claim first queued event job");
    let conn = fixture.database.conn.lock().expect("lock database");
    assert_eq!(
        get_job_event(&conn, &first.id)
            .expect("project running event")
            .queue_position,
        None
    );
    assert_eq!(
        get_job_event(&conn, &second.id)
            .expect("project remaining queued event")
            .queue_position,
        Some(1)
    );
}

#[test]
fn transition_variants_return_owned_committed_events_and_suppress_idempotent_replays() {
    let fixture = JobFixture::new();
    let first = fixture.enqueue("event-transition-first", "event-transition-first");
    let second_request = fixture.prepared("event-transition-second", "event-transition-second");

    let mut conn = fixture.database.conn.lock().expect("lock database");
    let enqueued = enqueue_job_with_event(&mut conn, &second_request)
        .expect("enqueue transition with committed event");
    let enqueued_event = enqueued
        .event
        .as_ref()
        .expect("new enqueue must publish one event");
    assert_eq!(enqueued.value.job_id, second_request.job_id);
    assert_eq!(enqueued_event.status, GenerationJobStatus::Queued);
    assert_eq!(enqueued_event.stage, GenerationJobStage::Queued);
    assert_eq!(enqueued_event.queue_position, Some(2));

    let replayed = enqueue_job_with_event(&mut conn, &second_request)
        .expect("idempotent enqueue acknowledgement");
    assert_eq!(replayed.value.job_id, enqueued.value.job_id);
    assert!(
        replayed.event.is_none(),
        "idempotent enqueue must not masquerade as a new transition"
    );

    let claimed_first = claim_next_job_with_event(&mut conn)
        .expect("claim first transition")
        .expect("first queued job");
    assert_eq!(claimed_first.value.id, first.id);
    assert_eq!(
        claimed_first
            .event
            .as_ref()
            .expect("claim must publish one event")
            .stage,
        GenerationJobStage::Preparing
    );
    assert_eq!(
        get_job_event(&conn, &second_request.job_id)
            .expect("project reflowed second job")
            .queue_position,
        Some(1)
    );
    assert_eq!(
        enqueued
            .event
            .as_ref()
            .expect("owned enqueue event remains present")
            .queue_position,
        Some(2),
        "a committed event must remain an immutable historical snapshot"
    );

    let claimed_second = claim_next_job_with_event(&mut conn)
        .expect("claim second transition")
        .expect("second queued job");
    assert_eq!(claimed_second.value.id, second_request.job_id);
    let cancel_requested = request_cancel_with_event(&conn, &second_request.job_id)
        .expect("request running cancellation with committed event");
    let cancel_event = cancel_requested
        .event
        .as_ref()
        .expect("new cancellation request must publish one event");
    assert_eq!(cancel_event.status, GenerationJobStatus::Running);
    assert_eq!(cancel_event.stage, GenerationJobStage::Preparing);
    assert!(cancel_event.cancel_requested_at.is_some());

    let finished = finish_job_with_event(
        &conn,
        &GenerationJobTerminalUpdate {
            job_id: second_request.job_id.clone(),
            expected_status: GenerationJobStatus::Running,
            status: GenerationJobStatus::Cancelled,
            finished_at: "2026-07-10T00:00:03Z".to_string(),
            error_code: None,
            error_message: None,
            retryable: false,
        },
    )
    .expect("finish cancelled job with committed event");
    let finished_event = finished
        .event
        .as_ref()
        .expect("terminal transition must publish one event");
    assert_eq!(finished.value.status, GenerationJobStatus::Cancelled);
    assert_eq!(finished_event.status, GenerationJobStatus::Cancelled);
    assert_eq!(finished_event.stage, GenerationJobStage::Terminal);
    assert_eq!(
        cancel_requested
            .event
            .as_ref()
            .expect("owned cancellation event remains present")
            .status,
        GenerationJobStatus::Running,
        "a later terminal transition must not rewrite an earlier event snapshot"
    );
    drop(conn);

    let failed = fixture.fail_retryable("event-transition-parent", "event-transition-parent");
    let mut conn = fixture.database.conn.lock().expect("lock database");
    let retry = create_retry_job_with_event(&mut conn, &failed.id, "event-transition-retry")
        .expect("retry transition with committed event");
    let retry_event = retry
        .event
        .as_ref()
        .expect("new retry must publish one event");
    assert_eq!(retry.value.status, GenerationJobStatus::Queued);
    assert_eq!(retry_event.job_id, retry.value.job_id);
    assert_eq!(retry_event.status, GenerationJobStatus::Queued);
    assert_eq!(retry_event.stage, GenerationJobStage::Queued);

    let retry_replay = create_retry_job_with_event(&mut conn, &failed.id, "event-transition-retry")
        .expect("idempotent retry acknowledgement");
    assert_eq!(retry_replay.value.job_id, retry.value.job_id);
    assert!(
        retry_replay.event.is_none(),
        "idempotent retry must not masquerade as a new transition"
    );
}

#[test]
fn transition_variant_returns_no_event_when_commit_rolls_back() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("event-rollback", "event-rollback");
    fixture.claim().expect("claim rollback fixture");
    let conn = fixture.database.conn.lock().expect("lock database");
    conn.execute_batch(
        "CREATE TRIGGER fail_generation_job_finish_at_commit
         AFTER UPDATE OF status ON generation_jobs
         WHEN NEW.status = 'failed'
         BEGIN
             INSERT INTO images (id, generation_id, file_path)
             VALUES ('event-rollback-image', 'missing-generation', '/tmp/event-rollback.png');
         END;
         PRAGMA defer_foreign_keys=ON;",
    )
    .expect("install deferred commit failure");

    let result = finish_job_with_event(
        &conn,
        &GenerationJobTerminalUpdate {
            job_id: queued.id.clone(),
            expected_status: GenerationJobStatus::Running,
            status: GenerationJobStatus::Failed,
            finished_at: "2026-07-10T00:00:02Z".to_string(),
            error_code: Some("provider_unavailable".to_string()),
            error_message: None,
            retryable: true,
        },
    );
    assert!(result.is_err(), "failed commit must not return an event");
    let current = get_job(&conn, &queued.id).expect("read rolled-back job");
    assert_eq!(current.status, GenerationJobStatus::Running);
    assert_eq!(current.stage, GenerationJobStage::Preparing);
    assert_eq!(count_table(&conn, "images"), 0);
}

#[test]
fn insert_and_claim_exact_job_commits_only_the_requested_identity_and_leaves_fifo_queued() {
    let fixture = JobFixture::new();
    let older = fixture.enqueue("request-older", "older-fifo");
    let exact = fixture.prepared_at(
        "request-exact",
        "exact-compatibility",
        "2026-07-10T00:00:01Z",
    );

    let snapshot = {
        let mut conn = fixture.open_connection();
        insert_and_claim_exact_job(&mut conn, &exact).expect("insert and claim exact job")
    };

    assert_eq!(snapshot.context.job_id, exact.job_id);
    assert_eq!(snapshot.context.generation_id, exact.generation_id);
    assert_eq!(snapshot.request.prompt, exact.prompt);
    assert_eq!(snapshot.request.model, exact.model);
    assert_eq!(
        fixture.get(&exact.job_id).status,
        GenerationJobStatus::Running
    );
    assert_eq!(fixture.generation_status(&exact.generation_id), "running");
    assert_eq!(fixture.get(&older.id).status, GenerationJobStatus::Queued);
    assert_eq!(fixture.generation_status(&older.generation_id), "queued");
}

#[test]
fn insert_and_claim_exact_job_rolls_back_all_rows_when_snapshot_loading_fails() {
    let fixture = JobFixture::new();
    let exact = fixture.prepared("request-exact", "late-snapshot-failure");
    let counts_before = [
        fixture.count("conversations"),
        fixture.count("generations"),
        fixture.count("generation_recoveries"),
        fixture.count("generation_jobs"),
    ];

    let error = {
        let mut conn = fixture.open_connection();
        conn.execute_batch(
            "CREATE TRIGGER corrupt_exact_snapshot_after_claim
             AFTER UPDATE OF status ON generation_jobs
             WHEN NEW.status = 'running'
               AND NEW.client_request_id = 'request-exact'
             BEGIN
                 UPDATE generations
                 SET request_metadata = '{}'
                 WHERE id = NEW.generation_id;
             END;",
        )
        .expect("install deterministic late failure trigger");
        insert_and_claim_exact_job(&mut conn, &exact)
            .expect_err("corrupt execution snapshot must abort exact insertion")
    };

    assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    assert_eq!(
        counts_before,
        [
            fixture.count("conversations"),
            fixture.count("generations"),
            fixture.count("generation_recoveries"),
            fixture.count("generation_jobs"),
        ]
    );
}

#[test]
fn insert_and_claim_exact_job_rejects_idempotent_hit_with_different_persisted_identity() {
    let fixture = JobFixture::new();
    let first_request = fixture.prepared("request-shared", "same-operation");
    let existing = fixture
        .enqueue_prepared(&first_request)
        .expect("enqueue existing identity");
    let replay_with_new_identity = fixture.prepared("request-shared", "same-operation");
    assert_ne!(replay_with_new_identity.job_id, existing.job_id);
    assert_ne!(
        replay_with_new_identity.generation_id,
        existing.generation_id
    );
    let counts_before = [
        fixture.count("conversations"),
        fixture.count("generations"),
        fixture.count("generation_recoveries"),
        fixture.count("generation_jobs"),
    ];

    let error = {
        let mut conn = fixture.open_connection();
        insert_and_claim_exact_job(&mut conn, &replay_with_new_identity)
            .expect_err("exact compatibility path must reject another persisted identity")
    };

    assert_eq!(stable_code(&error), "generation_job_idempotency_conflict");
    assert_eq!(
        fixture.get(&existing.job_id).status,
        GenerationJobStatus::Queued
    );
    assert_eq!(fixture.generation_status(&existing.generation_id), "queued");
    assert_eq!(
        counts_before,
        [
            fixture.count("conversations"),
            fixture.count("generations"),
            fixture.count("generation_recoveries"),
            fixture.count("generation_jobs"),
        ]
    );
    let conn = fixture.database.conn.lock().expect("lock database");
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM generation_jobs WHERE id = ?1 OR generation_id = ?2",
            params![
                replay_with_new_identity.job_id,
                replay_with_new_identity.generation_id
            ],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        0
    );
}

#[test]
fn exact_claim_in_insert_transaction_never_steals_older_fifo_job_and_rolls_back_cleanly() {
    let fixture = JobFixture::new();
    let older = fixture.enqueue("request-older", "older-fifo");
    let counts_before = [
        fixture.count("conversations"),
        fixture.count("generations"),
        fixture.count("generation_jobs"),
        fixture.count("generation_recoveries"),
        fixture.count("images"),
    ];
    let exact = fixture.prepared_at(
        "request-exact",
        "exact-compatibility",
        "2026-07-10T00:00:01Z",
    );

    let mut conn = fixture.open_connection();
    let tx = begin_generation_job_write_transaction(&mut conn)
        .expect("begin exact insert-and-claim immediate transaction");
    let inserted = insert_job_in_transaction(&tx, &exact).expect("insert exact job");
    let claimed =
        claim_job_in_transaction(&tx, &inserted.job_id).expect("claim exact inserted job");

    assert_eq!(claimed.id, inserted.job_id);
    assert_eq!(claimed.generation_id, inserted.generation_id);
    assert_eq!(claimed.status, GenerationJobStatus::Running);
    assert_eq!(
        get_job_in_transaction(&tx, &older.id).unwrap().status,
        GenerationJobStatus::Queued
    );
    let statuses: (String, String) = tx
        .query_row(
            "SELECT j.status, g.status FROM generation_jobs j
             JOIN generations g ON g.id = j.generation_id WHERE j.id = ?1",
            params![inserted.job_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read exact in-transaction statuses");
    assert_eq!(statuses, ("running".to_string(), "running".to_string()));
    tx.rollback().expect("rollback exact insert-and-claim");

    assert_eq!(fixture.get(&older.id).status, GenerationJobStatus::Queued);
    assert_eq!(fixture.generation_status(&older.generation_id), "queued");
    assert_eq!(
        stable_code(&{
            let conn = fixture.database.conn.lock().expect("lock database");
            get_job(&conn, &exact.job_id).expect_err("rolled-back exact job must not remain")
        }),
        "generation_job_not_found"
    );
    let conn = fixture.database.conn.lock().expect("lock database");
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM generations WHERE id = ?1",
            params![exact.generation_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        0
    );
    drop(conn);
    assert_eq!(
        counts_before,
        [
            fixture.count("conversations"),
            fixture.count("generations"),
            fixture.count("generation_jobs"),
            fixture.count("generation_recoveries"),
            fixture.count("images"),
        ]
    );
}

#[test]
fn exact_claim_rejects_missing_and_nonqueued_ids_without_touching_older_queued_job() {
    let fixture = JobFixture::new();
    let older = fixture.enqueue("request-older", "older-exact-errors");
    let exact = fixture.prepared("request-exact", "exact-errors");

    let mut conn = fixture.open_connection();
    let tx =
        begin_generation_job_write_transaction(&mut conn).expect("begin exact error transaction");
    let missing = claim_job_in_transaction(&tx, "missing-exact-job")
        .expect_err("missing exact claim must fail");
    assert_eq!(stable_code(&missing), "generation_job_not_found");
    assert_eq!(
        get_job_in_transaction(&tx, &older.id).unwrap().status,
        GenerationJobStatus::Queued
    );

    let inserted = insert_job_in_transaction(&tx, &exact).expect("insert exact job");
    let running = claim_job_in_transaction(&tx, &inserted.job_id).expect("claim exact job once");
    assert_eq!(running.status, GenerationJobStatus::Running);
    let nonqueued = claim_job_in_transaction(&tx, &inserted.job_id)
        .expect_err("running exact job must not be claimable again");
    assert_eq!(stable_code(&nonqueued), "generation_job_invalid_transition");
    assert_eq!(
        get_job_in_transaction(&tx, &older.id).unwrap().status,
        GenerationJobStatus::Queued
    );
    tx.rollback().expect("rollback exact error transaction");

    assert_eq!(fixture.get(&older.id).status, GenerationJobStatus::Queued);
    assert_eq!(fixture.count("generation_jobs"), 1);
}

#[test]
fn failed_exact_insert_and_claim_rolls_back_every_inserted_row() {
    let fixture = JobFixture::new();
    let older = fixture.enqueue("request-older", "older-rollback-boundary");
    let counts_before = [
        fixture.count("conversations"),
        fixture.count("generations"),
        fixture.count("generation_recoveries"),
        fixture.count("generation_jobs"),
    ];
    let exact = fixture.prepared("request-exact", "exact-rollback-boundary");

    let error = (|| -> Result<(), AppError> {
        let mut conn = fixture.open_connection();
        let tx = begin_generation_job_write_transaction(&mut conn)?;
        insert_job_in_transaction(&tx, &exact)?;
        claim_job_in_transaction(&tx, "missing-exact-job")?;
        tx.commit().map_err(|error| {
            database_error("Commit exact compatibility transaction failed", error)
        })?;
        Ok(())
    })()
    .expect_err("failed exact claim must roll back the insert transaction");
    assert_eq!(stable_code(&error), "generation_job_not_found");

    let conn = fixture.database.conn.lock().expect("lock database");
    assert_eq!(
        get_job(&conn, &exact.job_id)
            .expect_err("rolled-back exact job must not exist")
            .stable_code(),
        "generation_job_not_found"
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM generations WHERE id = ?1",
            params![exact.generation_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        0
    );
    drop(conn);
    assert_eq!(
        counts_before,
        [
            fixture.count("conversations"),
            fixture.count("generations"),
            fixture.count("generation_recoveries"),
            fixture.count("generation_jobs"),
        ]
    );
    assert_eq!(fixture.get(&older.id).status, GenerationJobStatus::Queued);
}

#[test]
fn execution_snapshot_loads_only_running_persisted_values_without_refilling_options() {
    let fixture = JobFixture::new();
    let mut request = fixture.prepared("request-snapshot", "execution-snapshot");
    request.requested_project_id = None;
    request.size = "1536x1024".to_string();
    request.quality = "medium".to_string();
    request.background = "transparent".to_string();
    request.output_format = "webp".to_string();
    request.output_compression = 73;
    request.moderation = "low".to_string();
    request.input_fidelity = "low".to_string();
    request.image_count = 3;
    request.stream = true;
    request.partial_images = 2;
    request.request_options = GenerationJobOptions::default();
    request.provider_kind = "openai".to_string();
    request.provider_profile_id = "profile-stored".to_string();
    request.endpoint_snapshot = "https://stored.example.test/v1/images/generations".to_string();
    let queued = fixture
        .enqueue_prepared(&request)
        .expect("enqueue persisted snapshot fixture");

    {
        let conn = fixture.database.conn.lock().expect("lock database");
        let error = load_generation_execution_snapshot(&conn, &queued.job_id)
            .expect_err("queued job must not produce an execution snapshot");
        assert_eq!(stable_code(&error), "generation_job_invalid_transition");
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES
             ('gpt-image-2_api_key', 'sk-current-secret'),
             ('gpt-image-2_endpoint', 'https://current.example.test/changed')",
            [],
        )
        .expect("install conflicting current settings");
    }
    fixture.claim().expect("claim persisted snapshot fixture");

    let snapshot = {
        let conn = fixture.database.conn.lock().expect("lock database");
        load_generation_execution_snapshot(&conn, &queued.job_id)
            .expect("load running execution snapshot")
    };
    assert_eq!(snapshot.context.job_id, queued.job_id);
    assert_eq!(snapshot.context.generation_id, queued.generation_id);
    assert_eq!(snapshot.context.conversation_id, queued.conversation_id);
    assert_eq!(snapshot.context.provider_kind, "openai");
    assert_eq!(snapshot.context.provider_profile_id, "profile-stored");
    assert_eq!(
        snapshot.context.endpoint_url,
        "https://stored.example.test/v1/images/generations"
    );
    assert_eq!(snapshot.context.model, "gpt-image-2");
    assert_eq!(snapshot.request.options, GenerationJobOptions::default());
    assert_eq!(snapshot.request.requested_project_id, None);
    assert_eq!(snapshot.request.prompt, request.prompt);
    assert_eq!(snapshot.request.project_id, "default");
    assert_eq!(snapshot.runtime_options.size, request.size);
    assert_eq!(snapshot.runtime_options.quality, request.quality);
    assert_eq!(snapshot.runtime_options.background, request.background);
    assert_eq!(
        snapshot.runtime_options.output_format,
        request.output_format
    );
    assert_eq!(snapshot.runtime_options.output_compression, 73);
    assert_eq!(snapshot.runtime_options.moderation, request.moderation);
    assert_eq!(
        snapshot.runtime_options.input_fidelity,
        request.input_fidelity
    );
    assert!(snapshot.runtime_options.stream);
    assert_eq!(snapshot.runtime_options.partial_images, 2);
    assert_eq!(snapshot.runtime_options.image_count, 3);
    assert_eq!(snapshot.created_at, request.queued_at);
    assert_eq!(snapshot.output_format, "webp");
    let snapshot_debug = format!("{snapshot:?}");
    assert!(!snapshot_debug.contains("sk-current-secret"));
    assert!(!snapshot_debug.contains("https://current.example.test/changed"));

    let metadata: serde_json::Value = {
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.query_row(
            "SELECT request_metadata FROM generations WHERE id = ?1",
            params![snapshot.context.generation_id],
            |row| row.get::<_, String>(0),
        )
        .map(|raw| serde_json::from_str(&raw).unwrap())
        .unwrap()
    };
    assert!(metadata.get("actual_image_count").is_none());

    {
        let conn = fixture.database.conn.lock().expect("lock database");
        finish_job(
            &conn,
            &GenerationJobTerminalUpdate {
                job_id: snapshot.context.job_id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Failed,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: Some("provider_unavailable".to_string()),
                error_message: None,
                retryable: true,
            },
        )
        .expect("finish snapshot fixture");
        let error = load_generation_execution_snapshot(&conn, &snapshot.context.job_id)
            .expect_err("terminal job must not produce an execution snapshot");
        assert_eq!(stable_code(&error), "generation_job_invalid_transition");
    }
}

#[test]
fn execution_snapshot_loader_reuses_one_read_transaction_across_linked_rows() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("request-consistent-snapshot", "consistent-snapshot");
    fixture.claim().expect("claim consistent snapshot fixture");

    let mut reader = fixture.open_connection();
    let read_tx = reader
        .transaction()
        .expect("begin execution snapshot read transaction");
    assert_eq!(
        get_job_in_transaction(&read_tx, &queued.id).unwrap().status,
        GenerationJobStatus::Running
    );

    let writer = fixture.open_connection();
    let terminal = finish_job(
        &writer,
        &GenerationJobTerminalUpdate {
            job_id: queued.id.clone(),
            expected_status: GenerationJobStatus::Running,
            status: GenerationJobStatus::Failed,
            finished_at: "2026-07-10T00:00:02Z".to_string(),
            error_code: Some("provider_unavailable".to_string()),
            error_message: None,
            retryable: true,
        },
    )
    .expect("commit a concurrent terminal state");
    assert_eq!(terminal.status, GenerationJobStatus::Failed);

    assert_eq!(
        get_job_in_transaction(&read_tx, &queued.id).unwrap().status,
        GenerationJobStatus::Running
    );
    assert_eq!(
        find_job_by_client_request_id_in_transaction(&read_tx, "request-consistent-snapshot")
            .unwrap()
            .unwrap()
            .status,
        GenerationJobStatus::Running
    );
    assert_eq!(
        list_jobs_in_transaction(&read_tx, &GenerationJobFilter::default())
            .unwrap()
            .items[0]
            .status,
        GenerationJobStatus::Running
    );
    assert_eq!(
        find_enqueue_result_by_client_request_id_in_transaction(
            &read_tx,
            "request-consistent-snapshot"
        )
        .unwrap()
        .unwrap()
        .status,
        GenerationJobStatus::Running
    );
    let snapshot = load_generation_execution_snapshot_in_transaction(&read_tx, &queued.id)
        .expect("read transaction must retain its running projection");
    assert_eq!(snapshot.context.job_id, queued.id);
    assert_eq!(snapshot.context.generation_id, queued.generation_id);
    read_tx.commit().expect("commit execution snapshot read");

    let conn = fixture.database.conn.lock().expect("lock database");
    assert_eq!(
        get_job(&conn, &queued.id).unwrap().status,
        GenerationJobStatus::Failed
    );
    assert_eq!(
        find_job_by_client_request_id(&conn, "request-consistent-snapshot")
            .unwrap()
            .unwrap()
            .status,
        GenerationJobStatus::Failed
    );
    assert_eq!(
        list_jobs(&conn, &GenerationJobFilter::default())
            .unwrap()
            .items[0]
            .status,
        GenerationJobStatus::Failed
    );
    assert_eq!(
        find_enqueue_result_by_client_request_id(&conn, "request-consistent-snapshot")
            .unwrap()
            .unwrap()
            .status,
        GenerationJobStatus::Failed
    );
    let snapshot_error = load_generation_execution_snapshot(&conn, &queued.id)
        .expect_err("a fresh snapshot must observe the terminal state");
    assert_eq!(
        stable_code(&snapshot_error),
        "generation_job_invalid_transition"
    );
}

#[test]
fn hundred_row_list_projection_uses_exactly_main_plus_four_batch_queries() {
    let fixture = JobFixture::new();
    let mut writer = fixture.open_connection();
    let write_tx = begin_generation_job_write_transaction(&mut writer)
        .expect("begin hundred-row insert transaction");
    for index in 0..101 {
        insert_job_in_transaction(
            &write_tx,
            &fixture.prepared(
                &format!("request-batch-{index:03}"),
                &format!("batch-{index:03}"),
            ),
        )
        .expect("insert batch projection fixture");
    }
    write_tx.commit().expect("commit batch projection fixtures");

    let mut reader = fixture.open_connection();
    let read_tx = reader.transaction().expect("begin batch list read");
    let query_count = Cell::new(0usize);
    let mut observe_query = || query_count.set(query_count.get() + 1);
    let first = list_jobs_in_transaction_with_query_observer(
        &read_tx,
        &GenerationJobFilter {
            limit: Some(100),
            ..GenerationJobFilter::default()
        },
        &mut observe_query,
    )
    .expect("list first hundred-row page");
    assert_eq!(first.items.len(), 100);
    assert_eq!(first.items[0].client_request_id, "request-batch-000");
    assert_eq!(first.items[99].client_request_id, "request-batch-099");
    assert!(first.next_cursor.is_some());
    assert_eq!(query_count.get(), 5, "main query plus four batch queries");

    query_count.set(0);
    let second = list_jobs_in_transaction_with_query_observer(
        &read_tx,
        &GenerationJobFilter {
            limit: Some(100),
            cursor: first.next_cursor,
            ..GenerationJobFilter::default()
        },
        &mut observe_query,
    )
    .expect("list second batch page");
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].client_request_id, "request-batch-100");
    assert!(second.next_cursor.is_none());
    assert_eq!(
        query_count.get(),
        5,
        "small pages keep the fixed query plan"
    );

    query_count.set(0);
    let empty = list_jobs_in_transaction_with_query_observer(
        &read_tx,
        &GenerationJobFilter {
            generation_id: Some("missing-generation".to_string()),
            limit: Some(100),
            ..GenerationJobFilter::default()
        },
        &mut observe_query,
    )
    .expect("list empty batch page");
    assert!(empty.items.is_empty());
    assert!(empty.next_cursor.is_none());
    assert_eq!(
        query_count.get(),
        5,
        "empty pages keep the fixed query plan"
    );
    read_tx.commit().expect("commit batch list read");
}

#[test]
fn completed_projection_accepts_short_result_and_preserves_requested_count() {
    let (fixture, completed) = completed_projection_fixture(3, 2, "short-result");
    assert_eq!(completed.status, GenerationJobStatus::Completed);
    assert_eq!(fixture.count("images"), 2);
    assert_eq!(fixture.count("generation_recoveries"), 0);

    let conn = fixture.database.conn.lock().expect("lock database");
    let (requested, metadata_raw): (i32, String) = conn
        .query_row(
            "SELECT image_count, request_metadata FROM generations WHERE id = ?1",
            params![completed.generation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read completed generation projection");
    let metadata: serde_json::Value =
        serde_json::from_str(&metadata_raw).expect("parse completed metadata");
    assert_eq!(requested, 3);
    assert_eq!(metadata["image_count"], json!(3));
    assert_eq!(metadata["actual_image_count"], json!(2));
    get_job(&conn, &completed.id).expect("completed short result remains valid");
}

#[test]
fn every_non_completed_status_requires_no_actual_count_and_zero_images() {
    let statuses = [
        GenerationJobStatus::Queued,
        GenerationJobStatus::Running,
        GenerationJobStatus::Failed,
        GenerationJobStatus::Interrupted,
        GenerationJobStatus::Cancelled,
    ];

    for (index, status) in statuses.into_iter().enumerate() {
        let metadata_fixture = JobFixture::new();
        let metadata_job = transition_fixture_job(
            &metadata_fixture,
            status.clone(),
            &format!("metadata-{index}"),
        );
        {
            let conn = metadata_fixture
                .database
                .conn
                .lock()
                .expect("lock metadata fixture");
            set_actual_image_count(&conn, &metadata_job.generation_id, Some(1));
            let error = get_job(&conn, &metadata_job.id)
                .expect_err("non-completed actual count must be corrupt");
            assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        }

        let image_fixture = JobFixture::new();
        let image_job = transition_fixture_job(&image_fixture, status, &format!("image-{index}"));
        let conn = image_fixture
            .database
            .conn
            .lock()
            .expect("lock image fixture");
        insert_generation_images(&conn, &image_job.generation_id, 1);
        let error =
            get_job(&conn, &image_job.id).expect_err("non-completed image rows must be corrupt");
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    }
}

#[test]
fn completed_projection_requires_actual_range_image_count_paths_and_no_recovery() {
    for (requested, actual, suffix) in [(1, 0, "zero-actual"), (1, 2, "over-requested")] {
        let fixture = JobFixture::new();
        let mut request = fixture.prepared(&format!("request-{suffix}"), suffix);
        request.image_count = requested;
        request.request_options.image_count = Some(requested as u8);
        let queued = fixture
            .enqueue_prepared(&request)
            .expect("enqueue invalid range");
        fixture.claim().expect("claim invalid range fixture");
        let conn = fixture.database.conn.lock().expect("lock database");
        let tx = conn
            .unchecked_transaction()
            .expect("begin invalid range tx");
        insert_generation_images(&tx, &queued.generation_id, actual);
        set_actual_image_count(&tx, &queued.generation_id, Some(actual));
        tx.execute(
            "DELETE FROM generation_recoveries WHERE generation_id = ?1",
            params![queued.generation_id],
        )
        .expect("delete invalid range recovery");
        let error = finish_job_in_transaction(
            &tx,
            &GenerationJobTerminalUpdate {
                job_id: queued.job_id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Completed,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: None,
                error_message: None,
                retryable: false,
            },
        )
        .expect_err("invalid actual count must reject completion");
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        tx.rollback().expect("rollback invalid completion");
        drop(conn);
        assert_eq!(
            fixture.get(&queued.job_id).status,
            GenerationJobStatus::Running
        );
        assert_eq!(fixture.count("images"), 0);
        assert_eq!(fixture.count("generation_recoveries"), 1);
    }

    let missing_actual_fixture = JobFixture::new();
    let missing_actual = missing_actual_fixture.enqueue("request-missing-actual", "missing-actual");
    missing_actual_fixture
        .claim()
        .expect("claim missing actual fixture");
    {
        let conn = missing_actual_fixture
            .database
            .conn
            .lock()
            .expect("lock missing actual fixture");
        let tx = conn
            .unchecked_transaction()
            .expect("begin missing actual tx");
        insert_generation_images(&tx, &missing_actual.generation_id, 1);
        tx.execute(
            "DELETE FROM generation_recoveries WHERE generation_id = ?1",
            params![missing_actual.generation_id],
        )
        .expect("delete missing actual recovery");
        let error = finish_job_in_transaction(
            &tx,
            &GenerationJobTerminalUpdate {
                job_id: missing_actual.id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Completed,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: None,
                error_message: None,
                retryable: false,
            },
        )
        .expect_err("missing actual count must reject completion");
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        tx.rollback().expect("rollback missing actual completion");
    }

    let recovery_fixture = JobFixture::new();
    let recovery_job = recovery_fixture.enqueue("request-recovery", "completed-recovery");
    recovery_fixture
        .claim()
        .expect("claim completed recovery fixture");
    {
        let conn = recovery_fixture
            .database
            .conn
            .lock()
            .expect("lock recovery fixture");
        let tx = conn.unchecked_transaction().expect("begin recovery tx");
        insert_generation_images(&tx, &recovery_job.generation_id, 1);
        set_actual_image_count(&tx, &recovery_job.generation_id, Some(1));
        let error = finish_job_in_transaction(
            &tx,
            &GenerationJobTerminalUpdate {
                job_id: recovery_job.id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Completed,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: None,
                error_message: None,
                retryable: false,
            },
        )
        .expect_err("completed recovery must be removed before finish");
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        tx.rollback().expect("rollback recovery completion");
    }

    for (index, mutation) in ["metadata", "count", "path"].into_iter().enumerate() {
        let (fixture, completed) = completed_projection_fixture(3, 2, &format!("tamper-{index}"));
        let conn = fixture.database.conn.lock().expect("lock tamper fixture");
        match mutation {
            "metadata" => set_actual_image_count(&conn, &completed.generation_id, Some(1)),
            "count" => {
                conn.execute(
                    "DELETE FROM images WHERE id = (
                         SELECT id FROM images WHERE generation_id = ?1 ORDER BY id LIMIT 1
                     )",
                    params![completed.generation_id],
                )
                .expect("tamper completed image count");
            }
            "path" => {
                conn.execute(
                    "UPDATE images SET file_path = '' WHERE generation_id = ?1",
                    params![completed.generation_id],
                )
                .expect("tamper completed image path");
            }
            _ => unreachable!(),
        }
        let error = get_job(&conn, &completed.id).expect_err("tampered completion must be corrupt");
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    }
}

#[test]
fn retry_metadata_never_inherits_actual_image_count() {
    let fixture = JobFixture::new();
    let failed = fixture.fail_retryable("request-retry-actual", "retry-actual");
    let child = {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        create_retry_job(&mut conn, &failed.id, "retry-actual-child").expect("create retry child")
    };
    let conn = fixture.database.conn.lock().expect("lock database");
    let child_metadata: serde_json::Value = conn
        .query_row(
            "SELECT request_metadata FROM generations WHERE id = ?1",
            params![child.generation_id],
            |row| row.get::<_, String>(0),
        )
        .map(|raw| serde_json::from_str(&raw).unwrap())
        .unwrap();
    assert!(child_metadata.get("actual_image_count").is_none());
    drop(conn);

    let corrupt_fixture = JobFixture::new();
    let corrupt_parent =
        corrupt_fixture.fail_retryable("request-corrupt-actual", "corrupt-retry-actual");
    {
        let conn = corrupt_fixture
            .database
            .conn
            .lock()
            .expect("lock corrupt parent");
        set_actual_image_count(&conn, &corrupt_parent.generation_id, Some(1));
    }
    let error = {
        let mut conn = corrupt_fixture
            .database
            .conn
            .lock()
            .expect("lock corrupt retry");
        create_retry_job(&mut conn, &corrupt_parent.id, "corrupt-retry-child")
            .expect_err("retry must reject parent actual count")
    };
    assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    assert_eq!(corrupt_fixture.count("generation_jobs"), 1);
    assert_eq!(corrupt_fixture.count("generations"), 1);
}

#[test]
fn claim_rejects_corrupt_unresolved_queued_snapshot_before_transition() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("request-1", "corrupt-claim");
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            "UPDATE generation_jobs
             SET provider_kind = 'unresolved', provider_profile_id = 'unresolved',
                 endpoint_snapshot = ''
             WHERE id = ?1",
            params![queued.id],
        )
        .expect("corrupt queued provider snapshot");
    }

    let error = {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        claim_next_job(&mut conn).expect_err("corrupt queued row must not be claimed")
    };
    assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    let conn = fixture.database.conn.lock().expect("lock database");
    let statuses: (String, String) = conn
        .query_row(
            "SELECT j.status, g.status
             FROM generation_jobs j JOIN generations g ON g.id = j.generation_id
             WHERE j.id = ?1",
            params![queued.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("read rolled-back statuses");
    assert_eq!(statuses, ("queued".to_string(), "queued".to_string()));
}

#[test]
fn claim_rejects_blank_provider_identity_or_nonzero_queued_attempt() {
    for (index, mutation) in [
        "provider_kind = '   ', provider_profile_id = ''",
        "auto_attempt = 1",
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = JobFixture::new();
        let queued = fixture.enqueue(&format!("request-{index}"), "bad-queued-state");
        {
            let conn = fixture.database.conn.lock().expect("lock database");
            conn.execute(
                &format!("UPDATE generation_jobs SET {mutation} WHERE id = ?1"),
                params![queued.id],
            )
            .expect("corrupt queued state");
        }
        let error = {
            let mut conn = fixture.database.conn.lock().expect("lock database");
            claim_next_job(&mut conn).expect_err("malformed queued row must not be claimed")
        };
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    }
}

#[test]
fn linked_generation_corruption_is_rejected_by_get_list_ack_and_claim_without_transition() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("request-1", "projection-corruption");
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            "UPDATE generations SET prompt = 'different prompt' WHERE id = ?1",
            params![queued.generation_id],
        )
        .expect("corrupt linked prompt");

        for error in [
            get_job(&conn, &queued.id).expect_err("get must validate linked generation"),
            list_jobs(
                &conn,
                &GenerationJobFilter {
                    generation_id: Some(queued.generation_id.clone()),
                    ..GenerationJobFilter::default()
                },
            )
            .expect_err("list must validate linked generation"),
            find_enqueue_result_by_client_request_id(&conn, "request-1")
                .expect_err("ack must validate linked generation"),
        ] {
            assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        }
    }

    let before = {
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.query_row(
            "SELECT j.status, g.status FROM generation_jobs j
             JOIN generations g ON g.id = j.generation_id WHERE j.id = ?1",
            params![queued.id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .expect("read statuses before claim")
    };
    let error = {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        claim_next_job(&mut conn).expect_err("claim must validate linked generation")
    };
    assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    let after = {
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.query_row(
            "SELECT j.status, g.status FROM generation_jobs j
             JOIN generations g ON g.id = j.generation_id WHERE j.id = ?1",
            params![queued.id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .expect("read statuses after claim")
    };
    assert_eq!(before, after);
}

#[test]
fn claim_rejects_every_linked_generation_and_requesting_recovery_mismatch() {
    let mutations = [
        "UPDATE generations SET prompt = 'other' WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generations SET engine = 'other-model' WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generations SET request_kind = 'edit' WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generations SET size = '2048x2048' WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generations SET quality = 'low' WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generations SET background = 'opaque' WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generations SET output_format = 'jpeg' WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generations SET output_compression = 42 WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generations SET moderation = 'low' WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generations SET input_fidelity = 'low' WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generations SET image_count = 2 WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generations SET source_image_count = 1 WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generations SET source_image_paths = '[\"/tmp/other.png\"]' WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generations SET request_metadata = '{}' WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generations SET status = 'pending' WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generations SET created_at = '2026-07-10T00:00:01Z' WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generations SET deleted_at = '2026-07-10T00:00:01Z' WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "INSERT INTO conversations (id, project_id, title) VALUES ('other-conversation', 'default', 'Other'); UPDATE generations SET conversation_id = 'other-conversation' WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "DELETE FROM generation_recoveries WHERE generation_id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generation_recoveries SET request_kind = 'edit' WHERE generation_id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generation_recoveries SET request_state = 'response_ready' WHERE generation_id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generation_recoveries SET output_format = 'jpeg' WHERE generation_id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generation_recoveries SET response_file = '/tmp/response.json' WHERE generation_id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generation_recoveries SET created_at = '2026-07-10T00:00:01Z' WHERE generation_id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
        "UPDATE generation_recoveries SET updated_at = '2026-07-10T00:00:01Z' WHERE generation_id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1')",
    ];

    for (index, mutation) in mutations.into_iter().enumerate() {
        let fixture = JobFixture::new();
        let queued = fixture.enqueue("request-1", &format!("corruption-{index}"));
        let before = {
            let conn = fixture.database.conn.lock().expect("lock database");
            conn.execute_batch(mutation)
                .expect("apply corruption mutation");
            conn.query_row(
                "SELECT j.status, g.status FROM generation_jobs j
                 JOIN generations g ON g.id = j.generation_id WHERE j.id = ?1",
                params![queued.id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .expect("read preclaim statuses")
        };

        let error = {
            let mut conn = fixture.database.conn.lock().expect("lock database");
            claim_next_job(&mut conn).expect_err("corrupt snapshot must not claim")
        };
        assert_eq!(
            stable_code(&error),
            "generation_job_corrupt_persisted_data",
            "unexpected error for mutation {index}: {mutation}"
        );
        let after = {
            let conn = fixture.database.conn.lock().expect("lock database");
            conn.query_row(
                "SELECT j.status, g.status FROM generation_jobs j
                 JOIN generations g ON g.id = j.generation_id WHERE j.id = ?1",
                params![queued.id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .expect("read postclaim statuses")
        };
        assert_eq!(before, after, "claim mutated corruption case {index}");
    }
}

#[test]
fn stale_expected_status_cannot_finish_or_mutate_generation() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("request-1", "stale-finish");
    let update = GenerationJobTerminalUpdate {
        job_id: queued.id.clone(),
        expected_status: GenerationJobStatus::Running,
        status: GenerationJobStatus::Failed,
        finished_at: "2026-07-10T00:00:01Z".to_string(),
        error_code: Some("provider_error".to_string()),
        error_message: Some("Provider request failed".to_string()),
        retryable: true,
    };

    let conn = fixture.database.conn.lock().expect("lock database");
    let error = finish_job(&conn, &update).expect_err("stale transition must fail");
    drop(conn);
    assert_eq!(stable_code(&error), "generation_job_invalid_transition");
    assert_eq!(fixture.get(&queued.id).status, GenerationJobStatus::Queued);
    assert_eq!(fixture.generation_status(&queued.generation_id), "queued");
}

#[test]
fn composable_finish_allows_recovery_cleanup_earlier_in_the_same_transaction() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("request-1", "outer-finish");
    fixture.claim().expect("claim outer finish job");

    let mut conn = fixture.open_connection();
    let tx = conn
        .transaction()
        .expect("begin terminal outer transaction");
    prepare_completed_projection(&tx, &queued.generation_id, 1);
    let completed = finish_job_in_transaction(
        &tx,
        &GenerationJobTerminalUpdate {
            job_id: queued.id.clone(),
            expected_status: GenerationJobStatus::Running,
            status: GenerationJobStatus::Completed,
            finished_at: "2026-07-10T00:00:02Z".to_string(),
            error_code: None,
            error_message: None,
            retryable: false,
        },
    )
    .expect("finish inside outer transaction after recovery cleanup");
    assert_eq!(completed.status, GenerationJobStatus::Completed);
    assert_eq!(completed.stage, GenerationJobStage::Terminal);
    tx.commit().expect("commit terminal outer transaction");

    assert_eq!(
        fixture.get(&queued.id).status,
        GenerationJobStatus::Completed
    );
    assert_eq!(fixture.count("generation_recoveries"), 0);
}

#[test]
fn queued_cancel_updates_job_and_generation_while_running_cancel_only_stamps_request() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("request-1", "cancel-queued");
    let cancelled = {
        let conn = fixture.database.conn.lock().expect("lock database");
        request_cancel(&conn, &queued.id).expect("cancel queued job")
    };
    assert_eq!(cancelled.status, GenerationJobStatus::Cancelled);
    assert_eq!(cancelled.stage, GenerationJobStage::Terminal);
    assert!(cancelled.cancel_requested_at.is_some());
    assert!(cancelled.finished_at.is_some());
    assert_eq!(
        fixture.generation_status(&queued.generation_id),
        "cancelled"
    );
    assert_eq!(fixture.count("generation_recoveries"), 0);

    let running = fixture.enqueue("request-2", "cancel-running");
    let claimed = fixture.claim().expect("claim running job");
    assert_eq!(claimed.id, running.id);
    let requested = {
        let conn = fixture.database.conn.lock().expect("lock database");
        request_cancel(&conn, &running.id).expect("request running cancellation")
    };
    assert_eq!(requested.status, GenerationJobStatus::Running);
    assert_eq!(requested.stage, GenerationJobStage::Preparing);
    assert!(requested.cancel_requested_at.is_some());
    assert!(requested.finished_at.is_none());
    assert_eq!(fixture.generation_status(&running.generation_id), "running");
    assert_eq!(fixture.count("generation_recoveries"), 1);
}

#[test]
fn concurrent_claim_and_cancel_never_lose_the_durable_cancel_request() {
    const ROUNDS: usize = 64;
    let fixture = JobFixture::new();

    for round in 0..ROUNDS {
        let queued = fixture.enqueue(&format!("claim-cancel-{round}"), "claim-cancel-race");
        let mut claim_conn = fixture.open_connection();
        let cancel_conn = fixture.open_connection();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));

        let claim_barrier = std::sync::Arc::clone(&barrier);
        let claim = std::thread::spawn(move || {
            claim_barrier.wait();
            claim_next_job(&mut claim_conn)
        });
        let cancel_barrier = std::sync::Arc::clone(&barrier);
        let job_id = queued.id.clone();
        let cancel = std::thread::spawn(move || {
            cancel_barrier.wait();
            request_cancel(&cancel_conn, &job_id)
        });

        barrier.wait();
        let claim_result = claim.join().expect("claim thread must not panic");
        let cancel_result = cancel.join().expect("cancel thread must not panic");
        assert!(
            claim_result.is_ok(),
            "round {round} claim returned database error: {claim_result:?}"
        );
        assert!(
            cancel_result.is_ok(),
            "round {round} cancel returned database error: {cancel_result:?}"
        );

        let final_job = fixture.get(&queued.id);
        match final_job.status {
            GenerationJobStatus::Cancelled => {
                assert_eq!(final_job.stage, GenerationJobStage::Terminal);
                assert!(final_job.cancel_requested_at.is_some());
                assert!(final_job.finished_at.is_some());
            }
            GenerationJobStatus::Running => {
                assert_eq!(final_job.stage, GenerationJobStage::Preparing);
                assert!(
                    final_job.cancel_requested_at.is_some(),
                    "round {round} lost the durable cancellation request"
                );
                assert!(final_job.finished_at.is_none());
            }
            status => panic!("round {round} ended in unexpected status {status:?}"),
        }
    }
}

#[test]
fn queued_cancel_rolls_back_when_requesting_recovery_is_missing_or_not_requesting() {
    for (index, mutation) in [
        "DELETE FROM generation_recoveries WHERE generation_id = ?1",
        "UPDATE generation_recoveries SET request_state = 'response_ready' WHERE generation_id = ?1",
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = JobFixture::new();
        let queued = fixture.enqueue(&format!("request-{index}"), "bad-cancel-recovery");
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(mutation, params![queued.generation_id])
            .expect("corrupt recovery");
        let error = request_cancel(&conn, &queued.id)
            .expect_err("queued cancellation requires requesting recovery");
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        let statuses: (String, String) = conn
            .query_row(
                "SELECT j.status, g.status FROM generation_jobs j
                 JOIN generations g ON g.id = j.generation_id WHERE j.id = ?1",
                params![queued.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read rolled-back statuses");
        assert_eq!(statuses, ("queued".to_string(), "queued".to_string()));
    }
}

#[test]
fn cancel_request_wins_over_non_cancel_terminal_update() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("request-1", "cancel-first");
    fixture.claim().expect("claim cancel-first job");
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        request_cancel(&conn, &queued.id).expect("persist cancellation request");
    }

    let conn = fixture.database.conn.lock().expect("lock database");
    let error = finish_job(
        &conn,
        &GenerationJobTerminalUpdate {
            job_id: queued.id.clone(),
            expected_status: GenerationJobStatus::Running,
            status: GenerationJobStatus::Completed,
            finished_at: "2026-07-10T00:00:02Z".to_string(),
            error_code: None,
            error_message: None,
            retryable: false,
        },
    )
    .expect_err("cancel-first must block completion");
    drop(conn);
    assert_eq!(stable_code(&error), "generation_job_invalid_transition");
    let job = fixture.get(&queued.id);
    assert_eq!(job.status, GenerationJobStatus::Running);
    assert!(job.cancel_requested_at.is_some());
    assert_eq!(fixture.generation_status(&queued.generation_id), "running");
}

#[test]
fn running_job_cannot_be_cancelled_without_durable_request() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("request-1", "spontaneous-cancel");
    fixture.claim().expect("claim spontaneous cancel job");
    let conn = fixture.database.conn.lock().expect("lock database");
    let error = finish_job(
        &conn,
        &GenerationJobTerminalUpdate {
            job_id: queued.id.clone(),
            expected_status: GenerationJobStatus::Running,
            status: GenerationJobStatus::Cancelled,
            finished_at: "2026-07-10T00:00:02Z".to_string(),
            error_code: Some("cancelled_by_user".to_string()),
            error_message: Some("The operation was cancelled".to_string()),
            retryable: false,
        },
    )
    .expect_err("cancel transition requires a durable request");
    drop(conn);
    assert_eq!(stable_code(&error), "generation_job_invalid_transition");
    assert_eq!(fixture.get(&queued.id).status, GenerationJobStatus::Running);
}

#[test]
fn failed_retry_creates_immutable_child_with_fresh_generation_and_reset_attempts() {
    let fixture = JobFixture::new();
    let failed = fixture.fail_retryable("request-1", "retry-parent");
    let parent_before = serde_json::to_value(&failed).expect("serialize parent");

    let retry_result = {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        create_retry_job(&mut conn, &failed.id, "retry-request-1").expect("create retry")
    };
    let retry = fixture.get(&retry_result.job_id);

    assert_eq!(retry.parent_job_id.as_deref(), Some(failed.id.as_str()));
    assert_ne!(retry.generation_id, failed.generation_id);
    assert_eq!(retry.chain_attempt, failed.chain_attempt + 1);
    assert_eq!(retry.auto_attempt, 0);
    assert_eq!(retry.status, GenerationJobStatus::Queued);
    assert_eq!(retry.stage, GenerationJobStage::Queued);
    assert_eq!(retry.request, failed.request);
    assert_eq!(retry.provider_kind, failed.provider_kind);
    assert_eq!(retry.provider_profile_id, failed.provider_profile_id);
    assert_eq!(retry.endpoint_snapshot, failed.endpoint_snapshot);
    assert!(retry.error_code.is_none());
    assert!(retry.error_message.is_none());
    assert!(retry.finished_at.is_none());
    assert_eq!(fixture.generation_status(&retry.generation_id), "queued");
    assert_eq!(
        serde_json::to_value(fixture.get(&failed.id)).expect("serialize unchanged parent"),
        parent_before
    );
}

#[test]
fn optional_request_options_preserve_exact_presence_through_enqueue_get_and_retry() {
    let fixture = JobFixture::new();
    let mut request = fixture.prepared("request-1", "optional-options");
    request.request_options.quality = None;
    request.request_options.background = None;
    request.request_options.output_format = None;
    request.request_options.output_compression = None;
    request.request_options.moderation = None;
    request.request_options.input_fidelity = None;
    request.request_options.image_count = None;
    request.request_options.stream = Some(false);
    request.request_options.partial_images = Some(0);

    let queued = fixture
        .enqueue_prepared(&request)
        .expect("enqueue optional options");
    fixture.claim().expect("claim optional options parent");
    let failed = {
        let conn = fixture.database.conn.lock().expect("lock database");
        finish_job(
            &conn,
            &GenerationJobTerminalUpdate {
                job_id: queued.job_id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Failed,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: Some("provider_unavailable".to_string()),
                error_message: None,
                retryable: true,
            },
        )
        .expect("fail optional options parent")
    };
    let child = {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        create_retry_job(&mut conn, &failed.id, "retry-request-1")
            .expect("retry optional options parent")
    };

    let parent_options = failed.request["options"]
        .as_object()
        .expect("parent options object");
    assert_eq!(
        parent_options
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["partial_images", "size", "stream"]
    );
    assert_eq!(parent_options["stream"], json!(false));
    assert_eq!(parent_options["partial_images"], json!(0));
    let child_job = fixture.get(&child.job_id);
    assert_eq!(child_job.request["options"], failed.request["options"]);
}

#[test]
fn present_request_option_must_match_its_normalized_generation_value() {
    let fixture = JobFixture::new();
    let mut request = fixture.prepared("request-1", "mismatched-option");
    request.request_options.quality = Some("low".to_string());

    let error = fixture
        .enqueue_prepared(&request)
        .expect_err("present option must match normalized value");
    assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
    assert_eq!(fixture.count("conversations"), 0);
    assert_eq!(fixture.count("generation_jobs"), 0);
}

#[test]
fn retry_client_request_reuse_for_different_parent_is_conflict_without_side_effects() {
    let fixture = JobFixture::new();
    let first = fixture.fail_retryable("request-1", "parent-one");
    let second = fixture.fail_retryable("request-2", "parent-two");
    {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        create_retry_job(&mut conn, &first.id, "retry-request").expect("first retry");
    }
    let jobs_before = fixture.count("generation_jobs");
    let generations_before = fixture.count("generations");

    let error = {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        create_retry_job(&mut conn, &second.id, "retry-request")
            .expect_err("cross-parent idempotency reuse must fail")
    };
    assert_eq!(stable_code(&error), "generation_job_idempotency_conflict");
    assert_eq!(fixture.count("generation_jobs"), jobs_before);
    assert_eq!(fixture.count("generations"), generations_before);
}

#[test]
fn retry_client_request_reuse_for_same_parent_returns_existing_child() {
    let fixture = JobFixture::new();
    let parent = fixture.fail_retryable("request-1", "parent");
    let first = {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        create_retry_job(&mut conn, &parent.id, "retry-request").expect("first retry")
    };
    let counts = (
        fixture.count("conversations"),
        fixture.count("generations"),
        fixture.count("generation_jobs"),
        fixture.count("generation_recoveries"),
    );
    let repeated = {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        create_retry_job(&mut conn, &parent.id, "retry-request").expect("same retry is idempotent")
    };
    assert_eq!(first.job_id, repeated.job_id);
    assert_eq!(first.generation_id, repeated.generation_id);
    assert_eq!(
        counts,
        (
            fixture.count("conversations"),
            fixture.count("generations"),
            fixture.count("generation_jobs"),
            fixture.count("generation_recoveries"),
        )
    );
}

#[test]
fn progressed_retry_child_remains_idempotently_addressable() {
    let fixture = JobFixture::new();
    let parent = fixture.fail_retryable("request-1", "progressed-parent");
    let child = {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        create_retry_job(&mut conn, &parent.id, "retry-request").expect("create retry")
    };
    let claimed = fixture.claim().expect("claim retry child");
    assert_eq!(claimed.id, child.job_id);
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            "UPDATE generation_jobs SET auto_attempt = 1 WHERE id = ?1 AND status = 'running'",
            params![child.job_id],
        )
        .expect("persist automatic attempt");
    }

    let repeated = {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        create_retry_job(&mut conn, &parent.id, "retry-request")
            .expect("progress does not change retry identity")
    };
    assert_eq!(repeated.job_id, child.job_id);
    assert_eq!(fixture.get(&child.job_id).auto_attempt, 1);
}

#[test]
fn retry_client_request_reuse_for_different_source_override_is_conflict() {
    let fixture = JobFixture::new();
    let parent = fixture.fail_retryable("request-1", "source-aware-parent");
    {
        let mut conn = fixture.open_connection();
        let tx = conn.transaction().expect("begin first source-aware retry");
        insert_retry_job_in_transaction(
            &tx,
            &parent.id,
            "retry-request",
            Some(&json!({ "id": "override-one" })),
        )
        .expect("insert source-aware retry");
        tx.commit().expect("commit source-aware retry");
    }
    let jobs_before = fixture.count("generation_jobs");

    let mut conn = fixture.open_connection();
    let tx = conn.transaction().expect("begin conflicting retry");
    let error = insert_retry_job_in_transaction(
        &tx,
        &parent.id,
        "retry-request",
        Some(&json!({ "id": "override-two" })),
    )
    .expect_err("different source override is a different logical retry");
    assert_eq!(stable_code(&error), "generation_job_idempotency_conflict");
    tx.rollback().expect("rollback conflicting retry");
    assert_eq!(fixture.count("generation_jobs"), jobs_before);
}

#[test]
fn retry_requires_retryable_failed_or_interrupted_parent() {
    let fixture = JobFixture::new();
    let nonretryable = fixture.enqueue("request-1", "nonretryable-failed");
    fixture.claim().expect("claim nonretryable job");
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        finish_job(
            &conn,
            &GenerationJobTerminalUpdate {
                job_id: nonretryable.id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Failed,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: Some("invalid_request".to_string()),
                error_message: Some("The request cannot be retried".to_string()),
                retryable: false,
            },
        )
        .expect("finish nonretryable job");
    }
    let error = {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        create_retry_job(&mut conn, &nonretryable.id, "retry-nonretryable")
            .expect_err("failed nonretryable job must reject retry")
    };
    assert_eq!(stable_code(&error), "generation_job_not_retryable");

    let interrupted = fixture.enqueue("request-2", "retryable-interrupted");
    fixture.claim().expect("claim interrupted job");
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        finish_job(
            &conn,
            &GenerationJobTerminalUpdate {
                job_id: interrupted.id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Interrupted,
                finished_at: "2026-07-10T00:00:03Z".to_string(),
                error_code: Some("app_interrupted".to_string()),
                error_message: Some("The operation was interrupted".to_string()),
                retryable: true,
            },
        )
        .expect("finish interrupted job");
    }
    let retry = {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        create_retry_job(&mut conn, &interrupted.id, "retry-interrupted")
            .expect("retry interrupted job")
    };
    assert_eq!(retry.status, GenerationJobStatus::Queued);
}

#[test]
fn retry_rejects_chain_attempt_overflow_without_panicking_or_writing() {
    let fixture = JobFixture::new();
    let parent = fixture.fail_retryable("request-1", "overflow-parent");
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            "UPDATE generation_jobs SET chain_attempt = ?1 WHERE id = ?2",
            params![i32::MAX, parent.id],
        )
        .expect("set maximum chain attempt");
    }
    let count_before = fixture.count("generation_jobs");
    let error = {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        create_retry_job(&mut conn, &parent.id, "retry-overflow")
            .expect_err("overflowed retry chain must fail safely")
    };
    assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    assert_eq!(fixture.count("generation_jobs"), count_before);
}

#[test]
fn generic_retry_rejects_unsupported_source_kind() {
    let fixture = JobFixture::new();
    let mut request = fixture.prepared("request-1", "canvas-parent");
    request.source_kind = "canvas".to_string();
    let queued = fixture
        .enqueue_prepared(&request)
        .expect("enqueue canvas job");
    fixture.claim().expect("claim canvas job");
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        finish_job(
            &conn,
            &GenerationJobTerminalUpdate {
                job_id: queued.job_id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Failed,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: Some("canvas_error".to_string()),
                error_message: Some("Canvas generation failed".to_string()),
                retryable: true,
            },
        )
        .expect("fail canvas job");
    }

    let error = {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        create_retry_job(&mut conn, &queued.job_id, "retry-request")
            .expect_err("generic canvas retry must fail")
    };
    assert_eq!(stable_code(&error), "generation_job_source_unsupported");
    assert_eq!(fixture.count("generation_jobs"), 1);
}

#[test]
fn source_aware_retry_supports_canvas_but_generic_replay_rejects_it() {
    let fixture = JobFixture::new();
    let mut request = fixture.prepared("request-1", "canvas-source-aware");
    request.source_kind = "canvas".to_string();
    let queued = fixture
        .enqueue_prepared(&request)
        .expect("enqueue canvas parent");
    fixture.claim().expect("claim canvas parent");
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        finish_job(
            &conn,
            &GenerationJobTerminalUpdate {
                job_id: queued.job_id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Failed,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: Some("canvas_error".to_string()),
                error_message: Some("Canvas generation failed".to_string()),
                retryable: true,
            },
        )
        .expect("fail canvas parent");
    }
    let child = {
        let mut conn = fixture.open_connection();
        let tx = conn.transaction().expect("begin source-aware retry");
        let result = insert_retry_job_in_transaction(
            &tx,
            &queued.job_id,
            "canvas-retry-request",
            Some(&json!({ "id": "canvas-round-2" })),
        )
        .expect("source-aware canvas retry");
        tx.commit().expect("commit source-aware retry");
        result
    };
    assert_eq!(
        fixture.get(&child.job_id).source_ref,
        json!({
            "id": "canvas-round-2",
            "conversation_id": queued.conversation_id,
            "project_id": "default"
        })
    );

    let error = {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        create_retry_job(&mut conn, &queued.job_id, "canvas-retry-request")
            .expect_err("generic replay cannot acknowledge canvas retry")
    };
    assert_eq!(stable_code(&error), "generation_job_source_unsupported");
}

#[test]
fn list_cursor_is_stable_filtered_and_bounded() {
    let fixture = JobFixture::new();
    let first = fixture.enqueue("request-1", "first");
    let second = fixture.enqueue("request-2", "second");
    let third = fixture.enqueue("request-3", "third");

    let first_page = fixture.list(&GenerationJobFilter {
        limit: Some(2),
        ..GenerationJobFilter::default()
    });
    assert_eq!(
        first_page
            .items
            .iter()
            .map(|job| job.id.as_str())
            .collect::<Vec<_>>(),
        [first.id.as_str(), second.id.as_str()]
    );
    let cursor = first_page.next_cursor.expect("next cursor");
    let second_page = fixture.list(&GenerationJobFilter {
        limit: Some(2),
        cursor: Some(cursor),
        ..GenerationJobFilter::default()
    });
    assert_eq!(second_page.items.len(), 1);
    assert_eq!(second_page.items[0].id, third.id);
    assert!(second_page.next_cursor.is_none());

    let by_generation = fixture.list(&GenerationJobFilter {
        generation_id: Some(second.generation_id.clone()),
        ..GenerationJobFilter::default()
    });
    assert_eq!(by_generation.items.len(), 1);
    assert_eq!(by_generation.items[0].id, second.id);

    let by_source = fixture.list(&GenerationJobFilter {
        source_kind: Some("generate".to_string()),
        source_ref_id: Some("third".to_string()),
        ..GenerationJobFilter::default()
    });
    assert_eq!(by_source.items.len(), 1);
    assert_eq!(by_source.items[0].id, third.id);

    let zero_limit_is_bounded = fixture.list(&GenerationJobFilter {
        limit: Some(0),
        ..GenerationJobFilter::default()
    });
    assert_eq!(zero_limit_is_bounded.items.len(), 1);
}

#[test]
fn list_huge_limit_is_capped_and_status_filter_is_applied() {
    let fixture = JobFixture::new();
    let mut conn = fixture.open_connection();
    let tx = conn.transaction().expect("begin batch enqueue");
    let mut conversation_id = None;
    for index in 0..=crate::models::MAX_GENERATION_JOB_PAGE_LIMIT {
        let mut request =
            fixture.prepared(&format!("request-{index}"), &format!("bounded-{index}"));
        request.requested_conversation_id = conversation_id.clone();
        let result = insert_job_in_transaction(&tx, &request).expect("insert batch job");
        conversation_id = Some(result.conversation_id);
    }
    tx.commit().expect("commit batch enqueue");

    let page = fixture.list(&GenerationJobFilter {
        limit: Some(i32::MAX),
        statuses: Some(vec![GenerationJobStatus::Queued]),
        ..GenerationJobFilter::default()
    });
    assert_eq!(
        page.items.len(),
        crate::models::MAX_GENERATION_JOB_PAGE_LIMIT as usize
    );
    assert!(page
        .items
        .iter()
        .all(|job| job.status == GenerationJobStatus::Queued));
    assert!(page.next_cursor.is_some());
}

#[test]
fn malformed_or_wrong_version_cursor_is_rejected_with_stable_sanitized_error() {
    let fixture = JobFixture::new();
    fixture.enqueue("request-1", "cursor");
    for cursor in [
        "not-base64",
        "eyJ2Ijo5OSwicXVldWVkX2F0IjoieCIsInJvd2lkIjoxfQ",
    ] {
        let conn = fixture.database.conn.lock().expect("lock database");
        let error = list_jobs(
            &conn,
            &GenerationJobFilter {
                cursor: Some(cursor.to_string()),
                ..GenerationJobFilter::default()
            },
        )
        .expect_err("invalid cursor must fail");
        assert_eq!(stable_code(&error), "generation_job_corrupt_cursor");
        assert!(!error.to_string().contains(cursor));
    }
}

#[test]
fn noncanonical_rfc3339_job_timestamps_and_cursors_are_rejected() {
    let fixture = JobFixture::new();
    for (index, timestamp) in ["2026-07-10T08:00:00+08:00", "2026-07-10T00:00:00.000Z"]
        .into_iter()
        .enumerate()
    {
        let request =
            fixture.prepared_at(&format!("request-{index}"), "noncanonical-time", timestamp);
        let error = fixture
            .enqueue_prepared(&request)
            .expect_err("queue timestamps must use canonical UTC seconds");
        assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
    }
    let cursor = URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&JobCursor {
            version: 1,
            queued_at: "2026-07-10T08:00:00+08:00".to_string(),
            rowid: 1,
        })
        .expect("serialize noncanonical cursor"),
    );
    let conn = fixture.database.conn.lock().expect("lock database");
    let error = list_jobs(
        &conn,
        &GenerationJobFilter {
            cursor: Some(cursor),
            ..GenerationJobFilter::default()
        },
    )
    .expect_err("cursor timestamps must use canonical UTC seconds");
    assert_eq!(stable_code(&error), "generation_job_corrupt_cursor");
}

#[test]
fn malformed_persisted_status_or_json_returns_classified_error_without_panic() {
    let fixture = JobFixture::new();
    let first = fixture.enqueue("request-1", "corrupt-status");
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            "UPDATE generation_jobs SET status = 'not-a-status' WHERE id = ?1",
            params![first.id],
        )
        .expect("corrupt status");
        let error = get_job(&conn, &first.id).expect_err("corrupt status must fail");
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    }

    let second = fixture.enqueue("request-2", "corrupt-json");
    let conn = fixture.database.conn.lock().expect("lock database");
    conn.execute(
        "UPDATE generation_jobs SET request_json = '{broken' WHERE id = ?1",
        params![second.id],
    )
    .expect("corrupt JSON");
    let error = get_job(&conn, &second.id).expect_err("corrupt JSON must fail");
    assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
}

#[test]
fn fabricated_terminal_state_combinations_are_rejected() {
    for (index, mutation) in [
        "status = 'completed', finished_at = '2026-07-10T00:00:02Z'",
        "status = 'failed', finished_at = '2026-07-10T00:00:02Z', \
         error_code = 'provider_unavailable', \
         error_message = 'The provider is temporarily unavailable', retryable = 1",
        "status = 'cancelled', finished_at = '2026-07-10T00:00:02Z', \
         error_code = 'cancelled_by_user', error_message = 'The operation was cancelled'",
    ]
    .into_iter()
    .enumerate()
    {
        let fixture = JobFixture::new();
        let job = fixture.enqueue(&format!("request-{index}"), "fabricated-terminal");
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            &format!("UPDATE generation_jobs SET {mutation} WHERE id = ?1"),
            params![job.id],
        )
        .expect("fabricate terminal state");
        let error = get_job(&conn, &job.id).expect_err("fabricated state must fail");
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    }
}

#[test]
fn persisted_private_or_wrong_shape_snapshot_is_rejected_before_projection() {
    let fixture = JobFixture::new();
    let wrong_shape = fixture.enqueue("request-1", "wrong-shape");
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            "UPDATE generation_jobs SET request_json = 'null' WHERE id = ?1",
            params![wrong_shape.id],
        )
        .expect("replace request with wrong JSON shape");
        let error = get_job(&conn, &wrong_shape.id).expect_err("wrong shape must fail");
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    }

    let private = fixture.enqueue("request-2", "private-persisted");
    let conn = fixture.database.conn.lock().expect("lock database");
    conn.execute(
        "UPDATE generation_jobs
         SET request_json = '{\"nested\":{\"apiKey\":\"secret-key\"}}',
             endpoint_snapshot = 'https://example.test/images?key=secret-key'
         WHERE id = ?1",
        params![private.id],
    )
    .expect("inject private persisted snapshot");
    let error = get_job(&conn, &private.id).expect_err("private snapshot must fail");
    assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    assert!(!error.to_string().contains("secret-key"));
}

#[test]
fn persisted_sql_type_mismatch_is_classified_as_corrupt_data() {
    let fixture = JobFixture::new();
    let job = fixture.enqueue("request-1", "wrong-sql-type");
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        conn.execute(
            "UPDATE generation_jobs SET status = 42 WHERE id = ?1",
            params![job.id],
        )
        .expect("replace status with integer");
        let error = get_job(&conn, &job.id).expect_err("wrong SQL type must fail");
        assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
        assert!(!error.to_string().contains("generation_jobs"));
    }

    let list_job = fixture.enqueue("request-2", "wrong-list-type");
    let conn = fixture.database.conn.lock().expect("lock database");
    conn.execute(
        "UPDATE generation_jobs SET retryable = 'not-an-integer' WHERE id = ?1",
        params![list_job.id],
    )
    .expect("replace retryable with text");
    let get_error = get_job(&conn, &list_job.id).expect_err("wrong row type must fail get");
    assert_eq!(
        stable_code(&get_error),
        "generation_job_corrupt_persisted_data"
    );
    let list_error = list_jobs(
        &conn,
        &GenerationJobFilter {
            generation_id: Some(list_job.generation_id),
            ..GenerationJobFilter::default()
        },
    )
    .expect_err("wrong row type must fail list");
    assert_eq!(
        stable_code(&list_error),
        "generation_job_corrupt_persisted_data"
    );
}

#[test]
fn malformed_linked_generation_conversation_is_classified_as_corrupt_data() {
    let fixture = JobFixture::new();
    fixture.enqueue("request-1", "bad-conversation-link");
    let conn = fixture.database.conn.lock().expect("lock database");
    conn.execute_batch("PRAGMA foreign_keys=OFF;")
        .expect("disable fixture foreign keys");
    conn.execute(
        "UPDATE generations SET conversation_id = x'3432' WHERE id = (
            SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1'
         )",
        [],
    )
    .expect("corrupt linked conversation type");
    let error = find_enqueue_result_by_client_request_id(&conn, "request-1")
        .expect_err("malformed linked conversation must fail");
    conn.execute_batch("PRAGMA foreign_keys=ON;")
        .expect("restore fixture foreign keys");
    assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
    assert!(!error.to_string().contains("conversation_id"));
}

#[test]
fn enqueue_ack_rejects_valid_but_mismatched_linked_conversation() {
    let fixture = JobFixture::new();
    fixture.enqueue("request-1", "mismatched-conversation");
    let conn = fixture.database.conn.lock().expect("lock database");
    conn.execute(
        "INSERT INTO conversations (id, project_id, title)
         VALUES ('different-conversation', 'default', 'Different')",
        [],
    )
    .expect("insert different conversation");
    conn.execute(
        "UPDATE generations SET conversation_id = 'different-conversation' WHERE id = (
            SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1'
         )",
        [],
    )
    .expect("mismatch linked conversation");
    let error = find_enqueue_result_by_client_request_id(&conn, "request-1")
        .expect_err("acknowledgement identity mismatch must fail");
    assert_eq!(stable_code(&error), "generation_job_corrupt_persisted_data");
}

#[test]
fn terminal_update_derives_fixed_message_without_persisting_provider_text() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("request-1", "unsafe-terminal");
    fixture.claim().expect("claim unsafe terminal fixture");
    let conn = fixture.database.conn.lock().expect("lock database");
    let finished = finish_job(
        &conn,
        &GenerationJobTerminalUpdate {
            job_id: queued.id.clone(),
            expected_status: GenerationJobStatus::Running,
            status: GenerationJobStatus::Failed,
            finished_at: "2026-07-10T00:00:02Z".to_string(),
            error_code: Some("provider_error".to_string()),
            error_message: Some("Incorrect API key provided: sk-secret".to_string()),
            retryable: true,
        },
    )
    .expect("terminal message is derived from the stable code");
    drop(conn);
    assert_eq!(finished.status, GenerationJobStatus::Failed);
    assert_eq!(finished.error_code.as_deref(), Some("provider_error"));
    assert_eq!(
        finished.error_message.as_deref(),
        Some("The generation job failed")
    );
    assert!(!serde_json::to_string(&finished)
        .expect("serialize terminal job")
        .contains("sk-secret"));
    let conn = fixture.database.conn.lock().expect("lock database");
    let generation_error: Option<String> = conn
        .query_row(
            "SELECT error_message FROM generations WHERE id = ?1",
            params![queued.generation_id],
            |row| row.get(0),
        )
        .expect("read sanitized generation error");
    assert_eq!(
        generation_error.as_deref(),
        Some("The generation job failed")
    );
}

#[test]
fn terminal_update_rejects_secret_shaped_error_code_without_mutation() {
    let fixture = JobFixture::new();
    let queued = fixture.enqueue("request-1", "secret-code");
    fixture.claim().expect("claim secret-code fixture");
    let conn = fixture.database.conn.lock().expect("lock database");
    let error = finish_job(
        &conn,
        &GenerationJobTerminalUpdate {
            job_id: queued.id.clone(),
            expected_status: GenerationJobStatus::Running,
            status: GenerationJobStatus::Failed,
            finished_at: "2026-07-10T00:00:02Z".to_string(),
            error_code: Some("ghp_secret_token".to_string()),
            error_message: None,
            retryable: false,
        },
    )
    .expect_err("secret-shaped error code must fail");
    drop(conn);
    assert_eq!(stable_code(&error), "generation_job_invalid_snapshot");
    assert_eq!(fixture.get(&queued.id).status, GenerationJobStatus::Running);
}

#[test]
fn missing_job_and_nonretryable_terminal_job_have_stable_codes() {
    let fixture = JobFixture::new();
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        let error = get_job(&conn, "missing").expect_err("missing job must fail");
        assert_eq!(stable_code(&error), "generation_job_not_found");
    }

    let queued = fixture.enqueue("request-1", "completed");
    fixture.claim().expect("claim completed job");
    {
        let conn = fixture.database.conn.lock().expect("lock database");
        let tx = conn
            .unchecked_transaction()
            .expect("begin completed job transaction");
        prepare_completed_projection(&tx, &queued.generation_id, 1);
        finish_job_in_transaction(
            &tx,
            &GenerationJobTerminalUpdate {
                job_id: queued.id.clone(),
                expected_status: GenerationJobStatus::Running,
                status: GenerationJobStatus::Completed,
                finished_at: "2026-07-10T00:00:02Z".to_string(),
                error_code: None,
                error_message: None,
                retryable: false,
            },
        )
        .expect("complete job");
        tx.commit().expect("commit completed job");
    }
    let error = {
        let mut conn = fixture.database.conn.lock().expect("lock database");
        create_retry_job(&mut conn, &queued.id, "retry-request")
            .expect_err("completed job cannot retry")
    };
    assert_eq!(stable_code(&error), "generation_job_not_retryable");
}

#[test]
fn preflight_lookup_returns_committed_enqueue_identity() {
    let fixture = JobFixture::new();
    let job = fixture.enqueue("request-1", "lookup");
    let result = fixture.get_result("request-1").expect("existing result");
    assert_eq!(result.job_id, job.id);
    assert_eq!(result.generation_id, job.generation_id);
    assert_eq!(result.status, GenerationJobStatus::Queued);
}

#[test]
fn fixture_uses_real_v16_file_database() {
    let fixture = JobFixture::new();
    assert!(Path::new(&fixture.path).is_file());
    let conn = fixture.database.conn.lock().expect("lock database");
    let version_exists: i64 = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = 16)",
            [],
            |row| row.get(0),
        )
        .expect("read schema version");
    assert_eq!(version_exists, 1);
}
