use crate::api_gateway;
use crate::db::Database;
use crate::error::AppError;
use crate::file_manager;
use crate::generation_execution::{
    submit_generation_request, GenerationExecutionMode, GenerationExecutionRequest,
};
use crate::generation_lifecycle::GenerationLifecycleKind;
use crate::models::*;
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Manager, State};

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

fn validate_source_image_file(path: &Path) -> Result<(), AppError> {
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

// Restart retries may reuse only the exact source list and overlay pair from a
// validated failed job. Picker registration still governs every other request.
fn persisted_edit_paths_match(
    request: &serde_json::Value,
    model: &str,
    sources: &[String],
    advanced: Option<&ImageAdvancedOptions>,
) -> bool {
    let saved_sources: Option<Vec<String>> =
        serde_json::from_value(request["source_image_paths"].clone()).ok();
    let saved_advanced: Option<ImageAdvancedOptions> =
        serde_json::from_value(request["options"]["advanced_options"].clone()).ok();
    request["model"].as_str() == Some(model)
        && saved_sources.as_deref() == Some(sources)
        && saved_advanced.as_ref().and_then(|a| a.markup_path.as_ref())
            == advanced.and_then(|a| a.markup_path.as_ref())
        && saved_advanced.as_ref().and_then(|a| a.mask_path.as_ref())
            == advanced.and_then(|a| a.mask_path.as_ref())
}

pub(crate) fn persisted_retry_authorizes_edit(
    db: &Database,
    model: Option<&str>,
    sources: &[String],
    advanced: Option<&ImageAdvancedOptions>,
) -> Result<bool, AppError> {
    let Some(model @ (ENGINE_GPT_IMAGE_26_FLARE | ENGINE_GPT_IMAGE_26_SUNBURST)) = model else {
        return Ok(false);
    };
    let conn = db.conn.lock().map_err(|_| AppError::Validation {
        message: "Cannot read saved edit authorization.".into(),
    })?;
    let ids = {
        let mut query = conn.prepare("SELECT j.id FROM generation_jobs j JOIN generations g ON g.id=j.generation_id WHERE j.status IN ('failed','interrupted') AND g.deleted_at IS NULL AND g.engine=?1 AND g.source_image_paths=?2")
            .map_err(|e| AppError::Database { message: e.to_string() })?;
        let rows = query
            .query_map(
                rusqlite::params![model, serde_json::to_string(sources).unwrap_or_default()],
                |row| row.get::<_, String>(0),
            )
            .map_err(|e| AppError::Database {
                message: e.to_string(),
            })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| AppError::Database {
                message: e.to_string(),
            })?
    };
    for id in ids {
        // This validates the job, linked generation and canonical metadata as
        // one read transaction before its paths can confer authority.
        if let Ok(job) = crate::generation_jobs::get_job(&conn, &id) {
            if persisted_edit_paths_match(&job.request, model, sources, advanced) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

// ── Commands ─────────────────────────────────────────────────────────────────

#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri command parameters form the stable IPC contract.
pub(crate) async fn generate_image(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    engine_state: State<'_, api_gateway::GptImageEngine>,
    prompt: String,
    client_request_id: Option<String>,
    model: Option<String>,
    advanced_options: Option<ImageAdvancedOptions>,
    aspect_ratio: Option<String>,
    resolution: Option<String>,
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
    submit_generation_request(
        &app,
        db.inner(),
        engine_state.inner(),
        GenerationExecutionRequest {
            advanced_options,
            aspect_ratio,
            resolution,
            kind: GenerationLifecycleKind::Generate,
            prompt,
            client_request_id,
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
            execution_mode: GenerationExecutionMode::Compatibility,
        },
    )
    .await
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri command parameters form the stable IPC contract.
pub(crate) async fn edit_image(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    engine_state: State<'_, api_gateway::GptImageEngine>,
    selected_images: State<'_, SelectedImageRegistry>,
    client_request_id: Option<String>,
    prompt: String,
    model: Option<String>,
    source_image_paths: Vec<String>,
    advanced_options: Option<ImageAdvancedOptions>,
    aspect_ratio: Option<String>,
    resolution: Option<String>,
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
    let persisted_retry = persisted_retry_authorizes_edit(
        db.inner(),
        model.as_deref(),
        &source_image_paths,
        advanced_options.as_ref(),
    )?;
    let source_image_paths = if persisted_retry {
        source_image_paths
            .iter()
            .map(|path| {
                let canonical = canonicalize_source_image_path(Path::new(path))?;
                if canonical.to_string_lossy() != *path {
                    return Err(AppError::Validation {
                        message: "Saved source path changed.".into(),
                    });
                }
                validate_source_image_file(&canonical)?;
                Ok(path.clone())
            })
            .collect::<Result<Vec<_>, AppError>>()?
    } else {
        resolve_source_image_paths(
            &app,
            db.inner(),
            selected_images.inner(),
            &source_image_paths,
        )?
    };

    let mut advanced_options = advanced_options;
    if let Some(advanced) = &mut advanced_options {
        for path in [&mut advanced.markup_path, &mut advanced.mask_path]
            .into_iter()
            .flatten()
        {
            if persisted_retry {
                let canonical = canonicalize_source_image_path(Path::new(path))?;
                if canonical.to_string_lossy() != *path {
                    return Err(AppError::Validation {
                        message: "Saved overlay path changed.".into(),
                    });
                }
                validate_source_image_file(&canonical)?;
            } else {
                let resolved = resolve_source_image_paths(
                    &app,
                    db.inner(),
                    selected_images.inner(),
                    std::slice::from_ref(path),
                )?;
                *path = resolved[0].clone();
            }
            let source =
                std::fs::read(&source_image_paths[0]).map_err(|_| AppError::Validation {
                    message: "Cannot read source image.".into(),
                })?;
            let overlay = std::fs::read(&*path).map_err(|_| AppError::Validation {
                message: "Cannot read overlay image.".into(),
            })?;
            validate_edit_overlay(&source, &overlay)?;
        }
    }
    submit_generation_request(
        &app,
        db.inner(),
        engine_state.inner(),
        GenerationExecutionRequest {
            advanced_options,
            aspect_ratio,
            resolution,
            kind: GenerationLifecycleKind::Edit,
            prompt,
            client_request_id,
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
            execution_mode: GenerationExecutionMode::Compatibility,
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
    let img = file_manager::decode_image_for_preview(&data)?;
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
        .add_filter("Image", &["png", "jpg", "jpeg", "webp", "gif", "svg"])
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

#[cfg(test)]
mod image26_overlay_tests {
    use super::*;
    fn png(w: u32, h: u32, alpha: u8) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(w, h, image::Rgba([0, 0, 0, alpha]));
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    }
    #[test]
    fn persisted_retry_does_not_authorize_another_source_or_overlay() {
        let request = serde_json::json!({"model":"gpt-image-2.6-flare","source_image_paths":["/source.png"],"options":{"advanced_options":{"markupPath":"/overlay.png"}}});
        let a: ImageAdvancedOptions =
            serde_json::from_value(serde_json::json!({"markupPath":"/overlay.png"})).unwrap();
        assert!(persisted_edit_paths_match(
            &request,
            "gpt-image-2.6-flare",
            &["/source.png".into()],
            Some(&a)
        ));
        assert!(!persisted_edit_paths_match(
            &request,
            "gpt-image-2.6-flare",
            &["/other.png".into()],
            Some(&a)
        ));
        let mut other = a.clone();
        other.markup_path = Some("/secret.png".into());
        assert!(!persisted_edit_paths_match(
            &request,
            "gpt-image-2.6-flare",
            &["/source.png".into()],
            Some(&other)
        ));
        assert!(!persisted_edit_paths_match(
            &request,
            "gpt-image-2.6-sunburst",
            &["/source.png".into()],
            Some(&a)
        ));
    }
    #[test]
    fn overlay_requires_transparency_and_matching_dimensions() {
        let source = png(8, 8, 255);
        assert!(validate_edit_overlay(&source, &png(8, 8, 0)).is_ok());
        assert!(validate_edit_overlay(&source, &png(8, 8, 255)).is_err());
        assert!(validate_edit_overlay(&source, &png(4, 8, 0)).is_err());
        assert!(validate_edit_overlay(&source, b"not png").is_err());
    }
}
