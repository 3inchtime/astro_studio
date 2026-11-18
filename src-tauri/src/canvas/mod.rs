pub(crate) mod documents;
pub(crate) mod exports;
pub(crate) mod generation;
#[cfg(test)]
mod test_support;

use crate::error::AppError;
use base64::Engine;
use std::{
    fs,
    path::{Path, PathBuf},
};

struct CanvasDirectories {
    documents: PathBuf,
    exports: PathBuf,
}

fn ensure_canvas_dirs(asset_root: &Path) -> Result<CanvasDirectories, AppError> {
    let base_dir = asset_root.join("canvas");
    let documents_dir = base_dir.join("documents");
    let previews_dir = base_dir.join("previews");
    let exports_dir = base_dir.join("exports");

    fs::create_dir_all(&documents_dir).map_err(|e| AppError::FileSystem {
        message: format!("Create canvas documents dir failed: {}", e),
    })?;
    fs::create_dir_all(&previews_dir).map_err(|e| AppError::FileSystem {
        message: format!("Create canvas previews dir failed: {}", e),
    })?;
    fs::create_dir_all(&exports_dir).map_err(|e| AppError::FileSystem {
        message: format!("Create canvas exports dir failed: {}", e),
    })?;

    Ok(CanvasDirectories {
        documents: documents_dir,
        exports: exports_dir,
    })
}

fn decode_base64_bytes(value: &str) -> Result<Vec<u8>, AppError> {
    let payload = value
        .split_once(',')
        .map(|(_, encoded)| encoded)
        .unwrap_or(value);
    base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|e| AppError::Validation {
            message: format!("Invalid base64 payload: {}", e),
        })
}
