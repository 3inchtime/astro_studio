use crate::error::AppError;
use crate::models::LogEntry;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Database {
    pub conn: Arc<Mutex<Connection>>,
    pub(crate) credentials: Arc<crate::credentials::Credentials>,
}

mod schema;
#[cfg(test)]
mod tests;
pub(crate) use schema::LATEST_SCHEMA_VERSION;

impl Database {
    pub(crate) fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, AppError> {
        self.conn.lock().map_err(|error| AppError::Database {
            message: error.to_string(),
        })
    }

    pub fn open(path: &Path) -> Result<Self, AppError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| AppError::FileSystem {
                message: format!("Create db dir failed: {}", e),
            })?;
        }
        if path.is_file() {
            schema::preflight(path)?;
        }
        let conn = Connection::open(path).map_err(|e| AppError::Database {
            message: format!("Open db failed: {}", e),
        })?;
        conn.execute_batch("PRAGMA foreign_keys=ON;")
            .map_err(|e| AppError::Database {
                message: format!("PRAGMA failed: {}", e),
            })?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            credentials: Arc::new(crate::credentials::Credentials::default()),
        })
    }

    #[cfg(test)]
    pub(crate) fn open_with_vault(
        path: &Path,
        vault: Arc<dyn crate::credentials::CredentialVault>,
    ) -> Result<Self, AppError> {
        let mut database = Self::open(path)?;
        database.credentials = Arc::new(crate::credentials::Credentials::new(vault));
        Ok(database)
    }

    #[cfg(test)]
    pub fn run_migrations(&self) -> Result<(), AppError> {
        self.run_library_migrations(None)
    }

    pub(crate) fn run_library_migrations(&self, root: Option<&Path>) -> Result<(), AppError> {
        let mut conn = self.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {e}"),
        })?;
        schema::upgrade(&mut conn, root)
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
                    SELECT 1 FROM library_assets WHERE file_path = ?1 OR thumbnail_path = ?1
                )",
                params![path],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| AppError::Database {
                message: format!("image_file_exists: {}", e),
            })? != 0;
        drop(conn);
        if exists {
            return Ok(true);
        }
        Ok(crate::storage::assets::asset_for_path(self, path)?.is_some())
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
