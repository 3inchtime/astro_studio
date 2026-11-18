use crate::{db::Database, error::AppError, file_manager};
use std::{
    collections::HashSet,
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
};

pub(crate) const MAX_SOURCE_IMAGE_BYTES: u64 = 50 * 1024 * 1024;

#[derive(Default)]
pub(crate) struct SelectedImageRegistry {
    paths: Mutex<HashSet<PathBuf>>,
}

impl SelectedImageRegistry {
    pub(crate) fn register_paths(&self, paths: &[PathBuf]) -> Result<Vec<String>, AppError> {
        let mut canonical_paths = Vec::with_capacity(paths.len());
        for path in paths {
            canonical_paths.push(canonicalize_source_image_path(path)?);
        }

        let mut selected = self.paths.lock().map_err(|e| AppError::Validation {
            message: format!("Lock selected image registry failed: {}", e),
        })?;
        for path in &canonical_paths {
            selected.insert(path.clone());
        }

        Ok(canonical_paths
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect())
    }

    pub(crate) fn contains_path(&self, path: &Path) -> Result<bool, AppError> {
        let canonical_path = canonicalize_source_image_path(path)?;
        let selected = self.paths.lock().map_err(|e| AppError::Validation {
            message: format!("Lock selected image registry failed: {}", e),
        })?;
        Ok(selected.contains(&canonical_path))
    }
}

pub(crate) fn canonicalize_source_image_path(path: &Path) -> Result<PathBuf, AppError> {
    let canonical_path = path.canonicalize().map_err(|e| AppError::Validation {
        message: format!("Resolve selected image failed: {}", e),
    })?;

    if !canonical_path.is_file() {
        return Err(AppError::Validation {
            message: "Selected image path is not a file.".to_string(),
        });
    }

    source_image_media_type_for_path(&canonical_path)?;
    Ok(canonical_path)
}

pub(crate) fn source_image_media_type_for_path(path: &Path) -> Result<&'static str, AppError> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => Ok("image/jpeg"),
        Some("png") => Ok("image/png"),
        Some("webp") => Ok("image/webp"),
        Some(extension) => Err(AppError::Validation {
            message: format!(
                "Unsupported image format '{}'. Supported: jpg, png, webp",
                extension
            ),
        }),
        None => Err(AppError::Validation {
            message: "Selected image has no file extension.".to_string(),
        }),
    }
}

pub(crate) fn validate_source_image_data(
    path: &Path,
    data: &[u8],
) -> Result<&'static str, AppError> {
    let media_type = source_image_media_type_for_path(path)?;
    if data.len() as u64 > MAX_SOURCE_IMAGE_BYTES {
        return Err(AppError::Validation {
            message: format!("Image '{}' exceeds 50MB limit", path.display()),
        });
    }

    let has_expected_magic = match media_type {
        "image/jpeg" => data.starts_with(b"\xff\xd8\xff"),
        "image/png" => data.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/webp" => data.len() >= 12 && data.starts_with(b"RIFF") && &data[8..12] == b"WEBP",
        _ => false,
    };

    if !has_expected_magic {
        return Err(AppError::Validation {
            message: format!("Image '{}' has invalid image data", path.display()),
        });
    }

    Ok(media_type)
}

pub(crate) fn validate_edit_overlay(source: &[u8], overlay: &[u8]) -> Result<(), AppError> {
    use image::{GenericImageView, ImageDecoder};
    let invalid = || {
        AppError::Validation { message: "Overlay must be a transparent PNG matching the first oriented image, at most 50 MB and 16,777,216 pixels.".into() }
    };
    if overlay.len() as u64 > MAX_SOURCE_IMAGE_BYTES || !overlay.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(invalid());
    }
    let mut decoder = image::ImageReader::new(std::io::Cursor::new(source))
        .with_guessed_format()
        .map_err(|_| invalid())?
        .into_decoder()
        .map_err(|_| invalid())?;
    let source_dimensions = decoder.dimensions();
    if u64::from(source_dimensions.0) * u64::from(source_dimensions.1) > 16_777_216 {
        return Err(invalid());
    }
    let orientation = decoder.orientation().map_err(|_| invalid())?;
    let mut source = image::DynamicImage::from_decoder(decoder).map_err(|_| invalid())?;
    source.apply_orientation(orientation);
    let reader = image::ImageReader::new(std::io::Cursor::new(overlay))
        .with_guessed_format()
        .map_err(|_| invalid())?;
    let dimensions = reader.into_dimensions().map_err(|_| invalid())?;
    if u64::from(dimensions.0) * u64::from(dimensions.1) > 16_777_216
        || dimensions != source.dimensions()
    {
        return Err(invalid());
    }
    let overlay = image::load_from_memory(overlay)
        .map_err(|_| invalid())?
        .to_rgba8();
    if !overlay.pixels().any(|pixel| pixel[3] < 255) {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn validate_source_image_file(path: &Path) -> Result<(), AppError> {
    let metadata = std::fs::metadata(path).map_err(|e| AppError::Validation {
        message: format!("Read selected image metadata failed: {}", e),
    })?;
    if metadata.len() > MAX_SOURCE_IMAGE_BYTES {
        return Err(AppError::Validation {
            message: format!("Image '{}' exceeds 50MB limit", path.display()),
        });
    }

    let mut file = std::fs::File::open(path).map_err(|e| AppError::Validation {
        message: format!("Open selected image failed: {}", e),
    })?;
    let mut header = [0u8; 16];
    let bytes_read = file.read(&mut header).map_err(|e| AppError::Validation {
        message: format!("Read selected image header failed: {}", e),
    })?;
    validate_source_image_data(path, &header[..bytes_read]).map(|_| ())
}

pub(crate) fn resolve_source_image_paths(
    app: &tauri::AppHandle,
    db: &Database,
    registry: &SelectedImageRegistry,
    paths: &[String],
) -> Result<Vec<String>, AppError> {
    let mut resolved_paths = Vec::with_capacity(paths.len());
    for path in paths {
        let resolved_path = if db.image_file_exists(path)? {
            validate_managed_image_path(app, db, path)?
        } else if registry.contains_path(Path::new(path))? {
            canonicalize_source_image_path(Path::new(path))?
        } else {
            return Err(AppError::Validation {
                message: "Image file was not selected from Astro Studio.".to_string(),
            });
        };
        validate_source_image_file(&resolved_path)?;
        resolved_paths.push(resolved_path.to_string_lossy().to_string());
    }

    Ok(resolved_paths)
}

fn managed_image_roots(app: &tauri::AppHandle) -> Result<Vec<PathBuf>, AppError> {
    let app_data_dir = crate::storage::asset_root(app).map_err(|e| AppError::FileSystem {
        message: format!("Get app data dir failed: {}", e),
    })?;
    Ok(vec![
        app_data_dir.join("images"),
        app_data_dir.join("thumbnails"),
        app_data_dir.join("canvas").join("assets"),
    ])
}

pub(crate) fn validate_managed_image_path(
    app: &tauri::AppHandle,
    db: &Database,
    image_path: &str,
) -> Result<PathBuf, AppError> {
    validate_recorded_managed_image_path(db, image_path, &managed_image_roots(app)?)
}

fn validate_recorded_managed_image_path(
    db: &Database,
    image_path: &str,
    allowed_roots: &[PathBuf],
) -> Result<PathBuf, AppError> {
    if !db.image_file_exists(image_path)? {
        return Err(AppError::Validation {
            message: "Image file is not recorded.".to_string(),
        });
    }

    file_manager::canonicalize_existing_managed_path(Path::new(image_path), allowed_roots)
}

#[cfg(test)]
mod path_boundary_tests {
    use super::*;

    #[test]
    fn copy_image_to_clipboard_rejects_outside_path_before_reading() {
        let dir = std::env::temp_dir().join(format!(
            "astro-studio-image-boundary-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        let outside_file = dir.join("outside.txt");
        std::fs::write(&outside_file, "not an allowed image").expect("write outside file");

        let result = file_manager::canonicalize_existing_managed_path(
            &outside_file,
            &[
                dir.join("app-data").join("images"),
                dir.join("app-data").join("thumbnails"),
            ],
        );

        assert!(matches!(result, Err(AppError::Validation { .. })));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn recorded_managed_image_paths_are_valid_source_images_without_picker_registration() {
        let dir = std::env::temp_dir().join(format!(
            "astro-studio-managed-source-test-{}",
            uuid::Uuid::new_v4()
        ));
        let db_path = dir.join("astro_studio.db");
        let image_dir = dir.join("app-data").join("images").join("2026");
        std::fs::create_dir_all(&image_dir).expect("create image dir");
        let image_path = image_dir.join("generated.png");
        std::fs::write(&image_path, b"\x89PNG\r\n\x1a\nrest").expect("write image");

        let db = Database::open(&db_path).expect("open db");
        db.run_migrations().expect("run migrations");
        {
            let conn = db.conn.lock().expect("lock db");
            conn.execute(
                "INSERT INTO generations (id, prompt) VALUES (?1, ?2)",
                rusqlite::params!["generation-1", "prompt"],
            )
            .expect("insert generation");
            conn.execute(
                "INSERT INTO images (id, generation_id, file_path) VALUES (?1, ?2, ?3)",
                rusqlite::params![
                    "image-1",
                    "generation-1",
                    image_path.to_string_lossy().to_string()
                ],
            )
            .expect("insert image");
        }

        let result = validate_recorded_managed_image_path(
            &db,
            &image_path.to_string_lossy(),
            &[dir.join("app-data").join("images")],
        );

        assert_eq!(result.unwrap(), image_path.canonicalize().unwrap());

        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }
}
