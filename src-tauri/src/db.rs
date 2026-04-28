use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

use crate::models::LogEntry;
use chrono::{SecondsFormat, Utc};

fn current_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn migrate_step(conn: &Connection, sql: &str) -> Result<(), String> {
    conn.execute_batch(sql).or_else(|e| {
        let msg = e.to_string();
        if msg.contains("already exists") || msg.contains("duplicate column") {
            Ok(())
        } else {
            Err(format!("Migration failed: {}", msg))
        }
    })
}

pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Create db dir failed: {}", e))?;
        }
        let conn = Connection::open(path).map_err(|e| format!("Open db failed: {}", e))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| format!("PRAGMA failed: {}", e))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn run_migrations(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS generations (
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
            CREATE INDEX IF NOT EXISTS idx_generations_created_at ON generations(created_at);
            "#,
        )
        .map_err(|e| format!("Migration failed: {}", e))?;

        migrate_step(
            &conn,
            "CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );",
        )?;

        migrate_step(&conn,
            "ALTER TABLE generations ADD COLUMN conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL;"
        )?;

        migrate_step(
            &conn,
            "ALTER TABLE generations ADD COLUMN error_message TEXT;",
        )?;

        migrate_step(&conn,
            "CREATE INDEX IF NOT EXISTS idx_generations_conversation_id ON generations(conversation_id);"
        )?;

        migrate_step(&conn, "ALTER TABLE generations ADD COLUMN deleted_at TEXT;")?;

        migrate_step(
            &conn,
            "CREATE INDEX IF NOT EXISTS idx_generations_deleted_at ON generations(deleted_at);",
        )?;

        migrate_step(
            &conn,
            "CREATE INDEX IF NOT EXISTS idx_conversations_updated_at ON conversations(updated_at);",
        )?;

        migrate_step(
            &conn,
            "CREATE TABLE IF NOT EXISTS folders (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );",
        )?;

        migrate_step(
            &conn,
            "CREATE TABLE IF NOT EXISTS folder_images (
                folder_id TEXT NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
                image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
                added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                PRIMARY KEY (folder_id, image_id)
            );",
        )?;

        migrate_step(
            &conn,
            "CREATE INDEX IF NOT EXISTS idx_folder_images_image_id ON folder_images(image_id);",
        )?;

        migrate_step(&conn,
            "INSERT OR IGNORE INTO folders (id, name, created_at) VALUES ('default', '默认收藏', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));"
        )?;

        migrate_step(
            &conn,
            "CREATE TABLE IF NOT EXISTS logs (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                log_type TEXT NOT NULL,
                level TEXT NOT NULL DEFAULT 'info',
                message TEXT NOT NULL,
                generation_id TEXT,
                metadata TEXT,
                response_file TEXT
            );",
        )?;

        migrate_step(
            &conn,
            "CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON logs(timestamp);",
        )?;

        migrate_step(
            &conn,
            "CREATE INDEX IF NOT EXISTS idx_logs_type ON logs(log_type);",
        )?;

        migrate_step(
            &conn,
            "CREATE TABLE IF NOT EXISTS generation_recoveries (
                generation_id TEXT PRIMARY KEY REFERENCES generations(id) ON DELETE CASCADE,
                request_kind TEXT NOT NULL,
                request_state TEXT NOT NULL,
                output_format TEXT NOT NULL,
                response_file TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )?;

        migrate_step(
            &conn,
            "CREATE INDEX IF NOT EXISTS idx_generation_recoveries_state ON generation_recoveries(request_state);",
        )?;

        migrate_step(
            &conn,
            "CREATE TABLE IF NOT EXISTS prompt_favorites (
                id TEXT PRIMARY KEY,
                prompt TEXT NOT NULL COLLATE NOCASE,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );",
        )?;

        migrate_step(
            &conn,
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_prompt_favorites_prompt ON prompt_favorites(prompt);",
        )?;

        migrate_step(
            &conn,
            "CREATE INDEX IF NOT EXISTS idx_prompt_favorites_updated_at ON prompt_favorites(updated_at);",
        )?;

        migrate_step(
            &conn,
            "CREATE TABLE IF NOT EXISTS prompt_folders (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            );",
        )?;

        migrate_step(
            &conn,
            "CREATE TABLE IF NOT EXISTS prompt_folder_favorites (
                folder_id TEXT NOT NULL REFERENCES prompt_folders(id) ON DELETE CASCADE,
                prompt_favorite_id TEXT NOT NULL REFERENCES prompt_favorites(id) ON DELETE CASCADE,
                added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                PRIMARY KEY (folder_id, prompt_favorite_id)
            );",
        )?;

        migrate_step(
            &conn,
            "CREATE INDEX IF NOT EXISTS idx_prompt_folder_favorites_favorite_id ON prompt_folder_favorites(prompt_favorite_id);",
        )?;

        migrate_step(
            &conn,
            "INSERT OR IGNORE INTO prompt_folders (id, name, created_at) VALUES ('default', 'Default', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));",
        )?;

        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT value FROM settings WHERE key = ?1")
            .map_err(|e| e.to_string())?;
        Ok(stmt
            .query_row(params![key], |row| row.get::<_, String>(0))
            .ok())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )
        .map_err(|e| e.to_string())?;
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
    ) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = current_timestamp();
        conn.execute(
            "INSERT INTO logs (id, timestamp, log_type, level, message, generation_id, metadata, response_file) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, timestamp, log_type, level, message, generation_id, metadata, response_file],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn search_logs(
        &self,
        log_type: Option<&str>,
        level: Option<&str>,
        page: i32,
        page_size: i32,
    ) -> Result<(Vec<LogEntry>, i32), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
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
            .map_err(|e| e.to_string())?;

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

        let mut stmt = conn.prepare(&query_sql).map_err(|e| e.to_string())?;
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
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok((logs, count))
    }

    pub fn get_log(&self, id: &str) -> Result<Option<LogEntry>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare("SELECT id, timestamp, log_type, level, message, generation_id, metadata, response_file FROM logs WHERE id = ?1")
            .map_err(|e| e.to_string())?;
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

    pub fn clear_logs_before(&self, before_timestamp: &str) -> Result<u64, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare(
                "SELECT response_file FROM logs WHERE response_file IS NOT NULL AND timestamp <= ?1",
            )
            .map_err(|e| e.to_string())?;
        let files: Vec<String> = stmt
            .query_map(params![before_timestamp], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?
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
            .map_err(|e| e.to_string())?;
        Ok(deleted as u64)
    }

    pub fn get_trash_retention_days(&self) -> Result<u32, String> {
        Ok(self
            .get_setting(crate::models::SETTING_TRASH_RETENTION_DAYS)?
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(crate::models::DEFAULT_TRASH_RETENTION_DAYS))
    }
}
