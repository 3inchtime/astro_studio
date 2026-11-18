use super::{decode_base64_bytes, ensure_canvas_dirs};
use crate::{error::AppError, source_images::SelectedImageRegistry};
use std::{fs, path::Path};

fn validate_canvas_document_id_component(document_id: &str) -> Result<String, AppError> {
    let document_id = document_id.trim();
    if uuid::Uuid::parse_str(document_id).is_err() {
        return Err(AppError::Validation {
            message: "Invalid canvas document id.".to_string(),
        });
    }
    Ok(document_id.to_string())
}

pub(crate) fn save(
    root: &Path,
    document_id: &str,
    png_base64: &str,
    registry: &SelectedImageRegistry,
) -> Result<String, AppError> {
    let exports_dir = ensure_canvas_dirs(root)?.exports;
    let exports_dir = exports_dir
        .canonicalize()
        .map_err(|e| AppError::FileSystem {
            message: format!("Resolve canvas exports dir failed: {}", e),
        })?;
    let document_id = validate_canvas_document_id_component(document_id)?;
    let document_dir = exports_dir.join(&document_id);
    if !document_dir.starts_with(&exports_dir) {
        return Err(AppError::Validation {
            message: "Canvas export path is outside managed storage.".to_string(),
        });
    }
    fs::create_dir_all(&document_dir).map_err(|e| AppError::FileSystem {
        message: format!("Create canvas export dir failed: {}", e),
    })?;

    let bytes = decode_base64_bytes(png_base64)?;
    let file_path = document_dir.join(format!("{}-frame.png", uuid::Uuid::new_v4()));
    write_registered_canvas_export(&file_path, &bytes, registry)
}

fn write_registered_canvas_export(
    file_path: &Path,
    bytes: &[u8],
    registry: &SelectedImageRegistry,
) -> Result<String, AppError> {
    fs::write(file_path, bytes).map_err(|error| AppError::FileSystem {
        message: format!("Write canvas export failed: {error}"),
    })?;
    // Generated canvas inputs enter the same explicit selection registry as
    // picker imports. Keep the generic image-edit file boundary unchanged.
    registry
        .register_paths(&[file_path.to_path_buf()])?
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Validation {
            message: "Canvas export registration failed.".to_string(),
        })
}

#[cfg(test)]
mod tests;
