use crate::canvas::{
    documents, exports,
    generation::{self, CanvasGenerationStatus},
};
use crate::db::Database;
use crate::error::AppError;
use crate::models::{CanvasDocument, CanvasDocumentContent, CanvasDocumentWithContent};
use crate::source_images::SelectedImageRegistry;
use std::path::{Path, PathBuf};
use tauri::{Manager, State};

fn asset_root(app: &tauri::AppHandle) -> Result<PathBuf, AppError> {
    crate::storage::asset_root(app).map_err(|e| AppError::FileSystem {
        message: e.to_string(),
    })
}

#[tauri::command]
pub(crate) fn create_canvas_document(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    project_id: Option<String>,
    name: Option<String>,
) -> Result<CanvasDocument, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    documents::create(
        &db,
        &asset_root(&app)?,
        project_id.as_deref(),
        name.as_deref(),
    )
}

#[tauri::command]
pub(crate) fn list_canvas_documents(
    db: State<'_, Database>,
    project_id: Option<String>,
) -> Result<Vec<CanvasDocument>, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    documents::list(&db, project_id.as_deref())
}

#[tauri::command]
pub(crate) fn get_canvas_document(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    id: String,
) -> Result<CanvasDocumentWithContent, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    documents::load(&db, &asset_root(&app)?, &id)
}

#[tauri::command]
pub(crate) fn save_canvas_document(
    app: tauri::AppHandle,
    selected_images: State<'_, SelectedImageRegistry>,
    db: State<'_, Database>,
    id: String,
    content: CanvasDocumentContent,
    preview_png_base64: Option<String>,
    expected_revision: i64,
) -> Result<CanvasDocument, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    documents::save(
        &db,
        &asset_root(&app)?,
        &id,
        content,
        preview_png_base64.as_deref(),
        expected_revision,
        &|path| selected_images.contains_path(path),
    )
}

#[tauri::command]
pub(crate) fn import_canvas_asset(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    selected_images: State<'_, SelectedImageRegistry>,
    path: String,
) -> Result<crate::canvas_assets::CanvasAsset, AppError> {
    let _operation = crate::storage::write_operation::<AppError>()?;
    if !selected_images.contains_path(Path::new(&path))? {
        return Err(AppError::Validation {
            message: "Image file was not selected from Astro Studio".into(),
        });
    }
    let root = asset_root(&app)?;
    let asset = crate::canvas_assets::import_asset(&db, &root, Path::new(&path))?;
    selected_images.register_paths(&[PathBuf::from(&asset.file_path)])?;
    Ok(asset)
}

#[tauri::command]
pub(crate) fn rename_canvas_document(
    db: State<'_, Database>,
    id: String,
    name: String,
) -> Result<CanvasDocument, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    documents::rename(&db, &id, &name)
}

#[tauri::command]
pub(crate) fn delete_canvas_document(db: State<'_, Database>, id: String) -> Result<(), AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    documents::delete(&db, &id)
}

#[tauri::command]
pub(crate) fn save_canvas_export(
    app: tauri::AppHandle,
    document_id: String,
    png_base64: String,
) -> Result<String, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    exports::save(
        &asset_root(&app)?,
        &document_id,
        &png_base64,
        app.state::<SelectedImageRegistry>().inner(),
    )
}

#[tauri::command]
pub(crate) fn get_canvas_generation_status(
    db: State<'_, Database>,
    document_id: String,
    request_id: String,
) -> Result<Option<CanvasGenerationStatus>, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    generation::read_canvas_generation_status(&*db.connection()?, &document_id, &request_id)
}
