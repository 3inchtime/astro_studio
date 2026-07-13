use crate::error::AppError;
use crate::models::LogEntry;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

pub struct Database {
    pub conn: Mutex<Connection>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::ErrorCode;

    struct TestDatabaseDirectory(std::path::PathBuf);

    impl Drop for TestDatabaseDirectory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    struct MigratedTestDatabase {
        database: Database,
        _directory: TestDatabaseDirectory,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct QueuedRecoverySnapshot {
        request_kind: String,
        request_state: String,
        output_format: String,
        response_file: Option<String>,
        expected_response_file: Option<String>,
        response_size: Option<i64>,
        response_sha256: Option<String>,
        created_at: String,
        updated_at: String,
    }

    impl MigratedTestDatabase {
        fn new(prefix: &str) -> Self {
            let db_path = test_db_path(prefix);
            let directory = TestDatabaseDirectory(
                db_path
                    .parent()
                    .expect("test database parent")
                    .to_path_buf(),
            );
            let database = Database::open(&db_path).expect("open test db");
            database.run_migrations().expect("run migrations");

            {
                let conn = database.conn.lock().expect("lock db");
                let foreign_keys = conn
                    .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
                    .expect("query foreign_keys pragma");
                assert_eq!(foreign_keys, 1, "foreign key enforcement must be enabled");
            }

            Self {
                database,
                _directory: directory,
            }
        }
    }

    fn test_db_path(prefix: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("{prefix}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir.join("astro_studio.db")
    }

    fn table_has_column(conn: &Connection, table: &str, column: &str) -> bool {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("prepare table info");
        let has_column = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .expect("query table info")
            .filter_map(|row| row.ok())
            .any(|name| name == column);
        has_column
    }

    fn table_column_definition(
        conn: &Connection,
        table: &str,
        column: &str,
    ) -> Option<(String, bool, Option<String>)> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("prepare table info");
        stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)? != 0,
                row.get::<_, Option<String>>(4)?,
            ))
        })
        .expect("query table info")
        .collect::<rusqlite::Result<Vec<_>>>()
        .expect("read table info")
        .into_iter()
        .find_map(|(name, data_type, not_null, default_value)| {
            (name == column).then_some((data_type, not_null, default_value))
        })
    }

    fn table_exists(conn: &Connection, table: &str) -> bool {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            params![table],
            |row| row.get::<_, i64>(0),
        )
        .expect("query table existence")
            != 0
    }

    fn migration_version_exists(conn: &Connection, version: i32) -> bool {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
            params![version],
            |row| row.get::<_, i64>(0),
        )
        .expect("query migration version")
            != 0
    }

    fn index_exists(conn: &Connection, index: &str) -> bool {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?1)",
            params![index],
            |row| row.get::<_, i64>(0),
        )
        .expect("query index")
            != 0
    }

    fn trigger_exists(conn: &Connection, trigger: &str) -> bool {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'trigger' AND name = ?1)",
            params![trigger],
            |row| row.get::<_, i64>(0),
        )
        .expect("query trigger existence")
            != 0
    }

    fn index_columns(conn: &Connection, index: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA index_info({index})"))
            .expect("prepare index info");
        let mut columns = stmt
            .query_map([], |row| {
                Ok((row.get::<_, i32>(0)?, row.get::<_, String>(2)?))
            })
            .expect("query index info")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("read index info");
        columns.sort_by_key(|(position, _)| *position);
        columns.into_iter().map(|(_, name)| name).collect()
    }

    fn insert_generation_jobs_context(conn: &Connection) {
        conn.execute(
            "INSERT INTO projects (id, name) VALUES (?1, ?2)",
            params!["generation-jobs-project", "Generation Jobs Project"],
        )
        .expect("insert project fixture");
        conn.execute(
            "INSERT INTO conversations (id, title, project_id) VALUES (?1, ?2, ?3)",
            params![
                "generation-jobs-conversation",
                "Generation Jobs Conversation",
                "generation-jobs-project"
            ],
        )
        .expect("insert conversation fixture");
    }

    fn insert_generation_fixture(conn: &Connection, id: &str) {
        conn.execute(
            "INSERT INTO generations (id, prompt, conversation_id) VALUES (?1, ?2, ?3)",
            params![
                id,
                format!("prompt for {id}"),
                "generation-jobs-conversation"
            ],
        )
        .expect("insert generation fixture");
    }

    fn insert_generation_job_fixture(
        conn: &Connection,
        id: &str,
        client_request_id: &str,
        generation_id: &str,
        parent_job_id: Option<&str>,
    ) -> rusqlite::Result<usize> {
        conn.execute(
            "INSERT INTO generation_jobs (
                id,
                client_request_id,
                generation_id,
                parent_job_id,
                source_kind,
                status,
                request_json,
                provider_kind,
                provider_profile_id,
                endpoint_snapshot,
                queued_at
            ) VALUES (?1, ?2, ?3, ?4, 'generate', 'queued', '{}', 'openai', 'default',
                'https://api.example.com/v1/images/generations', '2026-07-10T00:00:00Z')",
            params![id, client_request_id, generation_id, parent_job_id],
        )
    }

    fn assert_constraint_violation(error: rusqlite::Error, expected_message: &str) {
        match error {
            rusqlite::Error::SqliteFailure(code, message) => {
                assert_eq!(code.code, ErrorCode::ConstraintViolation);
                let message = message.expect("constraint violation message");
                assert!(
                    message.contains(expected_message),
                    "expected constraint message containing {expected_message:?}, got {message:?}"
                );
            }
            other => panic!("expected SQLite constraint violation, got {other:?}"),
        }
    }

    fn create_legacy_database_with_recorded_v7_but_missing_conversation_columns(db_path: &Path) {
        let conn = Connection::open(db_path).expect("open legacy test db");
        conn.execute_batch(
            "CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );
            CREATE TABLE projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                archived_at TEXT,
                pinned_at TEXT,
                deleted_at TEXT
            );
            CREATE TABLE conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                project_id TEXT REFERENCES projects(id) ON DELETE SET NULL
            );
            CREATE TABLE generations (
                id TEXT PRIMARY KEY,
                prompt TEXT NOT NULL,
                engine TEXT NOT NULL DEFAULT 'gpt-image-2',
                size TEXT NOT NULL DEFAULT '1024x1024',
                quality TEXT NOT NULL DEFAULT 'auto',
                status TEXT NOT NULL DEFAULT 'pending',
                request_kind TEXT NOT NULL DEFAULT 'generate',
                output_format TEXT NOT NULL DEFAULT 'png',
                conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );",
        )
        .expect("create legacy schema");

        for version in 1..=13 {
            conn.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                params![version, crate::current_timestamp()],
            )
            .expect("insert migration version");
        }
    }

    fn create_recorded_v16_generation_queue_database(db_path: &Path) -> Database {
        let database = Database::open(db_path).expect("open recorded v16 test db");
        {
            let conn = database.conn.lock().expect("lock recorded v16 db");
            conn.execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at TEXT NOT NULL
                );
                CREATE TABLE projects (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    archived_at TEXT,
                    pinned_at TEXT,
                    deleted_at TEXT
                );
                CREATE TABLE conversations (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL DEFAULT '',
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                    archived_at TEXT,
                    pinned_at TEXT,
                    deleted_at TEXT
                );
                CREATE TABLE generations (
                    id TEXT PRIMARY KEY,
                    prompt TEXT NOT NULL,
                    status TEXT NOT NULL,
                    request_kind TEXT NOT NULL DEFAULT 'generate',
                    output_format TEXT NOT NULL DEFAULT 'png',
                    conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
                    created_at TEXT NOT NULL
                );
                CREATE TABLE generation_recoveries (
                    generation_id TEXT PRIMARY KEY REFERENCES generations(id) ON DELETE CASCADE,
                    request_kind TEXT NOT NULL,
                    request_state TEXT NOT NULL,
                    output_format TEXT NOT NULL,
                    response_file TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE TABLE generation_jobs (
                    id TEXT PRIMARY KEY,
                    client_request_id TEXT NOT NULL UNIQUE,
                    generation_id TEXT NOT NULL UNIQUE REFERENCES generations(id) ON DELETE CASCADE,
                    parent_job_id TEXT REFERENCES generation_jobs(id) ON DELETE SET NULL,
                    source_kind TEXT NOT NULL,
                    source_ref_json TEXT NOT NULL DEFAULT '{}',
                    status TEXT NOT NULL,
                    request_json TEXT NOT NULL,
                    provider_kind TEXT NOT NULL,
                    provider_profile_id TEXT NOT NULL,
                    endpoint_snapshot TEXT NOT NULL,
                    chain_attempt INTEGER NOT NULL DEFAULT 1,
                    auto_attempt INTEGER NOT NULL DEFAULT 0,
                    max_auto_attempts INTEGER NOT NULL DEFAULT 2,
                    queued_at TEXT NOT NULL,
                    started_at TEXT,
                    finished_at TEXT,
                    cancel_requested_at TEXT,
                    last_heartbeat_at TEXT,
                    error_code TEXT,
                    error_message TEXT,
                    retryable INTEGER NOT NULL DEFAULT 0
                );
                INSERT INTO projects (id, name, created_at, updated_at)
                    VALUES ('v16-project', 'V16 Project', '2026-07-10T00:00:00Z', '2026-07-10T00:00:00Z');
                INSERT INTO conversations (id, title, project_id, created_at, updated_at)
                    VALUES ('v16-conversation', 'V16 Conversation', 'v16-project',
                        '2026-07-10T00:00:00Z', '2026-07-10T00:00:00Z');
                INSERT INTO generations (id, prompt, status, conversation_id, created_at)
                    VALUES ('v16-running-generation', 'running fixture', 'running',
                        'v16-conversation', '2026-07-10T00:00:00Z'),
                       ('v16-queued-generation', 'queued fixture', 'queued',
                        'v16-conversation', '2026-07-10T00:00:00Z'),
                       ('v16-failed-generation', 'failed fixture', 'failed',
                        'v16-conversation', '2026-07-10T00:00:00Z'),
                       ('v16-running-missing-recovery-generation',
                        'running without recovery fixture', 'running',
                        'v16-conversation', '2026-07-10T00:00:00Z'),
                       ('v16-unknown-generation', 'future status fixture', 'future_status',
                        'v16-conversation', '2026-07-10T00:00:00Z');
                INSERT INTO generation_recoveries (
                    generation_id, request_kind, request_state, output_format, response_file,
                    created_at, updated_at
                ) VALUES (
                    'v16-running-generation', 'generate', 'response_ready', 'png',
                    '/tmp/v16-response.json', '2026-07-10T00:00:00Z', '2026-07-10T00:00:01Z'
                );
                INSERT INTO generation_jobs (
                    id, client_request_id, generation_id, source_kind, status, request_json,
                    provider_kind, provider_profile_id, endpoint_snapshot, queued_at, started_at,
                    last_heartbeat_at
                ) VALUES (
                    'v16-running-job', 'v16-running-request', 'v16-running-generation',
                    'generate', 'running', '{}', 'openai', 'provider-a',
                    'https://api.example.test/v1/images/generations',
                    '2026-07-10T00:00:00Z', '2026-07-10T00:00:01Z',
                    '2026-07-10T00:00:01Z'
                );
                INSERT INTO generation_jobs (
                    id, client_request_id, generation_id, source_kind, status, request_json,
                    provider_kind, provider_profile_id, endpoint_snapshot, queued_at
                ) VALUES (
                    'v16-queued-job', 'v16-queued-request', 'v16-queued-generation',
                    'generate', 'queued', '{}', 'openai', 'provider-a',
                    'https://api.example.test/v1/images/generations',
                    '2026-07-10T00:00:00Z'
                );
                INSERT INTO generation_jobs (
                    id, client_request_id, generation_id, source_kind, status, request_json,
                    provider_kind, provider_profile_id, endpoint_snapshot, queued_at, started_at,
                    finished_at, last_heartbeat_at, error_code, error_message
                ) VALUES (
                    'v16-failed-job', 'v16-failed-request', 'v16-failed-generation',
                    'generate', 'failed', '{}', 'openai', 'provider-a',
                    'https://api.example.test/v1/images/generations',
                    '2026-07-10T00:00:00Z', '2026-07-10T00:00:01Z',
                    '2026-07-10T00:00:02Z', '2026-07-10T00:00:01Z',
                    'provider_unavailable', 'The provider is unavailable'
                );
                INSERT INTO generation_jobs (
                    id, client_request_id, generation_id, source_kind, status, request_json,
                    provider_kind, provider_profile_id, endpoint_snapshot, queued_at, started_at,
                    last_heartbeat_at
                ) VALUES (
                    'v16-running-missing-recovery-job',
                    'v16-running-missing-recovery-request',
                    'v16-running-missing-recovery-generation', 'generate', 'running', '{}',
                    'openai', 'provider-a',
                    'https://api.example.test/v1/images/generations',
                    '2026-07-10T00:00:00Z', '2026-07-10T00:00:01Z',
                    '2026-07-10T00:00:01Z'
                );
                INSERT INTO generation_jobs (
                    id, client_request_id, generation_id, source_kind, status, request_json,
                    provider_kind, provider_profile_id, endpoint_snapshot, queued_at
                ) VALUES (
                    'v16-unknown-job', 'v16-unknown-request', 'v16-unknown-generation',
                    'generate', 'future_status', '{}', 'openai', 'provider-a',
                    'https://api.example.test/v1/images/generations',
                    '2026-07-10T00:00:00Z'
                );",
            )
            .expect("create recorded v16 queue schema");
            for version in 1..=16 {
                conn.execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                    params![version, "2026-07-10T00:00:00Z"],
                )
                .expect("record v16 migration history");
            }
        }
        database
    }

    #[test]
    fn fresh_database_migrations_create_required_project_and_conversation_columns() {
        let db_path = test_db_path("astro-studio-fresh-migration-test");
        let database = Database::open(&db_path).expect("open test db");

        database.run_migrations().expect("run migrations");

        {
            let conn = database.conn.lock().expect("lock db");
            assert!(table_has_column(&conn, "projects", "deleted_at"));
            assert!(table_has_column(&conn, "conversations", "archived_at"));
            assert!(table_has_column(&conn, "conversations", "pinned_at"));
            assert!(table_has_column(&conn, "conversations", "deleted_at"));
            assert!(migration_version_exists(&conn, 7));
        }

        std::fs::remove_dir_all(db_path.parent().expect("db parent")).ok();
    }

    #[test]
    fn migrations_repair_legacy_recorded_v7_missing_conversation_columns() {
        let db_path = test_db_path("astro-studio-legacy-v7-repair-test");
        create_legacy_database_with_recorded_v7_but_missing_conversation_columns(&db_path);
        let database = Database::open(&db_path).expect("open test db");

        database.run_migrations().expect("run migrations");

        {
            let conn = database.conn.lock().expect("lock db");
            assert!(table_has_column(&conn, "projects", "deleted_at"));
            assert!(table_has_column(&conn, "conversations", "archived_at"));
            assert!(table_has_column(&conn, "conversations", "pinned_at"));
            assert!(table_has_column(&conn, "conversations", "deleted_at"));
            assert!(table_exists(&conn, "generation_recoveries"));
            for column in ["expected_response_file", "response_size", "response_sha256"] {
                assert!(table_has_column(&conn, "generation_recoveries", column));
            }
            assert!(migration_version_exists(&conn, 17));
        }

        std::fs::remove_dir_all(db_path.parent().expect("db parent")).ok();
    }

    #[test]
    fn fresh_database_migrations_create_canvas_documents_table() {
        let db_path = test_db_path("astro-studio-canvas-migration-test");
        let database = Database::open(&db_path).expect("open test db");

        database.run_migrations().expect("run migrations");

        {
            let conn = database.conn.lock().expect("lock db");
            assert!(table_has_column(&conn, "canvas_documents", "project_id"));
            assert!(table_has_column(&conn, "canvas_documents", "document_path"));
            assert!(table_has_column(&conn, "canvas_documents", "preview_path"));
            assert!(table_has_column(&conn, "canvas_documents", "deleted_at"));
            assert!(migration_version_exists(&conn, 14));
        }

        std::fs::remove_dir_all(db_path.parent().expect("db parent")).ok();
    }

    #[test]
    fn fresh_database_migrations_create_prompt_agent_tables() {
        let db_path = test_db_path("astro-studio-prompt-agent-migration-test");
        let database = Database::open(&db_path).expect("open test db");

        database.run_migrations().expect("run migrations");

        {
            let conn = database.conn.lock().expect("lock db");
            assert!(table_has_column(
                &conn,
                "prompt_agent_sessions",
                "conversation_id"
            ));
            assert!(table_has_column(
                &conn,
                "prompt_agent_sessions",
                "suggested_params"
            ));
            assert!(table_has_column(
                &conn,
                "prompt_agent_messages",
                "session_id"
            ));
            assert!(table_has_column(
                &conn,
                "prompt_agent_messages",
                "ready_to_generate"
            ));
            assert!(migration_version_exists(&conn, 15));
        }

        std::fs::remove_dir_all(db_path.parent().expect("db parent")).ok();
    }

    #[test]
    fn fresh_database_migrations_create_generation_jobs_table() {
        let fixture = MigratedTestDatabase::new("astro-studio-generation-jobs-migration-test");

        {
            let conn = fixture.database.conn.lock().expect("lock db");
            for column in [
                "id",
                "client_request_id",
                "generation_id",
                "parent_job_id",
                "source_kind",
                "source_ref_json",
                "status",
                "request_json",
                "provider_kind",
                "provider_profile_id",
                "endpoint_snapshot",
                "chain_attempt",
                "auto_attempt",
                "max_auto_attempts",
                "queued_at",
                "started_at",
                "finished_at",
                "cancel_requested_at",
                "last_heartbeat_at",
                "error_code",
                "error_message",
                "retryable",
            ] {
                assert!(
                    table_has_column(&conn, "generation_jobs", column),
                    "missing generation_jobs.{column}"
                );
            }
            assert!(index_exists(&conn, "idx_generation_jobs_status_queued"));
            assert!(index_exists(&conn, "idx_generation_jobs_parent"));
            assert!(index_exists(&conn, "idx_generation_jobs_source"));
            assert!(migration_version_exists(&conn, 16));
        }
    }

    #[test]
    fn generation_jobs_reject_duplicate_client_request_id() {
        let fixture =
            MigratedTestDatabase::new("astro-studio-generation-jobs-client-request-unique-test");
        let conn = fixture.database.conn.lock().expect("lock db");
        insert_generation_jobs_context(&conn);
        insert_generation_fixture(&conn, "generation-1");
        insert_generation_fixture(&conn, "generation-2");
        insert_generation_job_fixture(&conn, "job-1", "request-1", "generation-1", None)
            .expect("insert first job");

        let error =
            insert_generation_job_fixture(&conn, "job-2", "request-1", "generation-2", None)
                .expect_err("duplicate client_request_id must fail");
        assert_constraint_violation(error, "generation_jobs.client_request_id");
    }

    #[test]
    fn generation_jobs_reject_duplicate_generation_id() {
        let fixture =
            MigratedTestDatabase::new("astro-studio-generation-jobs-generation-unique-test");
        let conn = fixture.database.conn.lock().expect("lock db");
        insert_generation_jobs_context(&conn);
        insert_generation_fixture(&conn, "generation-1");
        insert_generation_job_fixture(&conn, "job-1", "request-1", "generation-1", None)
            .expect("insert first job");

        let error =
            insert_generation_job_fixture(&conn, "job-2", "request-2", "generation-1", None)
                .expect_err("duplicate generation_id must fail");
        assert_constraint_violation(error, "generation_jobs.generation_id");
    }

    #[test]
    fn generation_jobs_reject_nonexistent_generation_id() {
        let fixture =
            MigratedTestDatabase::new("astro-studio-generation-jobs-generation-foreign-key-test");
        let conn = fixture.database.conn.lock().expect("lock db");
        insert_generation_jobs_context(&conn);

        let error =
            insert_generation_job_fixture(&conn, "job-1", "request-1", "missing-generation", None)
                .expect_err("nonexistent generation_id must fail");
        assert_constraint_violation(error, "FOREIGN KEY constraint failed");
    }

    #[test]
    fn generation_jobs_reject_nonexistent_parent_job_id() {
        let fixture =
            MigratedTestDatabase::new("astro-studio-generation-jobs-parent-foreign-key-test");
        let conn = fixture.database.conn.lock().expect("lock db");
        insert_generation_jobs_context(&conn);
        insert_generation_fixture(&conn, "generation-1");

        let error = insert_generation_job_fixture(
            &conn,
            "job-1",
            "request-1",
            "generation-1",
            Some("missing-parent"),
        )
        .expect_err("nonexistent parent_job_id must fail");
        assert_constraint_violation(error, "FOREIGN KEY constraint failed");
    }

    #[test]
    fn deleting_parent_generation_job_clears_child_parent_id() {
        let fixture =
            MigratedTestDatabase::new("astro-studio-generation-jobs-parent-set-null-test");
        let conn = fixture.database.conn.lock().expect("lock db");
        insert_generation_jobs_context(&conn);
        insert_generation_fixture(&conn, "generation-1");
        insert_generation_fixture(&conn, "generation-2");
        insert_generation_job_fixture(&conn, "job-1", "request-1", "generation-1", None)
            .expect("insert parent job");
        insert_generation_job_fixture(&conn, "job-2", "request-2", "generation-2", Some("job-1"))
            .expect("insert child job");

        conn.execute(
            "DELETE FROM generation_jobs WHERE id = ?1",
            params!["job-1"],
        )
        .expect("delete parent job");

        let parent_job_id = conn
            .query_row(
                "SELECT parent_job_id FROM generation_jobs WHERE id = ?1",
                params!["job-2"],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("query child job");
        assert_eq!(parent_job_id, None);
    }

    #[test]
    fn deleting_generation_cascades_to_generation_job() {
        let fixture =
            MigratedTestDatabase::new("astro-studio-generation-jobs-generation-cascade-test");
        let conn = fixture.database.conn.lock().expect("lock db");
        insert_generation_jobs_context(&conn);
        insert_generation_fixture(&conn, "generation-1");
        insert_generation_job_fixture(&conn, "job-1", "request-1", "generation-1", None)
            .expect("insert job");

        conn.execute(
            "DELETE FROM generations WHERE id = ?1",
            params!["generation-1"],
        )
        .expect("delete generation");

        let job_count = conn
            .query_row(
                "SELECT COUNT(*) FROM generation_jobs WHERE id = ?1",
                params!["job-1"],
                |row| row.get::<_, i64>(0),
            )
            .expect("count generation jobs");
        assert_eq!(job_count, 0);
    }

    #[test]
    fn generation_job_indexes_use_expected_columns_in_order() {
        let fixture = MigratedTestDatabase::new("astro-studio-generation-jobs-index-columns-test");
        let conn = fixture.database.conn.lock().expect("lock db");

        assert_eq!(
            index_columns(&conn, "idx_generation_jobs_status_queued"),
            ["status", "queued_at"]
        );
        assert_eq!(
            index_columns(&conn, "idx_generation_jobs_parent"),
            ["parent_job_id"]
        );
        assert_eq!(
            index_columns(&conn, "idx_generation_jobs_source"),
            ["source_kind"]
        );
    }

    #[test]
    fn generation_worker_v17_fresh_database_has_queue_recovery_and_singleton_lease_schema() {
        let fixture = MigratedTestDatabase::new("astro-studio-generation-worker-v17-fresh-test");
        let conn = fixture.database.conn.lock().expect("lock db");

        assert_eq!(
            table_column_definition(&conn, "generation_jobs", "stage"),
            Some((
                "TEXT".to_string(),
                true,
                Some("'migration_unknown'".to_string())
            ))
        );
        for column in ["expected_response_file", "response_size", "response_sha256"] {
            assert!(
                table_has_column(&conn, "generation_recoveries", column),
                "missing generation_recoveries.{column}"
            );
        }
        assert_eq!(
            table_column_definition(&conn, "generation_recoveries", "expected_response_file"),
            Some(("TEXT".to_string(), false, None))
        );
        assert_eq!(
            table_column_definition(&conn, "generation_recoveries", "response_size"),
            Some(("INTEGER".to_string(), false, None))
        );
        assert_eq!(
            table_column_definition(&conn, "generation_recoveries", "response_sha256"),
            Some(("TEXT".to_string(), false, None))
        );
        assert!(table_exists(&conn, "generation_worker_lease"));
        assert_eq!(
            table_column_definition(&conn, "generation_worker_lease", "owner_id"),
            Some(("TEXT".to_string(), false, None))
        );
        assert_eq!(
            table_column_definition(&conn, "generation_worker_lease", "fencing_epoch"),
            Some(("INTEGER".to_string(), true, Some("0".to_string())))
        );
        for column in ["acquired_at", "heartbeat_at", "expires_at"] {
            assert_eq!(
                table_column_definition(&conn, "generation_worker_lease", column),
                Some(("INTEGER".to_string(), false, None)),
                "unexpected generation_worker_lease.{column} definition"
            );
        }
        let lease: (
            i64,
            Option<String>,
            i64,
            Option<i64>,
            Option<i64>,
            Option<i64>,
        ) = conn
            .query_row(
                "SELECT id, owner_id, fencing_epoch, acquired_at, heartbeat_at, expires_at
                 FROM generation_worker_lease",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("read singleton worker lease");
        assert_eq!(lease, (1, None, 0, None, None, None));
        assert!(migration_version_exists(&conn, 17));

        insert_generation_jobs_context(&conn);
        insert_generation_fixture(&conn, "v17-default-stage-generation");
        insert_generation_job_fixture(
            &conn,
            "v17-default-stage-job",
            "v17-default-stage-request",
            "v17-default-stage-generation",
            None,
        )
        .expect("insert post-migration job without explicit stage");
        assert_eq!(
            conn.query_row(
                "SELECT stage FROM generation_jobs WHERE id = 'v17-default-stage-job'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("read fail-closed default stage"),
            "migration_unknown"
        );
    }

    #[test]
    fn generation_worker_v17_upgrades_recorded_v16_rows_with_safe_backfill() {
        let db_path = test_db_path("astro-studio-generation-worker-v17-upgrade-test");
        let directory = TestDatabaseDirectory(
            db_path
                .parent()
                .expect("test database parent")
                .to_path_buf(),
        );
        let database = create_recorded_v16_generation_queue_database(&db_path);

        database.run_migrations().expect("upgrade recorded v16 db");

        let conn = database.conn.lock().expect("lock upgraded db");
        let stages = conn
            .prepare("SELECT id, stage FROM generation_jobs ORDER BY id")
            .expect("prepare upgraded stages")
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("query upgraded stages")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("read upgraded stages");
        assert_eq!(
            stages,
            [
                ("v16-failed-job".to_string(), "terminal".to_string()),
                ("v16-queued-job".to_string(), "queued".to_string()),
                (
                    "v16-running-job".to_string(),
                    "startup_reconciliation".to_string()
                ),
                (
                    "v16-running-missing-recovery-job".to_string(),
                    "startup_reconciliation".to_string()
                ),
                (
                    "v16-unknown-job".to_string(),
                    "migration_unknown".to_string()
                ),
            ]
        );
        let recovery: (String, Option<String>, Option<i64>, Option<String>) = conn
            .query_row(
                "SELECT response_file, expected_response_file, response_size, response_sha256
                 FROM generation_recoveries WHERE generation_id = 'v16-running-generation'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read upgraded recovery");
        assert_eq!(
            recovery,
            ("/tmp/v16-response.json".to_string(), None, None, None)
        );
        let queued_recovery = conn
            .query_row(
                "SELECT request_kind, request_state, output_format, response_file,
                        expected_response_file, response_size, response_sha256,
                        created_at, updated_at
                 FROM generation_recoveries
                 WHERE generation_id = 'v16-queued-generation'",
                [],
                |row| {
                    Ok(QueuedRecoverySnapshot {
                        request_kind: row.get(0)?,
                        request_state: row.get(1)?,
                        output_format: row.get(2)?,
                        response_file: row.get(3)?,
                        expected_response_file: row.get(4)?,
                        response_size: row.get(5)?,
                        response_sha256: row.get(6)?,
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                },
            )
            .expect("read synthesized queued recovery");
        assert_eq!(
            queued_recovery,
            QueuedRecoverySnapshot {
                request_kind: "generate".to_string(),
                request_state: "requesting".to_string(),
                output_format: "png".to_string(),
                response_file: None,
                expected_response_file: None,
                response_size: None,
                response_sha256: None,
                created_at: "2026-07-10T00:00:00Z".to_string(),
                updated_at: "2026-07-10T00:00:00Z".to_string(),
            }
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM generation_recoveries
                 WHERE generation_id = 'v16-running-missing-recovery-generation'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("count unsafe running recovery repair"),
            0,
            "migration must not synthesize replayable recovery for an unknown running outcome"
        );
        assert!(migration_version_exists(&conn, 17));
        drop(conn);
        drop(database);
        drop(directory);
    }

    #[test]
    fn generation_worker_v17_partial_rerun_preserves_explicit_stage() {
        let db_path = test_db_path("astro-studio-generation-worker-v17-partial-test");
        let directory = TestDatabaseDirectory(
            db_path
                .parent()
                .expect("test database parent")
                .to_path_buf(),
        );
        let database = create_recorded_v16_generation_queue_database(&db_path);
        {
            let conn = database.conn.lock().expect("lock partial v17 db");
            conn.execute(
                "ALTER TABLE generation_jobs
                 ADD COLUMN stage TEXT NOT NULL DEFAULT 'migration_unknown'",
                [],
            )
            .expect("partially add worker stage");
            conn.execute(
                "UPDATE generation_jobs SET stage = 'provider_request'
                 WHERE id = 'v16-running-job'",
                [],
            )
            .expect("persist explicit stage before migration rerun");
        }

        database
            .run_migrations()
            .expect("resume partially applied v17 migration");

        let conn = database.conn.lock().expect("lock resumed v17 db");
        assert_eq!(
            conn.query_row(
                "SELECT stage FROM generation_jobs WHERE id = 'v16-running-job'",
                [],
                |row| row.get::<_, String>(0),
            )
            .expect("read preserved explicit stage"),
            "provider_request"
        );
        drop(conn);
        drop(database);
        drop(directory);
    }

    #[test]
    fn generation_worker_v17_refuses_success_when_migration_record_is_suppressed() {
        let db_path = test_db_path("astro-studio-generation-worker-v17-record-test");
        let directory = TestDatabaseDirectory(
            db_path
                .parent()
                .expect("test database parent")
                .to_path_buf(),
        );
        let database = create_recorded_v16_generation_queue_database(&db_path);
        {
            let conn = database.conn.lock().expect("lock migration record db");
            conn.execute_batch(
                "CREATE TRIGGER suppress_v17_migration_record
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version = 17
                 BEGIN
                     SELECT RAISE(IGNORE);
                 END;",
            )
            .expect("install migration record suppression trigger");
        }

        let error = database
            .run_migrations()
            .expect_err("migration must fail when its version row is not persisted");
        assert!(
            format!("{error:?}").contains("record migration 17"),
            "unexpected migration record error: {error:?}"
        );
        {
            let conn = database.conn.lock().expect("lock failed migration db");
            assert!(!migration_version_exists(&conn, 17));
            assert!(
                !table_has_column(&conn, "generation_jobs", "stage"),
                "failed migration recording must roll back every schema side effect"
            );
            conn.execute("DROP TRIGGER suppress_v17_migration_record", [])
                .expect("remove migration record suppression trigger");
        }

        database
            .run_migrations()
            .expect("migration remains resumable after record failure");
        let conn = database.conn.lock().expect("lock resumed migration db");
        assert!(migration_version_exists(&conn, 17));
        drop(conn);
        drop(database);
        drop(directory);
    }

    #[test]
    fn generation_worker_v17_repeated_migration_is_idempotent() {
        let fixture = MigratedTestDatabase::new("astro-studio-generation-worker-v17-repeat-test");

        fixture
            .database
            .run_migrations()
            .expect("repeat v17 migrations");

        let conn = fixture.database.conn.lock().expect("lock db");
        let version_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = 17",
                [],
                |row| row.get(0),
            )
            .expect("count v17 migration record");
        let lease_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM generation_worker_lease", [], |row| {
                row.get(0)
            })
            .expect("count singleton lease rows");
        assert_eq!(version_count, 1);
        assert_eq!(lease_count, 1);
        assert_eq!(
            table_column_definition(&conn, "generation_jobs", "stage"),
            Some((
                "TEXT".to_string(),
                true,
                Some("'migration_unknown'".to_string())
            ))
        );
    }

    #[test]
    fn generation_worker_v17_lease_enforces_singleton_key_and_nonnegative_epoch() {
        let fixture = MigratedTestDatabase::new("astro-studio-generation-worker-v17-lease-test");
        let conn = fixture.database.conn.lock().expect("lock db");
        for trigger in V18_LEASE_TRIGGER_NAMES {
            conn.execute(&format!("DROP TRIGGER {trigger}"), [])
                .expect("isolate v17 lease constraints from v18 transition guards");
        }

        let wrong_key = conn
            .execute(
                "INSERT INTO generation_worker_lease (id, fencing_epoch) VALUES (2, 0)",
                [],
            )
            .expect_err("lease table must reject another singleton key");
        assert_constraint_violation(wrong_key, "CHECK constraint failed");

        let negative_epoch = conn
            .execute(
                "UPDATE generation_worker_lease SET fencing_epoch = -1 WHERE id = 1",
                [],
            )
            .expect_err("lease table must reject a negative fencing epoch");
        assert_constraint_violation(negative_epoch, "generation worker fencing epoch decreased");

        for (sql, expectation) in [
            (
                "UPDATE generation_worker_lease SET owner_id = 'worker-a' WHERE id = 1",
                "lease table must reject an owner without timestamps",
            ),
            (
                "UPDATE generation_worker_lease SET acquired_at = 1000 WHERE id = 1",
                "lease table must reject timestamps without an owner",
            ),
            (
                "UPDATE generation_worker_lease
                    SET owner_id = '', acquired_at = 1000, heartbeat_at = 1000,
                        expires_at = 2000
                  WHERE id = 1",
                "lease table must reject an empty owner",
            ),
            (
                "UPDATE generation_worker_lease
                    SET owner_id = 'worker-a', acquired_at = 1001, heartbeat_at = 1000,
                        expires_at = 2000
                  WHERE id = 1",
                "lease table must reject heartbeat before acquisition",
            ),
            (
                "UPDATE generation_worker_lease
                    SET owner_id = 'worker-a', acquired_at = 1000, heartbeat_at = 2000,
                        expires_at = 2000
                  WHERE id = 1",
                "lease table must require heartbeat before expiry",
            ),
            (
                "UPDATE generation_worker_lease
                    SET owner_id = 'worker-a', acquired_at = 1000, heartbeat_at = 1500,
                        expires_at = 2000
                  WHERE id = 1",
                "an owned lease must use a positive fencing epoch",
            ),
        ] {
            let error = conn.execute(sql, []).expect_err(expectation);
            assert_constraint_violation(error, "CHECK constraint failed");
        }

        conn.execute(
            "UPDATE generation_worker_lease
                SET owner_id = 'worker-a', fencing_epoch = 1, acquired_at = 1000,
                    heartbeat_at = 1500, expires_at = 2000
              WHERE id = 1",
            [],
        )
        .expect("lease table must accept a complete ordered lease");
        conn.execute(
            "UPDATE generation_worker_lease
                SET owner_id = NULL, acquired_at = NULL, heartbeat_at = NULL, expires_at = NULL
              WHERE id = 1",
            [],
        )
        .expect("lease table must accept a fully released lease");
        let decreased_epoch = conn
            .execute(
                "UPDATE generation_worker_lease SET fencing_epoch = 0 WHERE id = 1",
                [],
            )
            .expect_err("lease fencing epoch must never decrease");
        assert_constraint_violation(decreased_epoch, "generation worker fencing epoch decreased");

        let moved_key = conn
            .execute("UPDATE generation_worker_lease SET id = 2 WHERE id = 1", [])
            .expect_err("singleton lease key must remain fixed");
        assert_constraint_violation(moved_key, "CHECK constraint failed");
        assert_eq!(
            conn.query_row(
                "SELECT id, owner_id, fencing_epoch FROM generation_worker_lease",
                [],
                |row| Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?
                )),
            )
            .expect("read singleton after rejected writes"),
            (1, None, 1)
        );
    }

    #[test]
    fn generation_worker_v17_concurrent_upgrade_records_migration_and_lease_once() {
        let db_path = test_db_path("astro-studio-generation-worker-v17-concurrent-test");
        let directory = TestDatabaseDirectory(
            db_path
                .parent()
                .expect("test database parent")
                .to_path_buf(),
        );
        let setup = create_recorded_v16_generation_queue_database(&db_path);
        drop(setup);

        let databases = [
            Database::open(&db_path).expect("open first migration connection"),
            Database::open(&db_path).expect("open second migration connection"),
        ];
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(databases.len() + 1));
        let handles = databases.map(|database| {
            let barrier = std::sync::Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                database
                    .run_migrations()
                    .map_err(|error| format!("{error:?}"))
            })
        });

        barrier.wait();
        for handle in handles {
            handle
                .join()
                .expect("concurrent migration thread panicked")
                .expect("concurrent migration must succeed");
        }

        let database = Database::open(&db_path).expect("reopen concurrently migrated db");
        let conn = database.conn.lock().expect("lock concurrently migrated db");
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = 17",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("count concurrent v17 migration records"),
            1
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM generation_worker_lease", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("count concurrent singleton lease rows"),
            1
        );
        drop(conn);
        drop(database);
        drop(directory);
    }

    const V18_LEASE_TRIGGER_NAMES: [&str; 3] = [
        "prevent_generation_worker_lease_delete",
        "prevent_generation_worker_lease_insert",
        "enforce_generation_worker_lease_transition",
    ];

    fn remove_v18_lease_migration_state(database: &Database) {
        let conn = database.conn.lock().expect("lock v18 reset db");
        for trigger in V18_LEASE_TRIGGER_NAMES {
            conn.execute(&format!("DROP TRIGGER IF EXISTS {trigger}"), [])
                .expect("drop v18 lease trigger");
        }
        conn.execute("DELETE FROM schema_migrations WHERE version = 18", [])
            .expect("remove v18 migration record");
    }

    fn lease_row(
        conn: &Connection,
    ) -> (
        i64,
        Option<String>,
        i64,
        Option<i64>,
        Option<i64>,
        Option<i64>,
    ) {
        conn.query_row(
            "SELECT id, owner_id, fencing_epoch, acquired_at, heartbeat_at, expires_at
               FROM generation_worker_lease",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                ))
            },
        )
        .expect("read generation worker lease row")
    }

    #[test]
    fn generation_worker_v18_installs_sealed_transition_guards() {
        let fixture = MigratedTestDatabase::new("astro-studio-generation-worker-v18-fresh-test");
        let conn = fixture.database.conn.lock().expect("lock v18 db");

        assert!(migration_version_exists(&conn, 18));
        for trigger in V18_LEASE_TRIGGER_NAMES {
            assert!(trigger_exists(&conn, trigger), "missing trigger {trigger}");
        }
        assert!(trigger_exists(
            &conn,
            "prevent_generation_worker_lease_epoch_decrease"
        ));
        assert_eq!(lease_row(&conn), (1, None, 0, None, None, None));
    }

    #[test]
    fn generation_worker_v18_allows_only_renew_release_and_next_epoch_takeover() {
        let fixture = MigratedTestDatabase::new("astro-studio-generation-worker-v18-legal-test");
        let conn = fixture.database.conn.lock().expect("lock v18 db");

        conn.execute(
            "UPDATE generation_worker_lease
                SET owner_id = 'worker-a', fencing_epoch = 1,
                    acquired_at = 1000, heartbeat_at = 1000, expires_at = 1100
              WHERE id = 1",
            [],
        )
        .expect("acquire released lease at exact next epoch");
        conn.execute(
            "UPDATE generation_worker_lease
                SET heartbeat_at = 1050, expires_at = 1150
              WHERE id = 1",
            [],
        )
        .expect("renew active lease monotonically");
        conn.execute(
            "UPDATE generation_worker_lease
                SET owner_id = NULL, acquired_at = NULL, heartbeat_at = NULL, expires_at = NULL
              WHERE id = 1",
            [],
        )
        .expect("release while retaining epoch");
        conn.execute(
            "UPDATE generation_worker_lease
                SET owner_id = 'worker-b', fencing_epoch = 2,
                    acquired_at = 1150, heartbeat_at = 1150, expires_at = 1250
              WHERE id = 1",
            [],
        )
        .expect("acquire released lease at next epoch");
        conn.execute(
            "UPDATE generation_worker_lease
                SET owner_id = 'worker-c', fencing_epoch = 3,
                    acquired_at = 1250, heartbeat_at = 1250, expires_at = 1350
              WHERE id = 1",
            [],
        )
        .expect("take over exactly at expiry with next epoch");

        assert_eq!(
            lease_row(&conn),
            (
                1,
                Some("worker-c".to_string()),
                3,
                Some(1250),
                Some(1250),
                Some(1350)
            )
        );
    }

    #[test]
    fn generation_worker_v18_rejects_adversarial_lease_writes() {
        let fixture =
            MigratedTestDatabase::new("astro-studio-generation-worker-v18-adversarial-test");
        let conn = fixture.database.conn.lock().expect("lock v18 db");
        conn.execute(
            "UPDATE generation_worker_lease
                SET owner_id = 'worker-a', fencing_epoch = 1,
                    acquired_at = 1000, heartbeat_at = 1000, expires_at = 1100
              WHERE id = 1",
            [],
        )
        .expect("seed active lease");
        let expected = lease_row(&conn);

        for (sql, label) in [
            (
                "UPDATE generation_worker_lease SET owner_id = 'worker-b' WHERE id = 1",
                "same-epoch owner swap",
            ),
            (
                "UPDATE generation_worker_lease
                    SET owner_id = 'worker-b', fencing_epoch = 2,
                        acquired_at = 1099, heartbeat_at = 1099, expires_at = 1199
                  WHERE id = 1",
                "pre-expiry takeover",
            ),
            (
                "UPDATE generation_worker_lease SET heartbeat_at = 999, expires_at = 1200 WHERE id = 1",
                "heartbeat rollback",
            ),
            (
                "UPDATE generation_worker_lease SET heartbeat_at = 1050, expires_at = 1099 WHERE id = 1",
                "expiry shortening",
            ),
            (
                "UPDATE generation_worker_lease SET heartbeat_at = 1100, expires_at = 1200 WHERE id = 1",
                "renew at old expiry",
            ),
            (
                "UPDATE generation_worker_lease SET fencing_epoch = 2 WHERE id = 1",
                "partial epoch increment",
            ),
        ] {
            let error = conn.execute(sql, []).expect_err(label);
            assert_constraint_violation(error, "invalid generation worker lease transition");
            assert_eq!(lease_row(&conn), expected, "{label} changed the lease row");
        }

        let delete = conn
            .execute("DELETE FROM generation_worker_lease WHERE id = 1", [])
            .expect_err("singleton deletion must be sealed");
        assert_constraint_violation(delete, "generation worker lease row cannot be deleted");
        let replace = conn
            .execute(
                "INSERT OR REPLACE INTO generation_worker_lease (
                    id, owner_id, fencing_epoch, acquired_at, heartbeat_at, expires_at
                 ) VALUES (1, NULL, 0, NULL, NULL, NULL)",
                [],
            )
            .expect_err("replace must not bypass epoch fencing");
        assert_constraint_violation(replace, "generation worker lease row is sealed");
        conn.execute_batch("PRAGMA recursive_triggers=ON;")
            .expect("enable recursive triggers");
        let recursive_replace = conn
            .execute(
                "INSERT OR REPLACE INTO generation_worker_lease (
                    id, owner_id, fencing_epoch, acquired_at, heartbeat_at, expires_at
                 ) VALUES (1, NULL, 0, NULL, NULL, NULL)",
                [],
            )
            .expect_err("replace must also fail with recursive triggers enabled");
        assert_constraint_violation(recursive_replace, "generation worker lease row is sealed");
        assert_eq!(lease_row(&conn), expected);
    }

    #[test]
    fn generation_worker_v18_preserves_recorded_v17_active_lease_exactly() {
        let fixture =
            MigratedTestDatabase::new("astro-studio-generation-worker-v18-active-upgrade-test");
        remove_v18_lease_migration_state(&fixture.database);
        let expected = (
            1,
            Some("worker-before-upgrade".to_string()),
            7,
            Some(10_000),
            Some(10_050),
            Some(10_100),
        );
        {
            let conn = fixture.database.conn.lock().expect("lock active v17 db");
            conn.execute(
                "UPDATE generation_worker_lease
                    SET owner_id = 'worker-before-upgrade', fencing_epoch = 7,
                        acquired_at = 10000, heartbeat_at = 10050, expires_at = 10100
                  WHERE id = 1",
                [],
            )
            .expect("seed recorded-v17 active lease");
            assert_eq!(lease_row(&conn), expected);
        }

        fixture
            .database
            .run_migrations()
            .expect("upgrade active recorded-v17 lease");
        let conn = fixture
            .database
            .conn
            .lock()
            .expect("lock upgraded active db");
        assert!(migration_version_exists(&conn, 18));
        assert_eq!(lease_row(&conn), expected);
    }

    #[test]
    fn generation_worker_v18_allows_final_epoch_but_never_wraps_it() {
        let fixture =
            MigratedTestDatabase::new("astro-studio-generation-worker-v18-max-epoch-test");
        remove_v18_lease_migration_state(&fixture.database);
        {
            let conn = fixture.database.conn.lock().expect("lock max epoch v17 db");
            conn.execute(
                "UPDATE generation_worker_lease SET fencing_epoch = ?1 WHERE id = 1",
                params![i64::MAX - 1],
            )
            .expect("seed penultimate recorded-v17 epoch");
        }
        fixture
            .database
            .run_migrations()
            .expect("upgrade penultimate epoch row to v18");

        let conn = fixture.database.conn.lock().expect("lock max epoch v18 db");
        conn.execute(
            "UPDATE generation_worker_lease
                SET owner_id = 'worker-max', fencing_epoch = ?1,
                    acquired_at = 1000, heartbeat_at = 1000, expires_at = 1100
              WHERE id = 1",
            params![i64::MAX],
        )
        .expect("acquire the final representable epoch");
        conn.execute(
            "UPDATE generation_worker_lease
                SET owner_id = NULL, acquired_at = NULL, heartbeat_at = NULL, expires_at = NULL
              WHERE id = 1",
            [],
        )
        .expect("release final epoch");
        let overflow = conn
            .execute(
                "UPDATE generation_worker_lease
                    SET owner_id = 'worker-overflow', fencing_epoch = CAST(fencing_epoch AS REAL) + 1,
                        acquired_at = 1200, heartbeat_at = 1200, expires_at = 1300
                  WHERE id = 1",
                [],
            )
            .expect_err("no transition may advance past the final integer epoch");
        assert_constraint_violation(overflow, "invalid generation worker lease transition");
        assert_eq!(lease_row(&conn), (1, None, i64::MAX, None, None, None));
    }

    #[test]
    fn generation_worker_v18_missing_singleton_fails_without_recording_or_repairing() {
        let fixture = MigratedTestDatabase::new("astro-studio-generation-worker-v18-missing-test");
        remove_v18_lease_migration_state(&fixture.database);
        {
            let conn = fixture.database.conn.lock().expect("lock missing row db");
            conn.execute("DELETE FROM generation_worker_lease", [])
                .expect("remove v17 singleton before v18");
        }

        let error = fixture
            .database
            .run_migrations()
            .expect_err("v18 must fail-stop when the recorded-v17 singleton is missing");
        assert!(
            format!("{error:?}").contains("exactly one generation worker lease row"),
            "unexpected missing singleton error: {error:?}"
        );
        let conn = fixture.database.conn.lock().expect("lock failed v18 db");
        assert!(!migration_version_exists(&conn, 18));
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM generation_worker_lease", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("count missing singleton"),
            0
        );
        for trigger in V18_LEASE_TRIGGER_NAMES {
            assert!(!trigger_exists(&conn, trigger));
        }
    }

    #[test]
    fn generation_worker_v18_rolls_back_triggers_when_recording_fails() {
        let fixture = MigratedTestDatabase::new("astro-studio-generation-worker-v18-atomic-test");
        remove_v18_lease_migration_state(&fixture.database);
        {
            let conn = fixture.database.conn.lock().expect("lock v18 atomic db");
            conn.execute_batch(
                "CREATE TRIGGER suppress_v18_migration_record
                 BEFORE INSERT ON schema_migrations
                 WHEN NEW.version = 18
                 BEGIN
                     SELECT RAISE(IGNORE);
                 END;",
            )
            .expect("install migration-record suppression");
        }

        fixture
            .database
            .run_migrations()
            .expect_err("v18 record failure must fail the migration");
        let conn = fixture
            .database
            .conn
            .lock()
            .expect("lock rolled back v18 db");
        assert!(!migration_version_exists(&conn, 18));
        assert!(trigger_exists(
            &conn,
            "prevent_generation_worker_lease_epoch_decrease"
        ));
        for trigger in V18_LEASE_TRIGGER_NAMES {
            assert!(
                !trigger_exists(&conn, trigger),
                "failed v18 left trigger {trigger} behind"
            );
        }
    }

    #[test]
    fn generation_worker_v18_is_idempotent_and_concurrent() {
        let db_path = test_db_path("astro-studio-generation-worker-v18-concurrent-test");
        let directory = TestDatabaseDirectory(
            db_path
                .parent()
                .expect("test database parent")
                .to_path_buf(),
        );
        let setup = Database::open(&db_path).expect("open v18 setup db");
        setup.run_migrations().expect("run initial migrations");
        remove_v18_lease_migration_state(&setup);
        drop(setup);

        let databases = [
            Database::open(&db_path).expect("open first v18 migrator"),
            Database::open(&db_path).expect("open second v18 migrator"),
        ];
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
        let handles = databases.map(|database| {
            let barrier = std::sync::Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                database
                    .run_migrations()
                    .map_err(|error| format!("{error:?}"))
            })
        });
        barrier.wait();
        for handle in handles {
            handle
                .join()
                .expect("v18 migration thread panicked")
                .expect("concurrent v18 migration must succeed");
        }

        let database = Database::open(&db_path).expect("reopen v18 db");
        database
            .run_migrations()
            .expect("repeated v18 migration must be idempotent");
        let conn = database.conn.lock().expect("lock concurrent v18 db");
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM schema_migrations WHERE version = 18",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("count v18 migration records"),
            1
        );
        for trigger in V18_LEASE_TRIGGER_NAMES {
            assert!(trigger_exists(&conn, trigger));
        }
        drop(conn);
        drop(database);
        drop(directory);
    }
}

fn ensure_schema_migrations(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )
    .map_err(|e| AppError::Database {
        message: format!("Create schema_migrations failed: {}", e),
    })
}

fn migration_applied(conn: &Connection, version: i32) -> Result<bool, AppError> {
    let count: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM schema_migrations WHERE version = ?1",
            params![version],
            |row| row.get(0),
        )
        .map_err(|e| AppError::Database {
            message: format!("Check migration version {version} failed: {e}"),
        })?;
    Ok(count > 0)
}

fn record_migration(conn: &Connection, version: i32) -> Result<(), AppError> {
    let inserted = conn
        .execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)
             ON CONFLICT(version) DO NOTHING",
            params![version, crate::current_timestamp()],
        )
        .map_err(|e| AppError::Database {
            message: format!("record migration {version}: {e}"),
        })?;
    if inserted == 0 && !migration_applied(conn, version)? {
        return Err(AppError::Database {
            message: format!("record migration {version}: version row was not persisted"),
        });
    }
    Ok(())
}

fn migration_transaction_error(context: &str, error: rusqlite::Error) -> AppError {
    AppError::Database {
        message: format!("{context}: {error}"),
    }
}

fn rollback_migration(conn: &Connection) {
    let _ = conn.execute_batch("ROLLBACK");
}

fn begin_migration(conn: &Connection) -> Result<(), AppError> {
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| migration_transaction_error("Configure migration writer wait", error))?;
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|error| migration_transaction_error("Begin migration transaction", error))
}

fn commit_migration(conn: &Connection) -> Result<(), AppError> {
    conn.execute_batch("COMMIT").map_err(|error| {
        rollback_migration(conn);
        migration_transaction_error("Commit migration transaction", error)
    })
}

fn execute_migration_sql(conn: &Connection, sql: &str) -> Result<(), AppError> {
    for statement in sql
        .split(';')
        .map(str::trim)
        .filter(|stmt| !stmt.is_empty())
    {
        conn.execute_batch(statement).or_else(|e| {
            let msg = e.to_string();
            if msg.contains("already exists") || msg.contains("duplicate column") {
                Ok(())
            } else {
                Err(AppError::Database {
                    message: format!("Migration SQL failed: {}", msg),
                })
            }
        })?;
    }

    Ok(())
}

fn apply_migration(
    conn: &Connection,
    version: i32,
    _description: &str,
    sql: &str,
) -> Result<(), AppError> {
    begin_migration(conn)?;
    let result = (|| {
        if migration_applied(conn, version)? {
            return Ok(());
        }
        execute_migration_sql(conn, sql)?;
        record_migration(conn, version)
    })();
    if let Err(error) = result {
        rollback_migration(conn);
        return Err(error);
    }
    commit_migration(conn)
}

fn apply_migration_batch<F>(
    conn: &Connection,
    version: i32,
    _description: &str,
    guard: F,
    sql: &str,
) -> Result<(), AppError>
where
    F: FnOnce(&Connection) -> Result<(), AppError>,
{
    begin_migration(conn)?;
    let result = (|| {
        if migration_applied(conn, version)? {
            return Ok(());
        }
        guard(conn)?;
        conn.execute_batch(sql)
            .map_err(|error| AppError::Database {
                message: format!("Migration {version} SQL batch failed: {error}"),
            })?;
        record_migration(conn, version)
    })();
    if let Err(error) = result {
        rollback_migration(conn);
        return Err(error);
    }
    commit_migration(conn)
}

fn ensure_migration_compatibility(conn: &Connection) -> Result<(), AppError> {
    execute_migration_sql(
        conn,
        "ALTER TABLE projects ADD COLUMN deleted_at TEXT;
        ALTER TABLE conversations ADD COLUMN archived_at TEXT;
        ALTER TABLE conversations ADD COLUMN pinned_at TEXT;
        ALTER TABLE conversations ADD COLUMN deleted_at TEXT;
        UPDATE conversations SET project_id = 'default' WHERE project_id IS NULL;
        CREATE INDEX IF NOT EXISTS idx_conversations_project_id ON conversations(project_id);
        CREATE INDEX IF NOT EXISTS idx_conversations_pinned_at ON conversations(pinned_at);
        CREATE INDEX IF NOT EXISTS idx_conversations_archived_at ON conversations(archived_at);
        CREATE INDEX IF NOT EXISTS idx_conversations_deleted_at ON conversations(deleted_at);
        CREATE INDEX IF NOT EXISTS idx_projects_updated_at ON projects(updated_at);
        CREATE INDEX IF NOT EXISTS idx_projects_deleted_at ON projects(deleted_at);",
    )
    .map_err(|error| AppError::Database {
        message: format!("Repair project conversation migration failed: {}", error),
    })
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, AppError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AppError::FileSystem {
                message: format!("Create db dir failed: {}", e),
            })?;
        }
        let conn = Connection::open(path).map_err(|e| AppError::Database {
            message: format!("Open db failed: {}", e),
        })?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| AppError::Database {
                message: format!("PRAGMA failed: {}", e),
            })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn run_migrations(&self) -> Result<(), AppError> {
        let conn = self.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;

        ensure_schema_migrations(&conn)?;

        // v1: Core tables
        apply_migration(
            &conn,
            1,
            "core tables",
            "CREATE TABLE IF NOT EXISTS generations (
                id TEXT PRIMARY KEY,
                prompt TEXT NOT NULL,
                engine TEXT NOT NULL DEFAULT 'gpt-image-2',
                size TEXT NOT NULL DEFAULT '1024x1024',
                quality TEXT NOT NULL DEFAULT 'auto',
                status TEXT NOT NULL DEFAULT 'pending',
                error_message TEXT,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );
            CREATE TABLE IF NOT EXISTS images (
                id TEXT PRIMARY KEY,
                generation_id TEXT NOT NULL REFERENCES generations(id) ON DELETE CASCADE,
                file_path TEXT NOT NULL,
                thumbnail_path TEXT,
                width INTEGER NOT NULL DEFAULT 0,
                height INTEGER NOT NULL DEFAULT 0,
                file_size INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_images_generation_id ON images(generation_id);
            CREATE INDEX IF NOT EXISTS idx_generations_created_at ON generations(created_at);",
        )?;

        // v2: Conversations
        apply_migration(
            &conn,
            2,
            "conversations",
            "CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );
            ALTER TABLE generations ADD COLUMN conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL;
            ALTER TABLE generations ADD COLUMN error_message TEXT;
            CREATE INDEX IF NOT EXISTS idx_generations_conversation_id ON generations(conversation_id);",
        )?;

        // v3: Soft-delete for generations
        apply_migration(
            &conn,
            3,
            "generation soft delete",
            "ALTER TABLE generations ADD COLUMN deleted_at TEXT;
            CREATE INDEX IF NOT EXISTS idx_generations_deleted_at ON generations(deleted_at);",
        )?;

        // v4: Generation request parameters
        apply_migration(
            &conn,
            4,
            "generation request params",
            "ALTER TABLE generations ADD COLUMN request_kind TEXT NOT NULL DEFAULT 'generate';
            ALTER TABLE generations ADD COLUMN background TEXT NOT NULL DEFAULT 'auto';
            ALTER TABLE generations ADD COLUMN output_format TEXT NOT NULL DEFAULT 'png';
            ALTER TABLE generations ADD COLUMN output_compression INTEGER NOT NULL DEFAULT 100;
            ALTER TABLE generations ADD COLUMN moderation TEXT NOT NULL DEFAULT 'auto';
            ALTER TABLE generations ADD COLUMN input_fidelity TEXT NOT NULL DEFAULT 'high';
            ALTER TABLE generations ADD COLUMN image_count INTEGER NOT NULL DEFAULT 1;
            ALTER TABLE generations ADD COLUMN source_image_count INTEGER NOT NULL DEFAULT 0;
            ALTER TABLE generations ADD COLUMN source_image_paths TEXT NOT NULL DEFAULT '[]';
            ALTER TABLE generations ADD COLUMN request_metadata TEXT;",
        )?;

        // v5: Generation / conversation indexes
        apply_migration(
            &conn,
            5,
            "generation indexes",
            "CREATE INDEX IF NOT EXISTS idx_generations_engine ON generations(engine);
            CREATE INDEX IF NOT EXISTS idx_generations_request_kind ON generations(request_kind);
            CREATE INDEX IF NOT EXISTS idx_generations_size ON generations(size);
            CREATE INDEX IF NOT EXISTS idx_generations_quality ON generations(quality);
            CREATE INDEX IF NOT EXISTS idx_generations_output_format ON generations(output_format);
            CREATE INDEX IF NOT EXISTS idx_conversations_updated_at ON conversations(updated_at);",
        )?;

        // v6: Projects
        apply_migration(
            &conn,
            6,
            "projects",
            "CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                archived_at TEXT,
                pinned_at TEXT,
                deleted_at TEXT
            );
            INSERT OR IGNORE INTO projects (id, name, created_at, updated_at)
                VALUES ('default', 'Default Project', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));
            ALTER TABLE conversations ADD COLUMN project_id TEXT REFERENCES projects(id) ON DELETE SET NULL;",
        )?;

        // v7: Projects / conversations extensions
        apply_migration(
            &conn,
            7,
            "project / conversation extensions",
            "ALTER TABLE projects ADD COLUMN deleted_at TEXT;
            ALTER TABLE conversations ADD COLUMN archived_at TEXT;
            ALTER TABLE conversations ADD COLUMN pinned_at TEXT;
            ALTER TABLE conversations ADD COLUMN deleted_at TEXT;
            UPDATE conversations SET project_id = 'default' WHERE project_id IS NULL;
            CREATE INDEX IF NOT EXISTS idx_conversations_project_id ON conversations(project_id);
            CREATE INDEX IF NOT EXISTS idx_conversations_pinned_at ON conversations(pinned_at);
            CREATE INDEX IF NOT EXISTS idx_conversations_archived_at ON conversations(archived_at);
            CREATE INDEX IF NOT EXISTS idx_conversations_deleted_at ON conversations(deleted_at);
            CREATE INDEX IF NOT EXISTS idx_projects_updated_at ON projects(updated_at);
            CREATE INDEX IF NOT EXISTS idx_projects_deleted_at ON projects(deleted_at);",
        )?;

        // v8: Image folders
        apply_migration(
            &conn,
            8,
            "image folders",
            "CREATE TABLE IF NOT EXISTS folders (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );
            CREATE TABLE IF NOT EXISTS folder_images (
                folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
                image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
                added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                PRIMARY KEY (folder_id, image_id)
            );
            CREATE INDEX IF NOT EXISTS idx_folder_images_image_id ON folder_images(image_id);
            INSERT OR IGNORE INTO folders (id, name, created_at)
                VALUES ('default', '默认收藏', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));",
        )?;

        // v9: Logs
        apply_migration(
            &conn,
            9,
            "logs",
            "CREATE TABLE IF NOT EXISTS logs (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                log_type TEXT NOT NULL,
                level TEXT NOT NULL DEFAULT 'info',
                message TEXT NOT NULL,
                generation_id TEXT,
                metadata TEXT,
                response_file TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON logs(timestamp);
            CREATE INDEX IF NOT EXISTS idx_logs_type ON logs(log_type);",
        )?;

        // v10: Generation recoveries
        apply_migration(
            &conn,
            10,
            "generation recoveries",
            "CREATE TABLE IF NOT EXISTS generation_recoveries (
                generation_id TEXT PRIMARY KEY REFERENCES generations(id) ON DELETE CASCADE,
                request_kind TEXT NOT NULL,
                request_state TEXT NOT NULL,
                output_format TEXT NOT NULL,
                response_file TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_generation_recoveries_state ON generation_recoveries(request_state);",
        )?;

        // v11: Prompt favorites
        apply_migration(
            &conn,
            11,
            "prompt favorites",
            "CREATE TABLE IF NOT EXISTS prompt_favorites (
                id TEXT PRIMARY KEY,
                prompt TEXT NOT NULL COLLATE NOCASE,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_prompt_favorites_prompt ON prompt_favorites(prompt);
            CREATE INDEX IF NOT EXISTS idx_prompt_favorites_updated_at ON prompt_favorites(updated_at);",
        )?;

        // v12: Prompt folders
        apply_migration(
            &conn,
            12,
            "prompt folders",
            "CREATE TABLE IF NOT EXISTS prompt_folders (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );
            CREATE TABLE IF NOT EXISTS prompt_folder_favorites (
                folder_id TEXT NOT NULL REFERENCES prompt_folders(id) ON DELETE CASCADE,
                prompt_favorite_id TEXT NOT NULL REFERENCES prompt_favorites(id) ON DELETE CASCADE,
                added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                PRIMARY KEY (folder_id, prompt_favorite_id)
            );
            CREATE INDEX IF NOT EXISTS idx_prompt_folder_favorites_favorite_id ON prompt_folder_favorites(prompt_favorite_id);
            INSERT OR IGNORE INTO prompt_folders (id, name, created_at) VALUES ('default', 'Default', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));
            UPDATE prompt_folders SET name = '默认收藏夹' WHERE id = 'default' AND name <> '默认收藏夹';",
        )?;

        // v13: Prompt extractions
        apply_migration(
            &conn,
            13,
            "prompt extractions",
            "CREATE TABLE IF NOT EXISTS prompt_extractions (
                id TEXT PRIMARY KEY,
                image_path TEXT NOT NULL,
                prompt TEXT NOT NULL,
                llm_config_id TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );
            CREATE INDEX IF NOT EXISTS idx_prompt_extractions_updated_at ON prompt_extractions(updated_at);",
        )?;

        // v14: Canvas documents
        apply_migration(
            &conn,
            14,
            "canvas documents",
            "CREATE TABLE IF NOT EXISTS canvas_documents (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                document_path TEXT NOT NULL,
                preview_path TEXT,
                width INTEGER NOT NULL DEFAULT 0,
                height INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                deleted_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_canvas_documents_project_id ON canvas_documents(project_id);
            CREATE INDEX IF NOT EXISTS idx_canvas_documents_updated_at ON canvas_documents(updated_at);
            CREATE INDEX IF NOT EXISTS idx_canvas_documents_deleted_at ON canvas_documents(deleted_at);",
        )?;

        // v15: Prompt agent sessions
        apply_migration(
            &conn,
            15,
            "prompt agent sessions",
            "CREATE TABLE IF NOT EXISTS prompt_agent_sessions (
                id TEXT PRIMARY KEY,
                conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
                project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                status TEXT NOT NULL DEFAULT 'active',
                original_prompt TEXT NOT NULL,
                draft_prompt TEXT,
                accepted_prompt TEXT,
                selected_skill_ids TEXT NOT NULL DEFAULT '[]',
                suggested_params TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );
            CREATE TABLE IF NOT EXISTS prompt_agent_messages (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES prompt_agent_sessions(id) ON DELETE CASCADE,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                draft_prompt TEXT,
                selected_skill_ids TEXT NOT NULL DEFAULT '[]',
                suggested_params TEXT NOT NULL DEFAULT '{}',
                ready_to_generate INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );
            CREATE INDEX IF NOT EXISTS idx_prompt_agent_sessions_conversation_id ON prompt_agent_sessions(conversation_id);
            CREATE INDEX IF NOT EXISTS idx_prompt_agent_sessions_project_id ON prompt_agent_sessions(project_id);
            CREATE INDEX IF NOT EXISTS idx_prompt_agent_sessions_updated_at ON prompt_agent_sessions(updated_at);
            CREATE INDEX IF NOT EXISTS idx_prompt_agent_messages_session_id ON prompt_agent_messages(session_id);
            CREATE INDEX IF NOT EXISTS idx_prompt_agent_messages_created_at ON prompt_agent_messages(created_at);",
        )?;

        // v16: Durable generation job queue
        apply_migration(
            &conn,
            16,
            "generation jobs",
            "CREATE TABLE IF NOT EXISTS generation_jobs (
                id TEXT PRIMARY KEY,
                client_request_id TEXT NOT NULL UNIQUE,
                generation_id TEXT NOT NULL UNIQUE REFERENCES generations(id) ON DELETE CASCADE,
                parent_job_id TEXT REFERENCES generation_jobs(id) ON DELETE SET NULL,
                source_kind TEXT NOT NULL,
                source_ref_json TEXT NOT NULL DEFAULT '{}',
                status TEXT NOT NULL,
                request_json TEXT NOT NULL,
                provider_kind TEXT NOT NULL,
                provider_profile_id TEXT NOT NULL,
                endpoint_snapshot TEXT NOT NULL,
                chain_attempt INTEGER NOT NULL DEFAULT 1,
                auto_attempt INTEGER NOT NULL DEFAULT 0,
                max_auto_attempts INTEGER NOT NULL DEFAULT 2,
                queued_at TEXT NOT NULL,
                started_at TEXT,
                finished_at TEXT,
                cancel_requested_at TEXT,
                last_heartbeat_at TEXT,
                error_code TEXT,
                error_message TEXT,
                retryable INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_generation_jobs_status_queued
                ON generation_jobs(status, queued_at);
            CREATE INDEX IF NOT EXISTS idx_generation_jobs_parent
                ON generation_jobs(parent_job_id);
            CREATE INDEX IF NOT EXISTS idx_generation_jobs_source
                ON generation_jobs(source_kind);",
        )?;

        // v17: Managed generation worker state and response artifact metadata
        apply_migration(
            &conn,
            17,
            "generation worker state",
            "ALTER TABLE generation_jobs
                ADD COLUMN stage TEXT NOT NULL DEFAULT 'migration_unknown';
            UPDATE generation_jobs
                SET stage = CASE status
                    WHEN 'queued' THEN 'queued'
                    WHEN 'running' THEN 'startup_reconciliation'
                    WHEN 'completed' THEN 'terminal'
                    WHEN 'failed' THEN 'terminal'
                    WHEN 'cancelled' THEN 'terminal'
                    WHEN 'interrupted' THEN 'terminal'
                    ELSE 'migration_unknown'
                END
                WHERE stage = 'migration_unknown';
            CREATE TABLE IF NOT EXISTS generation_recoveries (
                generation_id TEXT PRIMARY KEY REFERENCES generations(id) ON DELETE CASCADE,
                request_kind TEXT NOT NULL,
                request_state TEXT NOT NULL,
                output_format TEXT NOT NULL,
                response_file TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_generation_recoveries_state
                ON generation_recoveries(request_state);
            INSERT INTO generation_recoveries (
                generation_id, request_kind, request_state, output_format, response_file,
                created_at, updated_at
            )
            SELECT j.generation_id, g.request_kind, 'requesting', g.output_format, NULL,
                   j.queued_at, j.queued_at
              FROM generation_jobs j
              JOIN generations g ON g.id = j.generation_id
             WHERE j.status = 'queued'
               AND j.stage = 'queued'
               AND NOT EXISTS (
                   SELECT 1 FROM generation_recoveries r
                    WHERE r.generation_id = j.generation_id
               );
            ALTER TABLE generation_recoveries ADD COLUMN expected_response_file TEXT;
            ALTER TABLE generation_recoveries
                ADD COLUMN response_size INTEGER
                CHECK (
                    response_size IS NULL
                    OR (typeof(response_size) = 'integer' AND response_size >= 0)
                );
            ALTER TABLE generation_recoveries ADD COLUMN response_sha256 TEXT;
            CREATE TABLE IF NOT EXISTS generation_worker_lease (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                owner_id TEXT,
                fencing_epoch INTEGER NOT NULL DEFAULT 0
                    CHECK (typeof(fencing_epoch) = 'integer' AND fencing_epoch >= 0),
                acquired_at INTEGER
                    CHECK (acquired_at IS NULL OR (typeof(acquired_at) = 'integer' AND acquired_at >= 0)),
                heartbeat_at INTEGER
                    CHECK (heartbeat_at IS NULL OR (typeof(heartbeat_at) = 'integer' AND heartbeat_at >= 0)),
                expires_at INTEGER
                    CHECK (expires_at IS NULL OR (typeof(expires_at) = 'integer' AND expires_at >= 0)),
                CHECK (
                    (
                        owner_id IS NULL
                        AND acquired_at IS NULL
                        AND heartbeat_at IS NULL
                        AND expires_at IS NULL
                    )
                    OR (
                        typeof(owner_id) = 'text'
                        AND length(trim(owner_id)) > 0
                        AND acquired_at IS NOT NULL
                        AND heartbeat_at IS NOT NULL
                        AND expires_at IS NOT NULL
                        AND fencing_epoch > 0
                        AND acquired_at <= heartbeat_at
                        AND heartbeat_at < expires_at
                    )
                )
            );
            INSERT OR IGNORE INTO generation_worker_lease (
                id, owner_id, fencing_epoch, acquired_at, heartbeat_at, expires_at
            ) VALUES (1, NULL, 0, NULL, NULL, NULL);",
        )?;

        conn.execute_batch(
            "CREATE TRIGGER IF NOT EXISTS prevent_generation_worker_lease_epoch_decrease
             BEFORE UPDATE OF fencing_epoch ON generation_worker_lease
             WHEN NEW.fencing_epoch < OLD.fencing_epoch
             BEGIN
                 SELECT RAISE(ABORT, 'generation worker fencing epoch decreased');
             END;",
        )
        .map_err(|error| AppError::Database {
            message: format!("Create generation worker lease fence trigger failed: {error}"),
        })?;

        // v18: Seal the singleton lease row and constrain every update to one
        // of the three legal fencing transitions. This uses execute_batch in
        // its own immediate transaction because trigger bodies contain
        // semicolons and cannot pass through the legacy statement splitter.
        apply_migration_batch(
            &conn,
            18,
            "seal generation worker lease transitions",
            |conn| {
                let (row_count, minimum_id, maximum_id) = conn
                    .query_row(
                        "SELECT COUNT(*), MIN(id), MAX(id) FROM generation_worker_lease",
                        [],
                        |row| {
                            Ok((
                                row.get::<_, i64>(0)?,
                                row.get::<_, Option<i64>>(1)?,
                                row.get::<_, Option<i64>>(2)?,
                            ))
                        },
                    )
                    .map_err(|error| AppError::Database {
                        message: format!(
                            "Validate generation worker lease singleton before migration 18: {error}"
                        ),
                    })?;
                if (row_count, minimum_id, maximum_id) != (1, Some(1), Some(1)) {
                    return Err(AppError::Database {
                        message: format!(
                            "migration 18 requires exactly one generation worker lease row with id 1; found count={row_count}, min_id={minimum_id:?}, max_id={maximum_id:?}"
                        ),
                    });
                }
                Ok(())
            },
            "CREATE TRIGGER prevent_generation_worker_lease_delete
             BEFORE DELETE ON generation_worker_lease
             BEGIN
                 SELECT RAISE(ABORT, 'generation worker lease row cannot be deleted');
             END;

             CREATE TRIGGER prevent_generation_worker_lease_insert
             BEFORE INSERT ON generation_worker_lease
             BEGIN
                 SELECT RAISE(ABORT, 'generation worker lease row is sealed');
             END;

             CREATE TRIGGER enforce_generation_worker_lease_transition
             BEFORE UPDATE ON generation_worker_lease
             WHEN CASE
                 WHEN OLD.owner_id IS NOT NULL
                      AND NEW.id = OLD.id
                      AND NEW.owner_id = OLD.owner_id
                      AND NEW.fencing_epoch = OLD.fencing_epoch
                      AND NEW.acquired_at = OLD.acquired_at
                      AND NEW.heartbeat_at >= OLD.heartbeat_at
                      AND NEW.heartbeat_at < OLD.expires_at
                      AND NEW.expires_at >= OLD.expires_at
                 THEN 0
                 WHEN OLD.owner_id IS NOT NULL
                      AND NEW.id = OLD.id
                      AND NEW.owner_id IS NULL
                      AND NEW.fencing_epoch = OLD.fencing_epoch
                      AND NEW.acquired_at IS NULL
                      AND NEW.heartbeat_at IS NULL
                      AND NEW.expires_at IS NULL
                 THEN 0
                 WHEN NEW.id = OLD.id
                      AND NEW.owner_id IS NOT NULL
                      AND OLD.fencing_epoch < 9223372036854775807
                      AND NEW.fencing_epoch = OLD.fencing_epoch + 1
                      AND NEW.acquired_at = NEW.heartbeat_at
                      AND (
                          (
                              OLD.owner_id IS NULL
                              AND OLD.acquired_at IS NULL
                              AND OLD.heartbeat_at IS NULL
                              AND OLD.expires_at IS NULL
                          )
                          OR OLD.expires_at <= NEW.acquired_at
                      )
                 THEN 0
                 ELSE 1
             END = 1
             BEGIN
                 SELECT RAISE(ABORT, 'invalid generation worker lease transition');
             END;",
        )?;

        ensure_migration_compatibility(&conn)?;

        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, AppError> {
        let conn = self.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        let mut stmt = conn
            .prepare("SELECT value FROM settings WHERE key = ?1")
            .map_err(|e| AppError::Database {
                message: format!("prepare get_setting: {}", e),
            })?;
        Ok(stmt
            .query_row(params![key], |row| row.get::<_, String>(0))
            .ok())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), AppError> {
        let conn = self.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )
        .map_err(|e| AppError::Database {
            message: format!("set_setting: {}", e),
        })?;
        Ok(())
    }

    pub fn insert_log(
        &self,
        log_type: &str,
        level: &str,
        message: &str,
        generation_id: Option<&str>,
        metadata: Option<&str>,
        response_file: Option<&str>,
    ) -> Result<(), AppError> {
        let conn = self.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO logs (id, timestamp, log_type, level, message, generation_id, metadata, response_file) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                crate::current_timestamp(),
                log_type,
                level,
                message,
                generation_id,
                metadata,
                response_file
            ],
        )
        .map_err(|e| AppError::Database {
            message: format!("insert_log: {}", e),
        })?;
        Ok(())
    }

    pub fn search_logs(
        &self,
        log_type: Option<&str>,
        level: Option<&str>,
        page: i32,
        page_size: i32,
    ) -> Result<(Vec<LogEntry>, i32), AppError> {
        let conn = self.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        let page = page.max(1);
        let page_size = page_size.max(1);
        let offset = (page - 1) * page_size;

        let mut where_clauses = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(t) = log_type {
            where_clauses.push(format!("log_type = ?{}", param_values.len() + 1));
            param_values.push(Box::new(t.to_string()));
        }
        if let Some(l) = level {
            where_clauses.push(format!("level = ?{}", param_values.len() + 1));
            param_values.push(Box::new(l.to_string()));
        }

        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let count_sql = format!("SELECT COUNT(*) FROM logs {}", where_sql);
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let count: i32 = conn
            .query_row(&count_sql, params_refs.as_slice(), |row| row.get(0))
            .map_err(|e| AppError::Database {
                message: format!("search_logs count: {}", e),
            })?;

        let query_sql = format!(
            "SELECT id, timestamp, log_type, level, message, generation_id, metadata, response_file \
             FROM logs {} ORDER BY timestamp DESC LIMIT ?{} OFFSET ?{}",
            where_sql,
            param_values.len() + 1,
            param_values.len() + 2
        );
        let mut all_params: Vec<Box<dyn rusqlite::types::ToSql>> = param_values;
        all_params.push(Box::new(page_size));
        all_params.push(Box::new(offset));
        let all_refs: Vec<&dyn rusqlite::types::ToSql> =
            all_params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&query_sql).map_err(|e| AppError::Database {
            message: format!("search_logs query: {}", e),
        })?;
        let logs = stmt
            .query_map(all_refs.as_slice(), |row| {
                Ok(LogEntry {
                    id: row.get("id")?,
                    timestamp: row.get("timestamp")?,
                    log_type: row.get("log_type")?,
                    level: row.get("level")?,
                    message: row.get("message")?,
                    generation_id: row.get("generation_id")?,
                    metadata: row.get("metadata")?,
                    response_file: row.get("response_file")?,
                })
            })
            .map_err(|e| AppError::Database {
                message: format!("search_logs map: {}", e),
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok((logs, count))
    }

    pub fn get_log(&self, id: &str) -> Result<Option<LogEntry>, AppError> {
        let conn = self.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        let mut stmt = conn
            .prepare("SELECT id, timestamp, log_type, level, message, generation_id, metadata, response_file FROM logs WHERE id = ?1")
            .map_err(|e| AppError::Database {
                message: format!("get_log: {}", e),
            })?;
        Ok(stmt
            .query_row(params![id], |row| {
                Ok(LogEntry {
                    id: row.get("id")?,
                    timestamp: row.get("timestamp")?,
                    log_type: row.get("log_type")?,
                    level: row.get("level")?,
                    message: row.get("message")?,
                    generation_id: row.get("generation_id")?,
                    metadata: row.get("metadata")?,
                    response_file: row.get("response_file")?,
                })
            })
            .ok())
    }

    pub fn response_file_exists(&self, path: &str) -> Result<bool, AppError> {
        let conn = self.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        let exists =
            conn.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM logs WHERE response_file = ?1
                    UNION
                    SELECT 1 FROM generation_recoveries WHERE response_file = ?1
                )",
                params![path],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| AppError::Database {
                message: format!("response_file_exists: {}", e),
            })? != 0;
        Ok(exists)
    }

    pub fn image_file_exists(&self, path: &str) -> Result<bool, AppError> {
        let conn = self.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        let exists =
            conn.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM images WHERE file_path = ?1 OR thumbnail_path = ?1
                )",
                params![path],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| AppError::Database {
                message: format!("image_file_exists: {}", e),
            })? != 0;
        Ok(exists)
    }

    pub fn clear_logs_before(&self, before_timestamp: &str) -> Result<u64, AppError> {
        let conn = self.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT response_file FROM logs WHERE response_file IS NOT NULL AND timestamp <= ?1",
            )
            .map_err(|e| AppError::Database {
                message: format!("clear_logs_before select: {}", e),
            })?;
        let files: Vec<String> = stmt
            .query_map(params![before_timestamp], |row| row.get::<_, String>(0))
            .map_err(|e| AppError::Database {
                message: format!("clear_logs_before map: {}", e),
            })?
            .filter_map(|r| r.ok())
            .collect();

        for f in &files {
            let _ = std::fs::remove_file(f);
        }

        let deleted = conn
            .execute(
                "DELETE FROM logs WHERE timestamp <= ?1",
                params![before_timestamp],
            )
            .map_err(|e| AppError::Database {
                message: format!("clear_logs_before delete: {}", e),
            })?;
        Ok(deleted as u64)
    }

    pub fn get_trash_retention_days(&self) -> Result<u32, AppError> {
        Ok(self
            .get_setting(crate::models::SETTING_TRASH_RETENTION_DAYS)?
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(crate::models::DEFAULT_TRASH_RETENTION_DAYS))
    }
}
