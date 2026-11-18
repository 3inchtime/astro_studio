use super::with_library_connection;
use crate::error::AppError;
use crate::projects;
use crate::{db::Database, models::*};
use tauri::State;

#[tauri::command]
pub(crate) fn create_project(
    db: State<'_, Database>,
    name: Option<String>,
) -> Result<Project, AppError> {
    with_library_connection(db.inner(), |conn| projects::create_project(conn, name))
}

#[tauri::command]
pub(crate) fn get_projects(
    db: State<'_, Database>,
    include_archived: Option<bool>,
) -> Result<Vec<Project>, AppError> {
    with_library_connection(db.inner(), |conn| {
        projects::get_projects(conn, include_archived)
    })
}

#[tauri::command]
pub(crate) fn rename_project(
    db: State<'_, Database>,
    id: String,
    name: String,
) -> Result<(), AppError> {
    with_library_connection(db.inner(), |conn| projects::rename_project(conn, id, name))
}

#[tauri::command]
pub(crate) fn archive_project(db: State<'_, Database>, id: String) -> Result<(), AppError> {
    with_library_connection(db.inner(), |conn| projects::archive_project(conn, id))
}

#[tauri::command]
pub(crate) fn unarchive_project(db: State<'_, Database>, id: String) -> Result<(), AppError> {
    with_library_connection(db.inner(), |conn| projects::unarchive_project(conn, id))
}

#[tauri::command]
pub(crate) fn pin_project(db: State<'_, Database>, id: String) -> Result<(), AppError> {
    with_library_connection(db.inner(), |conn| projects::pin_project(conn, id))
}

#[tauri::command]
pub(crate) fn unpin_project(db: State<'_, Database>, id: String) -> Result<(), AppError> {
    with_library_connection(db.inner(), |conn| projects::unpin_project(conn, id))
}

#[tauri::command]
pub(crate) fn delete_project(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    id: String,
) -> Result<(), AppError> {
    crate::generation_execution::delete_with_jobs(
        &app,
        db.inner(),
        crate::generation_execution::DeletionScope::Project(&id),
        |conn| projects::delete_project(conn, &id),
    )
}
