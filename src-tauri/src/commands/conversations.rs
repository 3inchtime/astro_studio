use super::with_library_connection;
use crate::error::AppError;
use crate::projects::conversations;
use crate::{db::Database, models::*};
use tauri::State;

#[tauri::command]
pub(crate) fn create_conversation(
    db: State<'_, Database>,
    title: Option<String>,
    project_id: Option<String>,
) -> Result<Conversation, AppError> {
    with_library_connection(db.inner(), |conn| {
        conversations::create_conversation(conn, title, project_id)
    })
}

#[tauri::command]
pub(crate) fn get_conversations(
    db: State<'_, Database>,
    query: Option<String>,
    project_id: Option<String>,
    include_archived: Option<bool>,
) -> Result<Vec<Conversation>, AppError> {
    with_library_connection(db.inner(), |conn| {
        conversations::get_conversations(conn, query, project_id, include_archived)
    })
}

#[tauri::command]
pub(crate) fn rename_conversation(
    db: State<'_, Database>,
    id: String,
    title: String,
) -> Result<(), AppError> {
    with_library_connection(db.inner(), |conn| {
        conversations::rename_conversation(conn, id, title)
    })
}

#[tauri::command]
pub(crate) fn move_conversation_to_project(
    db: State<'_, Database>,
    id: String,
    project_id: String,
) -> Result<(), AppError> {
    with_library_connection(db.inner(), |conn| {
        conversations::move_conversation_to_project(conn, id, project_id)
    })
}

#[tauri::command]
pub(crate) fn archive_conversation(db: State<'_, Database>, id: String) -> Result<(), AppError> {
    with_library_connection(db.inner(), |conn| {
        conversations::archive_conversation(conn, id)
    })
}

#[tauri::command]
pub(crate) fn unarchive_conversation(db: State<'_, Database>, id: String) -> Result<(), AppError> {
    with_library_connection(db.inner(), |conn| {
        conversations::unarchive_conversation(conn, id)
    })
}

#[tauri::command]
pub(crate) fn pin_conversation(db: State<'_, Database>, id: String) -> Result<(), AppError> {
    with_library_connection(db.inner(), |conn| conversations::pin_conversation(conn, id))
}

#[tauri::command]
pub(crate) fn unpin_conversation(db: State<'_, Database>, id: String) -> Result<(), AppError> {
    with_library_connection(db.inner(), |conn| {
        conversations::unpin_conversation(conn, id)
    })
}

#[tauri::command]
pub(crate) fn delete_conversation(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    id: String,
) -> Result<(), AppError> {
    crate::generation_execution::delete_with_jobs(
        &app,
        db.inner(),
        crate::generation_execution::DeletionScope::Conversation(&id),
        |conn| conversations::delete_conversation(conn, &id),
    )
}

#[tauri::command]
pub(crate) fn get_conversation_generations(
    db: State<'_, Database>,
    conversation_id: String,
) -> Result<Vec<GenerationResult>, AppError> {
    with_library_connection(db.inner(), |conn| {
        conversations::get_conversation_generations(conn, conversation_id)
    })
}
