pub(crate) mod canvas;
pub(crate) mod conversations;
pub(crate) mod generation;
pub(crate) mod llm;
pub(crate) mod logs;
pub(crate) mod projects;
pub(crate) mod prompt_agent;
pub(crate) mod prompts;
pub(crate) mod settings;

pub(crate) mod collections;

fn with_library_connection<T>(
    db: &crate::db::Database,
    operation: impl FnOnce(&rusqlite::Connection) -> Result<T, crate::error::AppError>,
) -> Result<T, crate::error::AppError> {
    let _permit = crate::storage::write_operation::<crate::error::AppError>()?;
    let conn = db
        .conn
        .lock()
        .map_err(|e| crate::error::AppError::Database {
            message: format!("Lock failed: {e}"),
        })?;
    operation(&conn)
}
