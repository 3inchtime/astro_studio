use crate::{
    credentials::{LlmConfigDto, LlmConfigWrite},
    db::Database,
    error::AppError,
    llm::{config, prompts},
    models::PromptExtraction,
};
use tauri::State;

#[tauri::command]
pub(crate) fn get_llm_configs(db: State<'_, Database>) -> Result<Vec<LlmConfigDto>, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    config::get_llm_configs(db.inner())
}

#[tauri::command]
pub(crate) fn save_llm_configs(
    db: State<'_, Database>,
    configs: Vec<LlmConfigWrite>,
) -> Result<(), AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    config::save_llm_configs(db.inner(), configs)
}

#[tauri::command]
pub(crate) fn get_prompt_extractions(
    db: State<'_, Database>,
    limit: Option<u32>,
) -> Result<Vec<PromptExtraction>, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    prompts::get_prompt_extractions(db.inner(), limit)
}

#[tauri::command]
pub(crate) async fn optimize_prompt(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    selected_images: State<'_, crate::source_images::SelectedImageRegistry>,
    prompt: String,
    config_id: String,
    image_paths: Option<Vec<String>>,
) -> Result<String, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    prompts::optimize_prompt(
        &app,
        db.inner(),
        selected_images.inner(),
        prompt,
        config_id,
        image_paths,
    )
    .await
}

#[tauri::command]
pub(crate) async fn extract_prompt_from_image(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    selected_images: State<'_, crate::source_images::SelectedImageRegistry>,
    image_path: String,
    config_id: String,
    language: String,
) -> Result<PromptExtraction, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    prompts::extract_prompt_from_image(
        &app,
        db.inner(),
        selected_images.inner(),
        image_path,
        config_id,
        language,
    )
    .await
}
