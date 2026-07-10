use super::*;
use crate::db::Database;
use crate::error::AppError;
use crate::models::{GenerationJobFilter, GenerationJobStatus};
use rusqlite::{params, Connection};
use serde_json::json;
use std::path::{Path, PathBuf};

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
        "conversations" | "generations" | "generation_jobs" | "generation_recoveries" | "logs"
    ));
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .expect("count fixture table")
}

fn stable_code(error: &AppError) -> &'static str {
    error.stable_code()
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
    for (index, endpoint) in [
        "https://api.example.test/images?api-version=2026-01-01",
        "https://api.example.test/images?routing=primary&region=west",
    ]
    .into_iter()
    .enumerate()
    {
        let mut request = fixture.prepared(&format!("request-{index}"), "safe-query");
        request.endpoint_snapshot = endpoint.to_string();
        request.prompt = "Paint a sketch of the bearer of light".to_string();
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
    source_ref.source_ref = json!({ "id": "AIza-secret-source" });
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
                    "UPDATE generations SET prompt = 'Bearer abcdefgh1234' WHERE id = ?1",
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
fn claim_and_list_use_stable_same_second_fifo_and_update_generation() {
    let fixture = JobFixture::new();
    let first = fixture.enqueue("request-1", "first");
    let second = fixture.enqueue("request-2", "second");
    assert_eq!(first.queued_at, second.queued_at);

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
    assert_eq!(fixture.generation_status(&first.generation_id), "running");

    let claimed_second = fixture.claim().expect("claim second job");
    assert_eq!(claimed_second.id, second.id);
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
        "INSERT INTO projects (id, name) VALUES ('other-project', 'Other'); UPDATE conversations SET project_id = 'other-project' WHERE id = (SELECT conversation_id FROM generations WHERE id = (SELECT generation_id FROM generation_jobs WHERE client_request_id = 'request-1'))",
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
    tx.execute(
        "DELETE FROM generation_recoveries WHERE generation_id = ?1",
        params![queued.generation_id],
    )
    .expect("clean recovery in outer transaction");
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
    assert!(requested.cancel_requested_at.is_some());
    assert!(requested.finished_at.is_none());
    assert_eq!(fixture.generation_status(&running.generation_id), "running");
    assert_eq!(fixture.count("generation_recoveries"), 1);
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
        finish_job(
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
        .expect("complete job");
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
