use crate::api_gateway;
use crate::db::Database;
use crate::error::AppError;
use crate::file_manager;
use crate::generation_lifecycle::{
    run_generation_lifecycle, GenerationLifecycleKind, GenerationLifecycleRequest,
};
use crate::models::*;
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Manager, State};

pub(crate) const MAX_SOURCE_IMAGE_BYTES: u64 = 10 * 1024 * 1024;

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

fn canonicalize_source_image_path(path: &Path) -> Result<PathBuf, AppError> {
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
            message: format!("Image '{}' exceeds 10MB limit", path.display()),
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

fn validate_source_image_file(path: &Path) -> Result<(), AppError> {
    let metadata = std::fs::metadata(path).map_err(|e| AppError::Validation {
        message: format!("Read selected image metadata failed: {}", e),
    })?;
    if metadata.len() > MAX_SOURCE_IMAGE_BYTES {
        return Err(AppError::Validation {
            message: format!("Image '{}' exceeds 10MB limit", path.display()),
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

// ── Commands ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub(crate) async fn generate_image(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    engine_state: State<'_, api_gateway::GptImageEngine>,
    prompt: String,
    model: Option<String>,
    size: Option<String>,
    quality: Option<String>,
    background: Option<String>,
    output_format: Option<String>,
    output_compression: Option<u8>,
    moderation: Option<String>,
    image_count: Option<u8>,
    conversation_id: Option<String>,
    project_id: Option<String>,
) -> Result<GenerateResult, AppError> {
    run_generation_lifecycle(
        &app,
        db.inner(),
        engine_state.inner(),
        GenerationLifecycleRequest {
            kind: GenerationLifecycleKind::Generate,
            prompt,
            model,
            source_image_paths: Vec::new(),
            size,
            quality,
            background,
            output_format,
            output_compression,
            moderation,
            input_fidelity: None,
            image_count,
            conversation_id,
            project_id,
        },
    )
    .await
}

#[tauri::command]
pub(crate) async fn edit_image(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    engine_state: State<'_, api_gateway::GptImageEngine>,
    selected_images: State<'_, SelectedImageRegistry>,
    prompt: String,
    model: Option<String>,
    source_image_paths: Vec<String>,
    size: Option<String>,
    quality: Option<String>,
    background: Option<String>,
    input_fidelity: Option<String>,
    output_format: Option<String>,
    output_compression: Option<u8>,
    moderation: Option<String>,
    image_count: Option<u8>,
    conversation_id: Option<String>,
    project_id: Option<String>,
) -> Result<GenerateResult, AppError> {
    if source_image_paths.is_empty() {
        return Err(AppError::Validation {
            message: "Please select at least one source image.".to_string(),
        });
    }
    let source_image_paths = resolve_source_image_paths(
        &app,
        db.inner(),
        selected_images.inner(),
        &source_image_paths,
    )?;

    run_generation_lifecycle(
        &app,
        db.inner(),
        engine_state.inner(),
        GenerationLifecycleRequest {
            kind: GenerationLifecycleKind::Edit,
            prompt,
            model,
            source_image_paths,
            size,
            quality,
            background,
            output_format,
            output_compression,
            moderation,
            input_fidelity,
            image_count,
            conversation_id,
            project_id,
        },
    )
    .await
}

// ── Lightbox commands ────────────────────────────────────────────────────────

fn managed_image_roots(app: &tauri::AppHandle) -> Result<Vec<PathBuf>, AppError> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::FileSystem {
            message: format!("Get app data dir failed: {}", e),
        })?;
    Ok(vec![
        app_data_dir.join("images"),
        app_data_dir.join("thumbnails"),
    ])
}

fn validate_managed_image_path(
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

#[tauri::command]
pub(crate) fn copy_image_to_clipboard(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    image_path: String,
) -> Result<(), AppError> {
    let image_path = validate_managed_image_path(&app, db.inner(), &image_path)?;
    let data = std::fs::read(&image_path).map_err(|e| AppError::FileSystem {
        message: format!("Read image failed: {}", e),
    })?;
    let img = image::load_from_memory(&data).map_err(|e| AppError::FileSystem {
        message: format!("Decode image failed: {}", e),
    })?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();

    let mut clipboard = arboard::Clipboard::new().map_err(|e| AppError::FileSystem {
        message: format!("Clipboard access failed: {}", e),
    })?;
    clipboard
        .set_image(arboard::ImageData {
            width: w as usize,
            height: h as usize,
            bytes: std::borrow::Cow::Owned(rgba.into_raw()),
        })
        .map_err(|e| AppError::FileSystem {
            message: format!("Copy to clipboard failed: {}", e),
        })?;

    Ok(())
}

#[tauri::command]
pub(crate) async fn save_image_to_file(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    image_path: String,
) -> Result<(), AppError> {
    let image_path = validate_managed_image_path(&app, db.inner(), &image_path)?;
    let file_name = image_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image.png");

    let save_path = rfd::AsyncFileDialog::new()
        .set_file_name(file_name)
        .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
        .save_file()
        .await
        .ok_or_else(|| AppError::Validation {
            message: "Save cancelled".to_string(),
        })?;

    let save_path = save_path.path().to_path_buf();
    tokio::task::spawn_blocking(move || {
        std::fs::copy(&image_path, &save_path)
            .map(|_| ())
            .map_err(|e| AppError::FileSystem {
                message: format!("Save failed: {}", e),
            })
    })
    .await
    .map_err(|e| AppError::Validation {
        message: format!("Spawn blocking failed: {}", e),
    })?
}

#[tauri::command]
pub(crate) async fn pick_source_images(
    selected_images: State<'_, SelectedImageRegistry>,
) -> Result<Vec<String>, AppError> {
    let files = rfd::AsyncFileDialog::new()
        .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
        .pick_files()
        .await;

    let Some(files) = files else {
        return Ok(vec![]);
    };

    let paths: Vec<PathBuf> = files
        .into_iter()
        .map(|file| file.path().to_path_buf())
        .collect();
    selected_images.register_paths(&paths)
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
