use crate::db::Database;
use crate::error::AppError;
use crate::llm::{self, ImageData, MULTIMODAL_TIMEOUT_SECS};
use crate::models::{LlmConfig, PromptExtraction, SETTING_LLM_CONFIGS};
use crate::current_timestamp;
use rusqlite::params;
use tauri::State;

const MAX_ENABLED_TEXT_CONFIGS: usize = 1;
const MAX_ENABLED_MULTIMODAL_CONFIGS: usize = 2;

const OPTIMIZE_PROMPT_SYSTEM_PROMPT: &str = "\
You are an expert at writing prompts for AI image generation models. \
When the user provides a prompt, improve it by following these rules:\n\
1. Add specific details about composition, lighting, color, mood, and style\n\
2. Use descriptive, visual language — paint a picture with words\n\
3. Specify image quality keywords where appropriate (e.g., high resolution, detailed, photorealistic)\n\
4. Keep the original intent and subject matter intact\n\
5. Preserve the language of the user's input — output in the same language the user used\n\
6. Output ONLY the improved prompt text, no explanations or meta-commentary\n\
7. Keep the output concise — at most 3-4 sentences unless the original prompt is very detailed";

const OPTIMIZE_PROMPT_WITH_IMAGES_SYSTEM_PROMPT: &str = "\
You are an expert at writing prompts for AI image generation models. \
The user has provided one or more reference images along with their prompt. \
Analyze the images carefully — note the subject, composition, lighting, color palette, \
mood, style, textures, and any notable details. Then improve the user's prompt by:\n\
1. Describing visual elements from the reference images that should be preserved or enhanced\n\
2. Adding specific details about composition, lighting, color, mood, and style inspired by the images\n\
3. Using descriptive, visual language that captures the essence of the reference images\n\
4. Specifying image quality keywords where appropriate (e.g., high resolution, detailed, photorealistic)\n\
5. Keeping the user's original intent intact while leveraging visual context from the images\n\
6. Preserving the language of the user's input — output in the same language the user used\n\
7. Output ONLY the improved prompt text, no explanations or meta-commentary\n\
8. Keeping the output concise — at most 3-4 sentences unless the original prompt is very detailed";

const EXTRACT_PROMPT_FROM_IMAGE_SYSTEM_PROMPT: &str = "\
You are an expert at reverse-engineering prompts for AI image generation models from reference images. \
Analyze the image carefully and write a ready-to-use image generation prompt that captures the subject, composition, lighting, color palette, mood, style, textures, and camera feel.\n\
1. Prioritize clear visual description over abstract commentary\n\
2. Include composition, lighting, palette, style, and material details when they are visible\n\
3. Keep the prompt concise but production-ready — usually 2-4 sentences\n\
4. Output ONLY the prompt text, with no explanation, labels, or bullet points";

fn describe_output_language(language: &str) -> &'static str {
    match language.trim().to_ascii_lowercase().as_str() {
        "zh-cn" => "Simplified Chinese",
        "zh-tw" | "zh-hk" | "zh-mo" => "Traditional Chinese",
        "ja" => "Japanese",
        "ko" => "Korean",
        "es" => "Spanish",
        "fr" => "French",
        "de" => "German",
        _ => "English",
    }
}

fn build_extract_prompt_from_image_system_prompt(language: &str) -> String {
    format!(
        "{EXTRACT_PROMPT_FROM_IMAGE_SYSTEM_PROMPT}\n5. Output the prompt in the user's interface language: {} ({language})",
        describe_output_language(language),
    )
}

fn read_llm_configs(db: &Database) -> Result<Vec<LlmConfig>, AppError> {
    match db.get_setting(SETTING_LLM_CONFIGS)? {
        Some(json) => serde_json::from_str(&json).map_err(|e| AppError::Database {
            message: format!("Failed to deserialize LLM configs: {}", e),
        }),
        None => Ok(Vec::new()),
    }
}

fn write_llm_configs(db: &Database, configs: &[LlmConfig]) -> Result<(), AppError> {
    let json = serde_json::to_string(configs).map_err(|e| AppError::Database {
        message: format!("Failed to serialize LLM configs: {}", e),
    })?;
    db.set_setting(SETTING_LLM_CONFIGS, &json)
}

fn validate_and_store_llm_configs(db: &Database, configs: Vec<LlmConfig>) -> Result<(), AppError> {
    for config in &configs {
        if config.id.trim().is_empty() {
            return Err(AppError::Validation {
                message: "LLM config 'id' must not be empty".to_string(),
            });
        }
        if config.name.trim().is_empty() {
            return Err(AppError::Validation {
                message: "LLM config 'name' must not be empty".to_string(),
            });
        }
        if config.protocol != "openai" && config.protocol != "anthropic" {
            return Err(AppError::Validation {
                message: format!(
                    "LLM config protocol must be 'openai' or 'anthropic', got '{}'",
                    config.protocol
                ),
            });
        }
        if config.capability != "text" && config.capability != "multimodal" {
            return Err(AppError::Validation {
                message: format!(
                    "LLM config capability must be 'text' or 'multimodal', got '{}'",
                    config.capability
                ),
            });
        }
    }

    validate_llm_enabled_limits(&configs)?;
    let normalized = normalize_llm_enabled_state(&configs);
    validate_llm_enabled_limits(&normalized)?;
    write_llm_configs(db, &normalized)
}

fn validate_llm_enabled_limits(configs: &[LlmConfig]) -> Result<(), AppError> {
    let enabled_total = configs.iter().filter(|config| config.enabled).count();
    if enabled_total > 2 {
        return Err(AppError::Validation {
            message: "Only 2 LLM configs can be enabled at once.".to_string(),
        });
    }

    let text_enabled = configs
        .iter()
        .filter(|config| config.enabled && config.capability == "text")
        .count();
    if text_enabled > MAX_ENABLED_TEXT_CONFIGS {
        return Err(AppError::Validation {
            message: format!(
                "Only {} text LLM config can be enabled at once.",
                MAX_ENABLED_TEXT_CONFIGS
            ),
        });
    }

    let multimodal_enabled = configs
        .iter()
        .filter(|config| config.enabled && config.capability == "multimodal")
        .count();
    if multimodal_enabled > MAX_ENABLED_MULTIMODAL_CONFIGS {
        return Err(AppError::Validation {
            message: format!(
                "Only {} multimodal LLM configs can be enabled at once.",
                MAX_ENABLED_MULTIMODAL_CONFIGS
            ),
        });
    }

    Ok(())
}

fn normalize_llm_enabled_state(configs: &[LlmConfig]) -> Vec<LlmConfig> {
    let mut text_enabled = 0usize;
    let mut multimodal_enabled = 0usize;
    let mut total_enabled = 0usize;

    configs
        .iter()
        .cloned()
        .map(|mut config| {
            if !config.enabled {
                return config;
            }

            if total_enabled >= 2 {
                config.enabled = false;
                return config;
            }

            if config.capability == "text" {
                if text_enabled >= MAX_ENABLED_TEXT_CONFIGS {
                    config.enabled = false;
                } else {
                    text_enabled += 1;
                    total_enabled += 1;
                }
                return config;
            }

            if config.capability == "multimodal" {
                if multimodal_enabled >= MAX_ENABLED_MULTIMODAL_CONFIGS {
                    config.enabled = false;
                } else {
                    multimodal_enabled += 1;
                    total_enabled += 1;
                }
            }

            config
        })
        .collect()
}

fn load_images(paths: &[String]) -> Result<Vec<ImageData>, AppError> {
    let mut images = Vec::new();
    for path in paths {
        let data = std::fs::read(path).map_err(|e| AppError::Validation {
            message: format!("Failed to read image '{}': {}", path, e),
        })?;
        let media_type = match path.rsplit('.').next().unwrap_or("") {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "webp" => "image/webp",
            ext => {
                return Err(AppError::Validation {
                    message: format!(
                        "Unsupported image format '{}'. Supported: jpg, png, webp",
                        ext
                    ),
                })
            }
        };
        if data.len() > 10 * 1024 * 1024 {
            return Err(AppError::Validation {
                message: format!("Image '{}' exceeds 10MB limit", path),
            });
        }
        images.push(ImageData {
            data,
            media_type: media_type.to_string(),
        });
    }
    Ok(images)
}

fn create_multimodal_llm_client(
    config: &LlmConfig,
) -> Result<Box<dyn llm::LlmClient>, AppError> {
    match config.protocol.as_str() {
        "openai" => {
            let client = llm::openai::OpenAiLlmClient::with_timeout(config, MULTIMODAL_TIMEOUT_SECS)?;
            Ok(Box::new(client))
        }
        "anthropic" => {
            let client = llm::anthropic::AnthropicLlmClient::with_timeout(config, MULTIMODAL_TIMEOUT_SECS)?;
            Ok(Box::new(client))
        }
        other => Err(AppError::Validation {
            message: format!("Unknown LLM protocol: {}", other),
        }),
    }
}

fn resolve_multimodal_config<'a>(
    configs: &'a [LlmConfig],
    config_id: &str,
) -> Result<&'a LlmConfig, AppError> {
    configs
        .iter()
        .find(|c| c.id == config_id && c.capability == "multimodal" && c.enabled)
        .or_else(|| {
            configs
                .iter()
                .find(|c| c.enabled && c.capability == "multimodal")
        })
        .ok_or_else(|| AppError::Validation {
            message:
                "No enabled multimodal LLM config found. Please configure a multimodal LLM in settings."
                    .to_string(),
        })
}

fn insert_prompt_extraction(
    db: &Database,
    image_path: &str,
    prompt: &str,
    llm_config_id: &str,
) -> Result<PromptExtraction, AppError> {
    let image_path = image_path.trim();
    let prompt = prompt.trim();
    let llm_config_id = llm_config_id.trim();

    if image_path.is_empty() {
        return Err(AppError::Validation {
            message: "Image path cannot be empty".to_string(),
        });
    }

    if prompt.is_empty() {
        return Err(AppError::Validation {
            message: "Extracted prompt cannot be empty".to_string(),
        });
    }

    if llm_config_id.is_empty() {
        return Err(AppError::Validation {
            message: "LLM config id cannot be empty".to_string(),
        });
    }

    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = current_timestamp();
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;

    conn.execute(
        "INSERT INTO prompt_extractions (id, image_path, prompt, llm_config_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![&id, &image_path, &prompt, &llm_config_id, &timestamp],
    )
    .map_err(|e| AppError::Database {
        message: format!("Insert prompt extraction failed: {}", e),
    })?;

    Ok(PromptExtraction {
        id,
        image_path: image_path.to_string(),
        prompt: prompt.to_string(),
        llm_config_id: llm_config_id.to_string(),
        created_at: timestamp.clone(),
        updated_at: timestamp,
    })
}

fn list_prompt_extractions(
    db: &Database,
    limit: u32,
) -> Result<Vec<PromptExtraction>, AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let limit = limit.clamp(1, 100) as i64;
    let mut stmt = conn
        .prepare(
            "SELECT id, image_path, prompt, llm_config_id, created_at, updated_at
             FROM prompt_extractions
             ORDER BY created_at DESC, updated_at DESC
             LIMIT ?1",
        )
        .map_err(|e| AppError::Database {
            message: format!("Prepare prompt extraction history query failed: {}", e),
        })?;

    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(PromptExtraction {
                id: row.get(0)?,
                image_path: row.get(1)?,
                prompt: row.get(2)?,
                llm_config_id: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(|e| AppError::Database {
            message: format!("Query prompt extraction history failed: {}", e),
        })?;

    Ok(rows.filter_map(|row| row.ok()).collect())
}

#[tauri::command]
pub(crate) fn get_llm_configs(
    db: State<'_, Database>,
) -> Result<Vec<LlmConfig>, AppError> {
    read_llm_configs(db.inner())
}

#[tauri::command]
pub(crate) fn save_llm_configs(
    db: State<'_, Database>,
    configs: Vec<LlmConfig>,
) -> Result<(), AppError> {
    validate_and_store_llm_configs(db.inner(), configs)
}

#[tauri::command]
pub(crate) fn get_prompt_extractions(
    db: State<'_, Database>,
    limit: Option<u32>,
) -> Result<Vec<PromptExtraction>, AppError> {
    list_prompt_extractions(db.inner(), limit.unwrap_or(20))
}

#[tauri::command]
pub(crate) async fn optimize_prompt(
    db: State<'_, Database>,
    prompt: String,
    config_id: String,
    image_paths: Option<Vec<String>>,
) -> Result<String, AppError> {
    let configs = read_llm_configs(db.inner())?;
    let has_images = image_paths.as_ref().map_or(false, |p| !p.is_empty());

    let config = if has_images {
        resolve_multimodal_config(&configs, &config_id)?
    } else {
        let config = configs.iter().find(|c| c.id == config_id).ok_or_else(|| {
            AppError::Validation {
                message: format!("LLM config not found: {}", config_id),
            }
        })?;

        if config.capability != "text" {
            return Err(AppError::Validation {
                message: format!(
                    "LLM config '{}' has capability '{}' — only text models are supported for prompt optimization without images",
                    config.name, config.capability
                ),
            });
        }

        if !config.enabled {
            return Err(AppError::Validation {
                message: format!("LLM config '{}' is disabled", config.name),
            });
        }

        config
    };

    log::info!(
        "Optimizing prompt with LLM config '{}' (protocol: {}, model: {}, images: {})",
        config.name,
        config.protocol,
        config.model,
        has_images
    );

    if has_images {
        let paths = image_paths.unwrap();
        if paths.len() > 3 {
            return Err(AppError::Validation {
                message: "Maximum 3 images supported for multimodal optimization".to_string(),
            });
        }
        let images = load_images(&paths)?;
        let client = create_multimodal_llm_client(config)?;
        let result = client
            .chat_with_images(OPTIMIZE_PROMPT_WITH_IMAGES_SYSTEM_PROMPT, &prompt, &images)
            .await;
        match &result {
            Ok(optimized) => {
                log::info!(
                    "Multimodal prompt optimization succeeded — original length: {}, optimized length: {}, images: {}",
                    prompt.len(),
                    optimized.len(),
                    images.len()
                );
            }
            Err(e) => {
                log::error!("Multimodal prompt optimization failed: {}", e);
            }
        }
        result
    } else {
        let client = llm::create_llm_client(config)?;
        let result = client.chat(OPTIMIZE_PROMPT_SYSTEM_PROMPT, &prompt).await;
        match &result {
            Ok(optimized) => {
                log::info!(
                    "Prompt optimization succeeded — original length: {}, optimized length: {}",
                    prompt.len(),
                    optimized.len()
                );
            }
            Err(e) => {
                log::error!("Prompt optimization failed: {}", e);
            }
        }
        result
    }
}

#[tauri::command]
pub(crate) async fn extract_prompt_from_image(
    db: State<'_, Database>,
    image_path: String,
    config_id: String,
    language: String,
) -> Result<PromptExtraction, AppError> {
    let image_path = image_path.trim().to_string();
    if image_path.is_empty() {
        return Err(AppError::Validation {
            message: "Image path cannot be empty".to_string(),
        });
    }

    let configs = read_llm_configs(db.inner())?;
    let config = resolve_multimodal_config(&configs, &config_id)?;
    let images = load_images(&[image_path.clone()])?;
    let client = create_multimodal_llm_client(config)?;

    log::info!(
        "Extracting prompt from image with LLM config '{}' (protocol: {}, model: {})",
        config.name,
        config.protocol,
        config.model,
    );

    let result = client
        .chat_with_images(
            &build_extract_prompt_from_image_system_prompt(&language),
            "Extract a ready-to-use image generation prompt for this image and keep the output language aligned with the user's interface language.",
            &images,
        )
        .await;

    match &result {
        Ok(prompt) => {
            log::info!(
                "Image prompt extraction succeeded — image: {}, prompt length: {}",
                image_path,
                prompt.len(),
            );
        }
        Err(error) => {
            log::error!("Image prompt extraction failed: {}", error);
        }
    }

    let prompt = result?;
    insert_prompt_extraction(db.inner(), &image_path, &prompt, &config.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::LlmConfig;

    fn temp_test_db(prefix: &str) -> (Database, std::path::PathBuf) {
        let db_path =
            std::env::temp_dir().join(format!("{prefix}-{}.sqlite", uuid::Uuid::new_v4()));
        let db = Database::open(&db_path).unwrap();
        db.run_migrations().unwrap();
        (db, db_path)
    }

    fn remove_temp_test_db(db_path: std::path::PathBuf) {
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn saves_an_empty_llm_config_list() {
        let (db, db_path) = temp_test_db("astro-studio-llm-empty-test");

        validate_and_store_llm_configs(&db, Vec::<LlmConfig>::new()).unwrap();

        assert_eq!(
            read_llm_configs(&db).unwrap(),
            Vec::<LlmConfig>::new()
        );

        drop(db);
        remove_temp_test_db(db_path);
    }

    #[test]
    fn rejects_enabling_more_than_one_text_config() {
        let (db, db_path) = temp_test_db("astro-studio-llm-limit-test");
        let configs = vec![
            LlmConfig {
                id: "text-a".to_string(),
                name: "Text A".to_string(),
                protocol: "openai".to_string(),
                model: "gpt-4o".to_string(),
                api_key: "sk-a".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                capability: "text".to_string(),
                enabled: true,
            },
            LlmConfig {
                id: "text-b".to_string(),
                name: "Text B".to_string(),
                protocol: "openai".to_string(),
                model: "gpt-4o-mini".to_string(),
                api_key: "sk-b".to_string(),
                base_url: "https://api.openai.com/v1".to_string(),
                capability: "text".to_string(),
                enabled: true,
            },
        ];

        let result = validate_and_store_llm_configs(&db, configs);

        assert!(matches!(result, Err(AppError::Validation { .. })));

        drop(db);
        remove_temp_test_db(db_path);
    }

    #[test]
    fn extract_prompt_system_prompt_requires_matching_ui_language() {
        let system_prompt = build_extract_prompt_from_image_system_prompt("zh-CN");

        assert!(system_prompt.contains("Simplified Chinese (zh-CN)"));
    }

    #[test]
    fn stores_prompt_extraction_records() {
        let (db, db_path) = temp_test_db("astro-studio-prompt-extraction-test");

        let record = insert_prompt_extraction(
            &db,
            "/tmp/reference.png",
            "cinematic portrait",
            "vision-1",
        )
        .unwrap();

        let conn = db.conn.lock().unwrap();
        let row = conn
            .query_row(
                "SELECT image_path, prompt, llm_config_id FROM prompt_extractions WHERE id = ?1",
                rusqlite::params![record.id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(row.0, "/tmp/reference.png");
        assert_eq!(row.1, "cinematic portrait");
        assert_eq!(row.2, "vision-1");

        drop(conn);
        drop(db);
        remove_temp_test_db(db_path);
    }

    #[test]
    fn get_prompt_extractions_returns_newest_first() {
        let (db, db_path) = temp_test_db("astro-studio-prompt-extraction-history-test");

        let older = insert_prompt_extraction(
            &db,
            "/tmp/older.png",
            "older prompt",
            "vision-1",
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
        let newer = insert_prompt_extraction(
            &db,
            "/tmp/newer.png",
            "newer prompt",
            "vision-1",
        )
        .unwrap();

        let rows = list_prompt_extractions(&db, 20).unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, newer.id);
        assert_eq!(rows[0].image_path, "/tmp/newer.png");
        assert_eq!(rows[0].prompt, "newer prompt");
        assert_eq!(rows[1].id, older.id);

        drop(db);
        remove_temp_test_db(db_path);
    }
}
