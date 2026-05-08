use crate::api_gateway;
use crate::api_gateway::ImageEngine;
use crate::commands::settings;
use crate::current_timestamp;
use crate::db::Database;
use crate::error::AppError;
use crate::file_manager;
use crate::model_registry::{
    image_endpoint_url_for_model_settings, normalize_image_model,
    sanitize_request_options_for_model, ImageEndpointKind,
};
use crate::models::*;
use rusqlite::params;
use tauri::{Emitter, Manager, State};

const RECOVERY_STATE_REQUESTING: &str = "requesting";
const RECOVERY_STATE_RESPONSE_READY: &str = "response_ready";
const RECOVERY_KIND_GENERATE: &str = "generate";
const RECOVERY_KIND_EDIT: &str = "edit";

fn resolve_image_endpoint_url_for_model(
    db: &Database,
    model: &str,
    kind: ImageEndpointKind,
) -> Result<String, AppError> {
    let settings = settings::read_model_endpoint_settings(db, model)?;
    Ok(image_endpoint_url_for_model_settings(model, &settings, kind))
}

fn normalize_image_moderation(moderation: &str) -> &'static str {
    match moderation {
        "low" => "low",
        _ => DEFAULT_IMAGE_MODERATION,
    }
}

fn normalize_input_fidelity(input_fidelity: &str) -> &'static str {
    match input_fidelity {
        "low" => "low",
        "high" => "high",
        _ => DEFAULT_INPUT_FIDELITY,
    }
}

fn image_request_options(
    size: Option<String>,
    quality: Option<String>,
    background: Option<String>,
    output_format: Option<String>,
    output_compression: Option<u8>,
    moderation: Option<String>,
    input_fidelity: Option<String>,
    image_count: Option<u8>,
) -> GptImageRequestOptions {
    GptImageRequestOptions {
        size: size.unwrap_or_else(|| DEFAULT_IMAGE_SIZE.to_string()),
        quality: quality.unwrap_or_else(|| DEFAULT_IMAGE_QUALITY.to_string()),
        background: background.unwrap_or_else(|| DEFAULT_IMAGE_BACKGROUND.to_string()),
        output_format: output_format.unwrap_or_else(|| DEFAULT_OUTPUT_FORMAT.to_string()),
        output_compression: output_compression
            .unwrap_or(DEFAULT_OUTPUT_COMPRESSION)
            .min(100),
        moderation: normalize_image_moderation(
            moderation.as_deref().unwrap_or(DEFAULT_IMAGE_MODERATION),
        )
        .to_string(),
        input_fidelity: normalize_input_fidelity(
            input_fidelity.as_deref().unwrap_or(DEFAULT_INPUT_FIDELITY),
        )
        .to_string(),
        stream: DEFAULT_IMAGE_STREAM,
        partial_images: DEFAULT_PARTIAL_IMAGES,
        image_count: image_count.unwrap_or(DEFAULT_IMAGE_COUNT).clamp(1, 4),
    }
}

fn source_image_paths_json(source_image_paths: &[String]) -> Result<String, AppError> {
    serde_json::to_string(source_image_paths)
        .map_err(|e| AppError::Database {
            message: format!("Serialize source image paths failed: {}", e),
        })
}

fn generation_request_metadata_json(
    request_kind: &str,
    conversation_id: &str,
    model: &str,
    options: &GptImageRequestOptions,
    source_image_paths: &[String],
) -> Result<String, AppError> {
    serde_json::to_string(&serde_json::json!({
        "request_kind": request_kind,
        "conversation_id": conversation_id,
        "model": model,
        "size": &options.size,
        "quality": &options.quality,
        "background": &options.background,
        "output_format": &options.output_format,
        "output_compression": options.output_compression,
        "moderation": &options.moderation,
        "input_fidelity": &options.input_fidelity,
        "stream": options.stream,
        "partial_images": options.partial_images,
        "image_count": options.image_count,
        "source_image_count": source_image_paths.len(),
        "source_image_paths": source_image_paths,
    }))
    .map_err(|e| AppError::Database {
        message: format!("Serialize generation metadata failed: {}", e),
    })
}

fn create_processing_generation(
    conn: &rusqlite::Connection,
    generation_id: &str,
    prompt: &str,
    model: &str,
    options: &GptImageRequestOptions,
    conversation_id: &str,
    created_at: &str,
    request_kind: &str,
    source_image_paths: &[String],
) -> Result<(), AppError> {
    let source_image_paths_json = source_image_paths_json(source_image_paths)?;
    let request_metadata = generation_request_metadata_json(
        request_kind,
        conversation_id,
        model,
        options,
        source_image_paths,
    )?;
    let tx = conn.unchecked_transaction().map_err(|e| AppError::Database {
        message: format!("Begin transaction failed: {}", e),
    })?;
    tx.execute(
        "INSERT INTO generations (
            id, prompt, engine, request_kind, size, quality, background, output_format,
            output_compression, moderation, input_fidelity, image_count, source_image_count,
            source_image_paths, request_metadata, status, error_message, conversation_id, created_at
         ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
            'processing', NULL, ?16, ?17
         )",
        params![
            generation_id,
            prompt,
            model,
            request_kind,
            &options.size,
            &options.quality,
            &options.background,
            &options.output_format,
            options.output_compression,
            &options.moderation,
            &options.input_fidelity,
            options.image_count,
            source_image_paths.len() as i64,
            source_image_paths_json,
            request_metadata,
            conversation_id,
            created_at
        ],
    )
    .map_err(|e| AppError::Database {
        message: format!("Insert processing generation failed: {}", e),
    })?;
    tx.execute(
        "INSERT INTO generation_recoveries (generation_id, request_kind, request_state, output_format, response_file, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?5)",
        params![
            generation_id,
            request_kind,
            RECOVERY_STATE_REQUESTING,
            &options.output_format,
            created_at
        ],
    )
    .map_err(|e| AppError::Database {
        message: format!("Insert generation recovery failed: {}", e),
    })?;
    tx.commit().map_err(|e| AppError::Database {
        message: format!("Commit transaction failed: {}", e),
    })
}

fn update_generation_recovery_response(
    conn: &rusqlite::Connection,
    generation_id: &str,
    response_file: &str,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE generation_recoveries SET request_state = ?1, response_file = ?2, updated_at = ?3 WHERE generation_id = ?4",
        params![
            RECOVERY_STATE_RESPONSE_READY,
            response_file,
            current_timestamp(),
            generation_id
        ],
    )
    .map_err(|e| AppError::Database {
        message: format!("Update generation recovery failed: {}", e),
    })?;
    Ok(())
}

fn set_generation_failed(
    conn: &rusqlite::Connection,
    generation_id: &str,
    error_message: &str,
    clear_recovery: bool,
) -> Result<(), AppError> {
    let tx = conn.unchecked_transaction().map_err(|e| AppError::Database {
        message: format!("Begin transaction failed: {}", e),
    })?;
    tx.execute(
        "UPDATE generations SET status = 'failed', error_message = ?1 WHERE id = ?2",
        params![error_message, generation_id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Update generation status failed: {}", e),
    })?;
    if clear_recovery {
        tx.execute(
            "DELETE FROM generation_recoveries WHERE generation_id = ?1",
            params![generation_id],
        )
        .map_err(|e| AppError::Database {
            message: format!("Clear generation recovery failed: {}", e),
        })?;
    }
    tx.commit().map_err(|e| AppError::Database {
        message: format!("Commit transaction failed: {}", e),
    })
}

fn save_generation_images(
    app: &tauri::AppHandle,
    db: &Database,
    generation_id: &str,
    created_at: &str,
    output_format: &str,
    images_data: &[Vec<u8>],
) -> Result<Vec<GeneratedImage>, AppError> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| AppError::FileSystem {
        message: format!("Get app data dir failed: {}", e),
    })?;
    let fm = file_manager::FileManager::new(app_data_dir);
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let tx = conn.unchecked_transaction().map_err(|e| AppError::Database {
        message: format!("Begin transaction failed: {}", e),
    })?;

    tx.execute(
        "DELETE FROM images WHERE generation_id = ?1",
        params![generation_id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Clear existing images failed: {}", e),
    })?;

    let mut saved_images = Vec::with_capacity(images_data.len());
    for (i, data) in images_data.iter().enumerate() {
        let img_id = format!("{}_{}", generation_id, i);
        let saved = fm.save_image_at(&img_id, data, output_format, Some(created_at))
            .map_err(|e| AppError::FileSystem { message: e })?;

        tx.execute(
            "INSERT INTO images (id, generation_id, file_path, thumbnail_path, width, height, file_size, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![img_id, generation_id, saved.file_path, saved.thumbnail_path, saved.width, saved.height, saved.file_size, created_at],
        ).map_err(|e| AppError::Database {
            message: format!("Insert image record failed: {}", e),
        })?;

        saved_images.push(GeneratedImage {
            id: img_id,
            generation_id: generation_id.to_string(),
            file_path: saved.file_path,
            thumbnail_path: saved.thumbnail_path,
            width: saved.width,
            height: saved.height,
            file_size: saved.file_size,
        });
    }

    tx.execute(
        "UPDATE generations SET status = 'completed', error_message = NULL WHERE id = ?1",
        params![generation_id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Update generation status failed: {}", e),
    })?;
    tx.execute(
        "DELETE FROM generation_recoveries WHERE generation_id = ?1",
        params![generation_id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Clear generation recovery failed: {}", e),
    })?;
    tx.commit().map_err(|e| AppError::Database {
        message: format!("Commit transaction failed: {}", e),
    })?;

    Ok(saved_images)
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
    let mut options = image_request_options(
        size, quality, background, output_format, output_compression, moderation, None, image_count,
    );
    let generation_id = uuid::Uuid::new_v4().to_string();
    let created_at = current_timestamp();

    let conversation_id = {
        let conn = db.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        crate::commands::conversations::resolve_conversation_id_for_generation(
            &conn, conversation_id.as_deref(), project_id.as_deref(), &prompt,
        )?
    };
    let model = normalize_image_model(
        model
            .as_deref()
            .or(db.get_setting(SETTING_IMAGE_MODEL)?.as_deref())
            .unwrap_or(DEFAULT_IMAGE_MODEL),
    )
    .to_string();
    options = sanitize_request_options_for_model(&model, options);

    let _ = db.insert_log(
        "generation", "info",
        &format!(
            "Started — prompt: {:?}, size: {}, quality: {}, background: {}, output_format: {}, image_count: {}",
            prompt, options.size, options.quality, options.background, options.output_format, options.image_count
        ),
        Some(&generation_id),
        Some(&serde_json::json!({
            "model": &model, "prompt": prompt,
            "size": &options.size, "quality": &options.quality,
            "background": &options.background, "output_format": &options.output_format,
            "output_compression": options.output_compression, "moderation": &options.moderation,
            "stream": options.stream, "partial_images": options.partial_images,
            "image_count": options.image_count, "conversation_id": conversation_id
        }).to_string()),
        None,
    )?;
    let api_key = settings::read_model_api_key(db.inner(), &model)?.ok_or_else(|| {
        AppError::ApiKeyNotSet {
            model: model.clone(),
        }
    })?;
    let endpoint_url =
        resolve_image_endpoint_url_for_model(db.inner(), &model, ImageEndpointKind::Generate)?;
    let app_data_dir = app.path().app_data_dir().map_err(|e| AppError::FileSystem {
        message: format!("Get app data dir failed: {}", e),
    })?;

    {
        let conn = db.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        create_processing_generation(
            &conn, &generation_id, &prompt, &model, &options,
            &conversation_id, &created_at, RECOVERY_KIND_GENERATE, &[],
        )?;
    }

    let _ = app.emit(
        "generation:progress",
        serde_json::json!({ "generation_id": generation_id, "status": "processing" }),
    );

    let result = engine_state
        .generate(
            &generation_id, &model, &api_key, &endpoint_url, &prompt, &options,
            Some(&db), Some(&app_data_dir),
        )
        .await;

    match result {
        Ok(engine_response) => {
            if let Some(response_file) = engine_response.response_file.as_deref() {
                let conn = db.conn.lock().map_err(|e| AppError::Database {
                    message: format!("Lock failed: {}", e),
                })?;
                update_generation_recovery_response(&conn, &generation_id, response_file)?;
            }

            let saved_images = save_generation_images(
                &app, db.inner(), &generation_id, &created_at,
                &options.output_format, &engine_response.images,
            )?;

            let _ = db.insert_log(
                "generation", "info",
                &format!("Completed — {} image(s) saved", saved_images.len()),
                Some(&generation_id),
                Some(&serde_json::json!({"image_count": saved_images.len()}).to_string()),
                None,
            );

            let _ = app.emit(
                "generation:complete",
                serde_json::json!({ "generation_id": generation_id, "status": "completed" }),
            );

            Ok(GenerateResult { generation_id, conversation_id, images: saved_images })
        }
        Err(e) => {
            let _ = db.insert_log(
                "generation", "error",
                &format!("Failed: {}", &e), Some(&generation_id), None, None,
            );

            let conn = db.conn.lock().map_err(|e| AppError::Database {
                message: format!("Lock failed: {}", e),
            })?;
            set_generation_failed(&conn, &generation_id, &e, true)?;

            let _ = app.emit(
                "generation:failed",
                serde_json::json!({ "generation_id": generation_id, "error": &e }),
            );

            Err(AppError::Validation { message: e })
        }
    }
}

#[tauri::command]
pub(crate) async fn edit_image(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    engine_state: State<'_, api_gateway::GptImageEngine>,
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

    let mut options = image_request_options(
        size, quality, background, output_format, output_compression, moderation, input_fidelity,
        image_count,
    );
    let generation_id = uuid::Uuid::new_v4().to_string();
    let created_at = current_timestamp();

    let conversation_id = {
        let conn = db.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        crate::commands::conversations::resolve_conversation_id_for_generation(
            &conn, conversation_id.as_deref(), project_id.as_deref(), &prompt,
        )?
    };
    let model = normalize_image_model(
        model
            .as_deref()
            .or(db.get_setting(SETTING_IMAGE_MODEL)?.as_deref())
            .unwrap_or(DEFAULT_IMAGE_MODEL),
    )
    .to_string();
    options = sanitize_request_options_for_model(&model, options);

    let api_key = settings::read_model_api_key(db.inner(), &model)?.ok_or_else(|| {
        AppError::ApiKeyNotSet {
            model: model.clone(),
        }
    })?;
    let endpoint_url =
        resolve_image_endpoint_url_for_model(db.inner(), &model, ImageEndpointKind::Edit)?;
    let app_data_dir = app.path().app_data_dir().map_err(|e| AppError::FileSystem {
        message: format!("Get app data dir failed: {}", e),
    })?;

    {
        let conn = db.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        create_processing_generation(
            &conn, &generation_id, &prompt, &model, &options,
            &conversation_id, &created_at, RECOVERY_KIND_EDIT, &source_image_paths,
        )?;
    }

    let _ = app.emit(
        "generation:progress",
        serde_json::json!({ "generation_id": generation_id, "status": "processing" }),
    );

    let result = engine_state
        .edit(
            &generation_id, &model, &api_key, &endpoint_url, &prompt,
            &source_image_paths, &options, Some(&db), Some(&app_data_dir),
        )
        .await;

    match result {
        Ok(engine_response) => {
            if let Some(response_file) = engine_response.response_file.as_deref() {
                let conn = db.conn.lock().map_err(|e| AppError::Database {
                    message: format!("Lock failed: {}", e),
                })?;
                update_generation_recovery_response(&conn, &generation_id, response_file)?;
            }

            let saved_images = save_generation_images(
                &app, db.inner(), &generation_id, &created_at,
                &options.output_format, &engine_response.images,
            )?;

            let _ = db.insert_log(
                "generation", "info",
                &format!("Edit completed — {} image(s) saved", saved_images.len()),
                Some(&generation_id),
                Some(&serde_json::json!({"image_count": saved_images.len()}).to_string()),
                None,
            );

            let _ = app.emit(
                "generation:complete",
                serde_json::json!({ "generation_id": generation_id, "status": "completed" }),
            );

            Ok(GenerateResult { generation_id, conversation_id, images: saved_images })
        }
        Err(e) => {
            let _ = db.insert_log(
                "generation", "error",
                &format!("Edit failed: {}", &e), Some(&generation_id), None, None,
            );

            let conn = db.conn.lock().map_err(|e| AppError::Database {
                message: format!("Lock failed: {}", e),
            })?;
            set_generation_failed(&conn, &generation_id, &e, true)?;

            let _ = app.emit(
                "generation:failed",
                serde_json::json!({ "generation_id": generation_id, "error": &e }),
            );

            Err(AppError::Validation { message: e })
        }
    }
}

// ── Lightbox commands ────────────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn copy_image_to_clipboard(image_path: String) -> Result<(), AppError> {
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
pub(crate) async fn save_image_to_file(image_path: String) -> Result<(), AppError> {
    let file_name = std::path::Path::new(&image_path)
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
pub(crate) async fn pick_source_images() -> Result<Vec<String>, AppError> {
    let files = rfd::AsyncFileDialog::new()
        .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
        .pick_files()
        .await;

    let Some(files) = files else {
        return Ok(vec![]);
    };

    Ok(files
        .into_iter()
        .map(|file| file.path().to_string_lossy().to_string())
        .collect())
}
