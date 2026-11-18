use crate::{
    db::Database,
    error::AppError,
    llm::{self, ImageData, MULTIMODAL_TIMEOUT_SECS},
    models::LlmConfig,
};
use std::path::PathBuf;

pub(crate) fn load_registered_images(
    app: &tauri::AppHandle,
    db: &Database,
    registry: &crate::source_images::SelectedImageRegistry,
    paths: &[String],
) -> Result<Vec<ImageData>, AppError> {
    let paths = crate::source_images::resolve_source_image_paths(app, db, registry, paths)?
        .into_iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    load_images(&paths)
}

pub(super) fn load_images(paths: &[PathBuf]) -> Result<Vec<ImageData>, AppError> {
    let mut images = Vec::new();
    for path in paths {
        let data = std::fs::read(path).map_err(|e| AppError::Validation {
            message: format!("Failed to read image '{}': {}", path.display(), e),
        })?;
        let media_type = crate::source_images::validate_source_image_data(path.as_path(), &data)?;
        images.push(ImageData {
            data,
            media_type: media_type.to_string(),
        });
    }
    Ok(images)
}

pub(crate) fn create_multimodal_llm_client(
    config: &LlmConfig,
) -> Result<Box<dyn llm::LlmClient>, AppError> {
    llm::create_client_with_timeout(config, MULTIMODAL_TIMEOUT_SECS)
}
