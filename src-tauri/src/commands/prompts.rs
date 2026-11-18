use super::with_library_connection;
use crate::collections::prompts;
use crate::error::AppError;
use crate::{db::Database, models::*};
use tauri::State;

#[tauri::command]
pub(crate) fn create_prompt_favorite(
    db: State<'_, Database>,
    prompt: String,
) -> Result<PromptFavorite, AppError> {
    with_library_connection(db.inner(), |conn| {
        prompts::create_prompt_favorite(conn, prompt)
    })
}

#[tauri::command]
pub(crate) fn get_prompt_favorites(
    db: State<'_, Database>,
    query: Option<String>,
    folder_id: Option<String>,
) -> Result<Vec<PromptFavorite>, AppError> {
    with_library_connection(db.inner(), |conn| {
        prompts::get_prompt_favorites(conn, query, folder_id)
    })
}

#[tauri::command]
pub(crate) fn delete_prompt_favorite(db: State<'_, Database>, id: String) -> Result<(), AppError> {
    with_library_connection(db.inner(), |conn| prompts::delete_prompt_favorite(conn, id))
}

#[tauri::command]
pub(crate) fn create_prompt_folder(
    db: State<'_, Database>,
    name: String,
) -> Result<Folder, AppError> {
    with_library_connection(db.inner(), |conn| prompts::create_prompt_folder(conn, name))
}

#[tauri::command]
pub(crate) fn rename_prompt_folder(
    db: State<'_, Database>,
    id: String,
    name: String,
) -> Result<(), AppError> {
    with_library_connection(db.inner(), |conn| {
        prompts::rename_prompt_folder(conn, id, name)
    })
}

#[tauri::command]
pub(crate) fn delete_prompt_folder(db: State<'_, Database>, id: String) -> Result<(), AppError> {
    with_library_connection(db.inner(), |conn| prompts::delete_prompt_folder(conn, id))
}

#[tauri::command]
pub(crate) fn get_prompt_folders(db: State<'_, Database>) -> Result<Vec<Folder>, AppError> {
    with_library_connection(db.inner(), prompts::get_prompt_folders)
}

#[tauri::command]
pub(crate) fn add_prompt_favorite_to_folders(
    db: State<'_, Database>,
    favorite_id: String,
    folder_ids: Vec<String>,
) -> Result<(), AppError> {
    with_library_connection(db.inner(), |conn| {
        prompts::add_prompt_favorite_to_folders(conn, favorite_id, folder_ids)
    })
}

#[tauri::command]
pub(crate) fn remove_prompt_favorite_from_folders(
    db: State<'_, Database>,
    favorite_id: String,
    folder_ids: Vec<String>,
) -> Result<(), AppError> {
    with_library_connection(db.inner(), |conn| {
        prompts::remove_prompt_favorite_from_folders(conn, favorite_id, folder_ids)
    })
}

#[tauri::command]
pub(crate) fn get_prompt_favorite_folders(
    db: State<'_, Database>,
    favorite_id: String,
) -> Result<Vec<String>, AppError> {
    with_library_connection(db.inner(), |conn| {
        prompts::get_prompt_favorite_folders(conn, favorite_id)
    })
}
