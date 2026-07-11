use crate::api_gateway::ImageEngine;
use crate::commands::{conversations, settings};
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
use tauri::{Emitter, Manager};

const RECOVERY_STATE_REQUESTING: &str = "requesting";

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum GenerationLifecycleKind {
    Generate,
    Edit,
}

impl GenerationLifecycleKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Generate => "generate",
            Self::Edit => "edit",
        }
    }

    fn endpoint_kind(self) -> ImageEndpointKind {
        match self {
            Self::Generate => ImageEndpointKind::Generate,
            Self::Edit => ImageEndpointKind::Edit,
        }
    }

    fn completed_log_message(self, image_count: usize) -> String {
        match self {
            Self::Generate => format!("Completed — {} image(s) saved", image_count),
            Self::Edit => format!("Edit completed — {} image(s) saved", image_count),
        }
    }

    fn failed_log_message(self, error: &str) -> String {
        match self {
            Self::Generate => format!("Failed: {}", error),
            Self::Edit => format!("Edit failed: {}", error),
        }
    }
}

pub(crate) struct GenerationLifecycleRequest {
    pub(crate) kind: GenerationLifecycleKind,
    pub(crate) prompt: String,
    pub(crate) model: Option<String>,
    pub(crate) source_image_paths: Vec<String>,
    pub(crate) size: Option<String>,
    pub(crate) quality: Option<String>,
    pub(crate) background: Option<String>,
    pub(crate) output_format: Option<String>,
    pub(crate) output_compression: Option<u8>,
    pub(crate) moderation: Option<String>,
    pub(crate) input_fidelity: Option<String>,
    pub(crate) image_count: Option<u8>,
    pub(crate) conversation_id: Option<String>,
    pub(crate) project_id: Option<String>,
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

pub(crate) fn image_request_options(
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

pub(crate) fn source_image_paths_json(source_image_paths: &[String]) -> Result<String, AppError> {
    serde_json::to_string(source_image_paths).map_err(|e| AppError::Database {
        message: format!("Serialize source image paths failed: {}", e),
    })
}

pub(crate) fn generation_request_metadata_json(
    request_kind: GenerationLifecycleKind,
    conversation_id: &str,
    model: &str,
    options: &GptImageRequestOptions,
    source_image_paths: &[String],
) -> Result<String, AppError> {
    serde_json::to_string(&serde_json::json!({
        "request_kind": request_kind.as_str(),
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
    }))
    .map_err(|e| AppError::Database {
        message: format!("Serialize generation metadata failed: {}", e),
    })
}

fn resolve_image_endpoint_url_for_model(
    db: &Database,
    model: &str,
    kind: ImageEndpointKind,
) -> Result<String, AppError> {
    let settings = settings::read_model_endpoint_settings(db, model)?;
    Ok(image_endpoint_url_for_model_settings(
        model, &settings, kind,
    ))
}

fn create_processing_generation(
    conn: &rusqlite::Connection,
    generation_id: &str,
    prompt: &str,
    model: &str,
    options: &GptImageRequestOptions,
    conversation_id: &str,
    created_at: &str,
    request_kind: GenerationLifecycleKind,
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
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::Database {
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
            request_kind.as_str(),
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
            request_kind.as_str(),
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

fn set_generation_failed(
    conn: &rusqlite::Connection,
    generation_id: &str,
    error_message: &str,
    clear_recovery: bool,
) -> Result<(), AppError> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::Database {
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
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::FileSystem {
            message: format!("Get app data dir failed: {}", e),
        })?;
    let fm = file_manager::FileManager::new(app_data_dir);
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::Database {
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
        let saved = fm
            .save_image_at(&img_id, data, output_format, Some(created_at))
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

pub(crate) async fn run_generation_lifecycle(
    app: &tauri::AppHandle,
    db: &Database,
    engine: &crate::api_gateway::GptImageEngine,
    request: GenerationLifecycleRequest,
) -> Result<GenerateResult, AppError> {
    let mut options = image_request_options(
        request.size,
        request.quality,
        request.background,
        request.output_format,
        request.output_compression,
        request.moderation,
        request.input_fidelity,
        request.image_count,
    );
    let generation_id = uuid::Uuid::new_v4().to_string();
    let created_at = current_timestamp();

    let conversation_id = {
        let conn = db.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        conversations::resolve_conversation_id_for_generation(
            &conn,
            request.conversation_id.as_deref(),
            request.project_id.as_deref(),
            &request.prompt,
        )?
    };
    let model = normalize_image_model(
        request
            .model
            .as_deref()
            .or(db.get_setting(SETTING_IMAGE_MODEL)?.as_deref())
            .unwrap_or(DEFAULT_IMAGE_MODEL),
    )
    .to_string();
    options = sanitize_request_options_for_model(&model, options);

    if request.kind == GenerationLifecycleKind::Generate {
        let _ = db.insert_log(
            "generation",
            "info",
            &format!(
                "Started — size: {}, quality: {}, background: {}, output_format: {}, image_count: {}",
                options.size,
                options.quality,
                options.background,
                options.output_format,
                options.image_count
            ),
            Some(&generation_id),
            Some(
                &serde_json::json!({
                    "model": &model,
                    "size": &options.size, "quality": &options.quality,
                    "background": &options.background, "output_format": &options.output_format,
                    "output_compression": options.output_compression, "moderation": &options.moderation,
                    "stream": options.stream, "partial_images": options.partial_images,
                    "image_count": options.image_count, "conversation_id": conversation_id
                })
                .to_string(),
            ),
            None,
        )?;
    }

    let api_key =
        settings::read_model_api_key(db, &model)?.ok_or_else(|| AppError::ApiKeyNotSet {
            model: model.clone(),
        })?;
    let endpoint_url =
        resolve_image_endpoint_url_for_model(db, &model, request.kind.endpoint_kind())?;
    {
        let conn = db.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        create_processing_generation(
            &conn,
            &generation_id,
            &request.prompt,
            &model,
            &options,
            &conversation_id,
            &created_at,
            request.kind,
            &request.source_image_paths,
        )?;
    }

    let _ = app.emit(
        "generation:progress",
        serde_json::json!({ "generation_id": generation_id, "status": "processing" }),
    );

    let result = match request.kind {
        GenerationLifecycleKind::Generate => {
            engine
                .generate(&model, &api_key, &endpoint_url, &request.prompt, &options)
                .await
        }
        GenerationLifecycleKind::Edit => {
            engine
                .edit(
                    &model,
                    &api_key,
                    &endpoint_url,
                    &request.prompt,
                    &request.source_image_paths,
                    &options,
                )
                .await
        }
    };

    match result {
        Ok(engine_response) => {
            if engine_response.requested_image_count != options.image_count {
                return Err(AppError::Validation {
                    message: "Provider attempt request count did not match the persisted request"
                        .to_string(),
                });
            }
            let images = engine
                .decode_images_from_response(&engine_response.body_text)
                .await
                .map_err(|message| AppError::Validation { message })?;

            let saved_images = save_generation_images(
                app,
                db,
                &generation_id,
                &created_at,
                &options.output_format,
                &images,
            )?;

            let _ = db.insert_log(
                "generation",
                "info",
                &request.kind.completed_log_message(saved_images.len()),
                Some(&generation_id),
                Some(&serde_json::json!({"image_count": saved_images.len()}).to_string()),
                None,
            );

            let _ = app.emit(
                "generation:complete",
                serde_json::json!({ "generation_id": generation_id, "status": "completed" }),
            );

            Ok(GenerateResult {
                generation_id,
                conversation_id,
                images: saved_images,
            })
        }
        Err(e) => {
            let message = e.sanitized_message;
            let _ = db.insert_log(
                "generation",
                "error",
                &request.kind.failed_log_message(&message),
                Some(&generation_id),
                None,
                None,
            );

            let conn = db.conn.lock().map_err(|e| AppError::Database {
                message: format!("Lock failed: {}", e),
            })?;
            set_generation_failed(&conn, &generation_id, &message, true)?;

            let _ = app.emit(
                "generation:failed",
                serde_json::json!({ "generation_id": generation_id, "error": &message }),
            );

            Err(AppError::Validation { message })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DEFAULT_IMAGE_COUNT;

    #[test]
    fn lifecycle_request_kind_names_generate_and_edit_for_persistence() {
        assert_eq!(GenerationLifecycleKind::Generate.as_str(), "generate");
        assert_eq!(GenerationLifecycleKind::Edit.as_str(), "edit");
    }

    #[test]
    fn lifecycle_metadata_counts_source_images_without_storing_paths() {
        let options = image_request_options(
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(DEFAULT_IMAGE_COUNT),
        );
        let metadata = generation_request_metadata_json(
            GenerationLifecycleKind::Edit,
            "conversation-1",
            "gpt-image-2",
            &options,
            &["/Users/example/private.png".to_string()],
        )
        .expect("serialize metadata");

        assert!(metadata.contains("\"request_kind\":\"edit\""));
        assert!(metadata.contains("\"source_image_count\":1"));
        assert!(!metadata.contains("private.png"));
        assert!(!metadata.contains("source_image_paths"));
    }
}
