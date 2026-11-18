use crate::collections::images;
use crate::{db::Database, models::*};
use tauri::State;

fn with_library_connection<T>(
    db: &Database,
    operation: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
) -> Result<T, String> {
    let _permit = crate::storage::write_operation::<String>()?;
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    operation(&conn)
}

#[tauri::command]
pub(crate) fn search_generations(
    db: State<'_, Database>,
    query: Option<String>,
    page: Option<i32>,
    only_deleted: Option<bool>,
    filters: Option<GenerationSearchFilters>,
    project_id: Option<String>,
) -> Result<SearchResult, String> {
    with_library_connection(db.inner(), |conn| {
        images::search_generations(conn, query, page, only_deleted, filters, project_id)
    })
}

#[tauri::command]
pub(crate) fn create_folder(db: State<'_, Database>, name: String) -> Result<Folder, String> {
    with_library_connection(db.inner(), |conn| images::create_folder(conn, name))
}

#[tauri::command]
pub(crate) fn rename_folder(
    db: State<'_, Database>,
    id: String,
    name: String,
) -> Result<(), String> {
    with_library_connection(db.inner(), |conn| images::rename_folder(conn, id, name))
}

#[tauri::command]
pub(crate) fn delete_folder(db: State<'_, Database>, id: String) -> Result<(), String> {
    with_library_connection(db.inner(), |conn| images::delete_folder(conn, id))
}

#[tauri::command]
pub(crate) fn get_folders(db: State<'_, Database>) -> Result<Vec<Folder>, String> {
    with_library_connection(db.inner(), images::get_folders)
}

#[tauri::command]
pub(crate) fn add_image_to_folders(
    db: State<'_, Database>,
    image_id: String,
    folder_ids: Vec<String>,
) -> Result<(), String> {
    with_library_connection(db.inner(), |conn| {
        images::add_image_to_folders(conn, image_id, folder_ids)
    })
}

#[tauri::command]
pub(crate) fn remove_image_from_folders(
    db: State<'_, Database>,
    image_id: String,
    folder_ids: Vec<String>,
) -> Result<(), String> {
    with_library_connection(db.inner(), |conn| {
        images::remove_image_from_folders(conn, image_id, folder_ids)
    })
}

#[tauri::command]
pub(crate) fn get_image_folders(
    db: State<'_, Database>,
    image_id: String,
) -> Result<Vec<String>, String> {
    with_library_connection(db.inner(), |conn| images::get_image_folders(conn, image_id))
}

#[tauri::command]
pub(crate) fn get_favorite_images(
    db: State<'_, Database>,
    folder_id: Option<String>,
    query: Option<String>,
    page: Option<i32>,
) -> Result<SearchResult, String> {
    with_library_connection(db.inner(), |conn| {
        images::get_favorite_images(conn, folder_id, query, page)
    })
}
