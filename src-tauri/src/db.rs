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

    fn migration_version_exists(conn: &Connection, version: i32) -> bool {
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
            params![version],
            |row| row.get::<_, i64>(0),
        )
        .expect("query migration version")
            != 0
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
            assert!(table_has_column(&conn, "prompt_agent_sessions", "conversation_id"));
            assert!(table_has_column(&conn, "prompt_agent_sessions", "suggested_params"));
            assert!(table_has_column(&conn, "prompt_agent_messages", "session_id"));
            assert!(table_has_column(&conn, "prompt_agent_messages", "ready_to_generate"));
            assert!(migration_version_exists(&conn, 15));
        }

        std::fs::remove_dir_all(db_path.parent().expect("db parent")).ok();
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
    conn.execute(
        "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
        params![version, crate::current_timestamp()],
    )
    .map_err(|e| AppError::Database {
        message: format!("record migration {}: {}", version, e),
    })?;
    Ok(())
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
    if migration_applied(conn, version)? {
        return Ok(());
    }
    execute_migration_sql(conn, sql)?;
    record_migration(conn, version)
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
