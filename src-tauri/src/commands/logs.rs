use crate::error::AppError;
use crate::models::*;
use crate::runtime_logs;
use crate::db::Database;
use crate::format_log_clear_cutoff;
use chrono::Utc;
use tauri::State;

#[tauri::command]
pub(crate) fn get_logs(
    db: State<'_, Database>,
    log_type: Option<String>,
    level: Option<String>,
    page: Option<i32>,
    page_size: Option<i32>,
) -> Result<LogSearchResult, AppError> {
    let page = page.unwrap_or(1);
    let page_size = page_size.unwrap_or(DEFAULT_PAGE_SIZE);
    let (logs, total) = db.search_logs(log_type.as_deref(), level.as_deref(), page, page_size)?;
    Ok(LogSearchResult {
        logs,
        total,
        page,
        page_size,
    })
}

#[tauri::command]
pub(crate) fn get_runtime_logs(limit: Option<usize>) -> Result<Vec<RuntimeLogEntry>, AppError> {
    Ok(runtime_logs::recent_logs(limit.unwrap_or(200)))
}

#[tauri::command]
pub(crate) fn get_log_detail(
    db: State<'_, Database>,
    id: String,
) -> Result<LogEntry, AppError> {
    db.get_log(&id)?
        .ok_or_else(|| AppError::Validation {
            message: "Log not found".to_string(),
        })
}

#[tauri::command]
pub(crate) fn read_log_response_file(path: String) -> Result<String, AppError> {
    std::fs::read_to_string(&path)
        .map_err(|e| AppError::FileSystem {
            message: format!("Read failed: {}", e),
        })
}

#[tauri::command]
pub(crate) fn clear_logs(
    db: State<'_, Database>,
    before_days: Option<u32>,
) -> Result<u64, AppError> {
    let days = before_days.unwrap_or(DEFAULT_LOG_RETENTION_DAYS);
    let before_str = format_log_clear_cutoff(Utc::now(), days);
    Ok(db.clear_logs_before(&before_str)?)
}

#[tauri::command]
pub(crate) fn get_log_settings(db: State<'_, Database>) -> Result<LogSettings, AppError> {
    let enabled = db
        .get_setting(SETTING_LOG_ENABLED)?
        .map(|v| v == "true")
        .unwrap_or(true);
    let retention_days = db
        .get_setting(SETTING_LOG_RETENTION_DAYS)?
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(DEFAULT_LOG_RETENTION_DAYS);
    Ok(LogSettings {
        enabled,
        retention_days,
    })
}

#[tauri::command]
pub(crate) fn save_log_settings(
    db: State<'_, Database>,
    enabled: bool,
    retention_days: u32,
) -> Result<(), AppError> {
    db.set_setting(SETTING_LOG_ENABLED, if enabled { "true" } else { "false" })?;
    db.set_setting(
        SETTING_LOG_RETENTION_DAYS,
        &retention_days.to_string(),
    )?;
    Ok(())
}

#[tauri::command]
pub(crate) fn get_trash_settings(db: State<'_, Database>) -> Result<TrashSettings, AppError> {
    Ok(TrashSettings {
        retention_days: db.get_trash_retention_days()?,
    })
}

#[tauri::command]
pub(crate) fn save_trash_settings(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    retention_days: u32,
) -> Result<(), AppError> {
    let retention_days = retention_days.max(1);
    db.set_setting(
        SETTING_TRASH_RETENTION_DAYS,
        &retention_days.to_string(),
    )?;
    let _ = crate::gallery::purge_trashed_generations(&app, db.inner(), retention_days);
    Ok(())
}
