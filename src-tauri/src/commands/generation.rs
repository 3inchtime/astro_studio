use crate::db::Database;
use crate::error::AppError;
use crate::file_manager;
use crate::generation_execution::GenerationExecutionRequest;
use crate::generation_lifecycle::GenerationLifecycleKind;
use crate::models::*;
use crate::source_images::{
    canonicalize_source_image_path, resolve_source_image_paths, validate_edit_overlay,
    validate_managed_image_path, validate_source_image_file, SelectedImageRegistry,
};
use std::path::{Path, PathBuf};
use tauri::State;

// Restart retries may reuse only the exact source list and overlay pair from a
// validated failed job. Picker registration still governs every other request.
pub(crate) fn persisted_edit_paths_match(
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
    let Some(model) = model
        .and_then(crate::model_registry::image_model)
        .filter(|m| m.profile.advanced_options)
        .map(|m| m.id)
    else {
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

// A reviewed saved recipe grants access only to its exact saved files. New
// references still require picker or managed-image authorization.
fn saved_recipe_authorizes_path(
    request: &serde_json::Value,
    model: &str,
    path: &str,
    role: &str,
) -> bool {
    if request["model"].as_str() != Some(model) {
        return false;
    }
    match role {
        "source" => request["source_image_paths"]
            .as_array()
            .is_some_and(|paths| paths.iter().any(|p| p.as_str() == Some(path))),
        "markupPath" | "maskPath" => {
            request["options"]["advanced_options"][role].as_str() == Some(path)
        }
        _ => false,
    }
}

// ── Commands ─────────────────────────────────────────────────────────────────

fn validate_edit_sources(
    app: &tauri::AppHandle,
    db: &Database,
    selected_images: &SelectedImageRegistry,
    model: Option<&str>,
    source_image_paths: &[String],
    advanced_options: Option<ImageAdvancedOptions>,
    source_generation_id: Option<&str>,
) -> Result<(Vec<String>, Option<ImageAdvancedOptions>), AppError> {
    if source_image_paths.is_empty() {
        return Err(AppError::Validation {
            message: "Please select at least one source image.".to_string(),
        });
    }
    let saved_recipe = source_generation_id
        .map(|id| {
            crate::generation_recipe::saved_job(db, id)?.ok_or_else(|| AppError::Validation {
                message:
                    "Saved source authorization is unavailable; select the source images again."
                        .into(),
            })
        })
        .transpose()?;
    let persisted_retry = saved_recipe.is_none()
        && persisted_retry_authorizes_edit(
            db,
            model,
            source_image_paths,
            advanced_options.as_ref(),
        )?;
    let resolve_path = |path: &String, role: &str| -> Result<String, AppError> {
        let saved = saved_recipe.as_ref().is_some_and(|job| {
            saved_recipe_authorizes_path(&job.request, model.unwrap_or_default(), path, role)
        });
        if persisted_retry || saved {
            let canonical = canonicalize_source_image_path(Path::new(path))?;
            if canonical.to_string_lossy() != *path {
                return Err(AppError::Validation {
                    message: "Saved source path changed.".into(),
                });
            }
            validate_source_image_file(&canonical)?;
            Ok(path.clone())
        } else {
            Ok(
                resolve_source_image_paths(app, db, selected_images, std::slice::from_ref(path))?
                    [0]
                .clone(),
            )
        }
    };
    let source_image_paths = source_image_paths
        .iter()
        .map(|path| resolve_path(path, "source"))
        .collect::<Result<Vec<_>, _>>()?;

    let mut advanced_options = advanced_options;
    if let Some(advanced) = &mut advanced_options {
        for (role, value) in [
            ("markupPath", &mut advanced.markup_path),
            ("maskPath", &mut advanced.mask_path),
        ] {
            let Some(path) = value else {
                continue;
            };
            *path = resolve_path(path, role)?;
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
    Ok((source_image_paths, advanced_options))
}

#[tauri::command]
pub(crate) async fn enqueue_generation(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    selected_images: State<'_, SelectedImageRegistry>,
    request: GenerationExecutionRequest,
) -> Result<EnqueueGenerationResult, AppError> {
    crate::generation_execution::enqueue_request(&app, &db, request, |request| {
        if request.kind == GenerationLifecycleKind::Edit {
            (request.source_image_paths, request.advanced_options) = validate_edit_sources(
                &app,
                db.inner(),
                selected_images.inner(),
                request.model.as_deref(),
                &request.source_image_paths,
                request.advanced_options.take(),
                request.source_generation_id.as_deref(),
            )?;
        } else if !request.source_image_paths.is_empty() {
            return Err(AppError::Validation {
                message: "Generate requests cannot contain edit sources.".into(),
            });
        }
        Ok(())
    })
}

// ── Lightbox commands ────────────────────────────────────────────────────────

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
    fn edited_recipe_authority_is_per_saved_path_and_overlay_role() {
        let request = serde_json::json!({"model":"gpt-image-2.6-flare","source_image_paths":["/one.png","/two.png"],"options":{"advanced_options":{"markupPath":"/overlay.png"}}});
        assert!(saved_recipe_authorizes_path(
            &request,
            "gpt-image-2.6-flare",
            "/two.png",
            "source"
        ));
        assert!(!saved_recipe_authorizes_path(
            &request,
            "gpt-image-2.6-flare",
            "/arbitrary.png",
            "source"
        ));
        assert!(saved_recipe_authorizes_path(
            &request,
            "gpt-image-2.6-flare",
            "/overlay.png",
            "markupPath"
        ));
        assert!(!saved_recipe_authorizes_path(
            &request,
            "gpt-image-2.6-flare",
            "/overlay.png",
            "maskPath"
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
