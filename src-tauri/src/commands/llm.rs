use crate::db::Database;
use crate::error::AppError;
use crate::llm::{self, ImageData, MULTIMODAL_TIMEOUT_SECS};
use crate::models::{LlmConfig, SETTING_LLM_CONFIGS};
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
pub(crate) async fn optimize_prompt(
    db: State<'_, Database>,
    prompt: String,
    config_id: String,
    image_paths: Option<Vec<String>>,
) -> Result<String, AppError> {
    let configs = read_llm_configs(db.inner())?;
    let has_images = image_paths.as_ref().map_or(false, |p| !p.is_empty());

    let config = if has_images {
        // Prefer the provided config if it's multimodal, otherwise find the first enabled multimodal
        let config = configs
            .iter()
            .find(|c| c.id == config_id && c.capability == "multimodal" && c.enabled);
        config
            .or_else(|| {
                configs
                    .iter()
                    .find(|c| c.enabled && c.capability == "multimodal")
            })
            .ok_or_else(|| AppError::Validation {
                message:
                    "No enabled multimodal LLM config found. Please configure a multimodal LLM in settings."
                        .to_string(),
            })?
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
}
