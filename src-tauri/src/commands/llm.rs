use crate::db::Database;
use crate::error::AppError;
use crate::llm;
use crate::models::{LlmConfig, SETTING_LLM_CONFIGS};
use tauri::State;

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
    // Validate configs before saving
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

    write_llm_configs(db.inner(), &configs)
}

#[tauri::command]
pub(crate) async fn optimize_prompt(
    db: State<'_, Database>,
    prompt: String,
    config_id: String,
) -> Result<String, AppError> {
    let configs = read_llm_configs(db.inner())?;

    let config = configs
        .iter()
        .find(|c| c.id == config_id)
        .ok_or_else(|| AppError::Validation {
            message: format!("LLM config not found: {}", config_id),
        })?;

    if config.capability != "text" {
        return Err(AppError::Validation {
            message: format!(
                "LLM config '{}' has capability '{}' — only text models are supported for prompt optimization",
                config.name, config.capability
            ),
        });
    }

    if !config.enabled {
        return Err(AppError::Validation {
            message: format!("LLM config '{}' is disabled", config.name),
        });
    }

    log::info!(
        "Optimizing prompt with LLM config '{}' (protocol: {}, model: {})",
        config.name,
        config.protocol,
        config.model
    );

    let client = llm::create_llm_client(config)?;
    let result = client
        .chat(OPTIMIZE_PROMPT_SYSTEM_PROMPT, &prompt)
        .await;

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
