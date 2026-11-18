use crate::{
    credentials::{CredentialUpdate, LlmConfigDto, LlmConfigWrite},
    db::Database,
    error::AppError,
    llm,
    models::{LlmConfig, SETTING_LLM_CONFIGS},
};

fn supports_text(config: &LlmConfig) -> bool {
    matches!(config.capability.as_str(), "text" | "multimodal")
}

pub(crate) fn read_llm_configs(db: &Database) -> Result<Vec<LlmConfig>, AppError> {
    match db.read_credential_json(SETTING_LLM_CONFIGS)? {
        Some(json) => serde_json::from_str(&json).map_err(|e| AppError::Database {
            message: format!("Failed to deserialize LLM configs: {}", e),
        }),
        None => Ok(Vec::new()),
    }
}

#[cfg(test)]
pub(crate) fn validate_and_store_llm_configs(
    db: &Database,
    configs: Vec<LlmConfig>,
) -> Result<(), AppError> {
    validate_and_store_llm_configs_with_kept_ids(db, configs, &std::collections::HashSet::new())
}
fn validate_and_store_llm_configs_with_kept_ids(
    db: &Database,
    configs: Vec<LlmConfig>,
    kept_ids: &std::collections::HashSet<String>,
) -> Result<(), AppError> {
    let mut ids = std::collections::HashSet::new();
    for config in &configs {
        if !ids.insert(config.id.as_str()) {
            return Err(AppError::Validation {
                message: "Duplicate LLM configuration id".into(),
            });
        }
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
        llm::protocol_settings(&config.protocol)?;
        if !supports_text(config) {
            return Err(AppError::Validation {
                message: format!(
                    "LLM config capability must be 'text' or 'multimodal', got '{}'",
                    config.capability
                ),
            });
        }
    }

    validate_llm_enabled_limits(&configs)?;
    db.write_credential_json_with_kept_ids(
        SETTING_LLM_CONFIGS,
        &serde_json::to_string(&configs)?,
        kept_ids,
    )
}

fn validate_llm_enabled_limits(configs: &[LlmConfig]) -> Result<(), AppError> {
    let limits = &crate::model_catalog::CATALOG.llm.enabled_limits;
    let enabled_total = configs.iter().filter(|config| config.enabled).count();
    if enabled_total > limits.total {
        return Err(AppError::Validation {
            message: format!("Only {} LLM configs can be enabled at once.", limits.total),
        });
    }

    let text_enabled = configs
        .iter()
        .filter(|config| config.enabled && config.capability == "text")
        .count();
    if text_enabled > limits.text {
        return Err(AppError::Validation {
            message: format!(
                "Only {} text LLM config can be enabled at once.",
                limits.text
            ),
        });
    }

    let multimodal_enabled = configs
        .iter()
        .filter(|config| config.enabled && config.capability == "multimodal")
        .count();
    if multimodal_enabled > limits.multimodal {
        return Err(AppError::Validation {
            message: format!(
                "Only {} multimodal LLM configs can be enabled at once.",
                limits.multimodal
            ),
        });
    }

    Ok(())
}

pub(crate) fn resolve_multimodal_config<'a>(
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

pub(crate) fn resolve_prompt_optimization_config<'a>(
    configs: &'a [LlmConfig],
    config_id: &str,
) -> Result<&'a LlmConfig, AppError> {
    let config =
        configs
            .iter()
            .find(|c| c.id == config_id)
            .ok_or_else(|| AppError::Validation {
                message: format!("LLM config not found: {}", config_id),
            })?;

    if !supports_text(config) {
        return Err(AppError::Validation {
            message: format!(
                "LLM config '{}' has unsupported capability '{}'",
                config.name, config.capability
            ),
        });
    }

    if !config.enabled {
        return Err(AppError::Validation {
            message: format!("LLM config '{}' is disabled", config.name),
        });
    }

    Ok(config)
}

pub(crate) fn get_llm_configs(db: &Database) -> Result<Vec<LlmConfigDto>, AppError> {
    Ok(read_llm_configs(db)?.into_iter().map(Into::into).collect())
}

pub(crate) fn save_llm_configs(
    db: &Database,
    configs: Vec<LlmConfigWrite>,
) -> Result<(), AppError> {
    let kept_ids = configs
        .iter()
        .filter(|c| matches!(c.credential_update, CredentialUpdate::Keep))
        .map(|c| c.id.clone())
        .collect();
    let previous = read_llm_configs(db)?;
    let configs = configs
        .into_iter()
        .map(|c| c.apply(&previous))
        .collect::<Result<Vec<_>, _>>()?;
    validate_and_store_llm_configs_with_kept_ids(db, configs, &kept_ids)
}

pub(crate) fn resolve_agent_llm_config<'a>(
    configs: &'a [LlmConfig],
    config_id: &str,
) -> Result<&'a LlmConfig, AppError> {
    let config = configs
        .iter()
        .find(|config| config.id == config_id && config.enabled)
        .ok_or_else(|| AppError::Validation {
            message: "Select an enabled LLM config before using deep thinking mode.".to_string(),
        })?;

    if !supports_text(config) {
        return Err(AppError::Validation {
            message: format!("LLM config '{}' cannot run the prompt agent.", config.name),
        });
    }

    Ok(config)
}
