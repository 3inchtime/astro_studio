use crate::credentials::{
    CredentialStatus, CredentialUpdate, ProviderProfileWrite, ProviderProfilesDto,
};
use crate::db::Database;
use crate::error::AppError;
use crate::model_registry::{
    default_endpoint_settings_for_model, endpoint_value_or_default, legacy_model_setting_ids,
    model_setting_key, normalize_endpoint_mode, normalize_image_model,
};
use crate::models::*;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn normalize_api_key_for_storage(key: &str) -> String {
    let trimmed = key.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let prefix = parts.next().unwrap_or_default();

    if prefix.eq_ignore_ascii_case("bearer") {
        return parts.next().unwrap_or_default().trim().to_string();
    }

    trimmed.to_string()
}

fn normalize_provider_name(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        "Provider".to_string()
    } else {
        name.to_string()
    }
}

fn normalize_provider_endpoint_settings(
    model: &str,
    settings: &EndpointSettings,
) -> EndpointSettings {
    let defaults = default_endpoint_settings_for_model(model);
    let mode = normalize_endpoint_mode(&settings.mode).to_string();
    let base_url = crate::model_registry::migrate_official_gemini_url(&endpoint_value_or_default(
        Some(settings.base_url.clone()),
        &defaults.base_url,
    ));
    let generation_url =
        crate::model_registry::migrate_official_gemini_url(&endpoint_value_or_default(
            Some(settings.generation_url.clone()),
            &defaults.generation_url,
        ));
    let edit_url = if defaults.edit_url == defaults.generation_url {
        generation_url.clone()
    } else {
        endpoint_value_or_default(Some(settings.edit_url.clone()), &defaults.edit_url)
    };

    EndpointSettings {
        mode,
        base_url,
        generation_url,
        edit_url,
    }
}

fn model_provider_profiles_key(model: &str) -> String {
    format!(
        "{}::{}",
        SETTING_MODEL_PROVIDER_PROFILES_PREFIX,
        normalize_image_model(model)
    )
}

fn model_active_provider_key(model: &str) -> String {
    format!(
        "{}::{}",
        SETTING_MODEL_ACTIVE_PROVIDER_PREFIX,
        normalize_image_model(model)
    )
}

fn get_model_setting(
    db: &Database,
    model: &str,
    suffix: &str,
    legacy_key: Option<&str>,
) -> Result<Option<String>, AppError> {
    let namespaced = db.get_setting(&model_setting_key(model, suffix))?;
    if namespaced.is_some() {
        return Ok(namespaced);
    }

    for legacy_model in legacy_model_setting_ids(model) {
        let legacy_namespaced =
            db.get_setting(&format!("model_config::{legacy_model}::{suffix}"))?;
        if legacy_namespaced.is_some() {
            return Ok(legacy_namespaced);
        }
    }

    if normalize_image_model(model) == ENGINE_GPT_IMAGE_2 {
        if let Some(legacy_key) = legacy_key {
            return db.get_setting(legacy_key);
        }
    }

    Ok(None)
}

fn set_model_setting(
    db: &Database,
    model: &str,
    suffix: &str,
    value: &str,
) -> Result<(), AppError> {
    db.set_setting(&model_setting_key(model, suffix), value)
}

fn read_legacy_model_api_key(db: &Database, model: &str) -> Result<Option<String>, AppError> {
    db.migrate_credentials()?;
    get_model_setting(db, model, SETTING_API_KEY, Some(SETTING_API_KEY))?
        .map(|key| db.resolve_legacy_credential(&key))
        .transpose()
}

fn read_legacy_model_endpoint_settings(
    db: &Database,
    model: &str,
) -> Result<EndpointSettings, AppError> {
    let defaults = default_endpoint_settings_for_model(model);

    Ok(EndpointSettings {
        mode: normalize_endpoint_mode(
            get_model_setting(
                db,
                model,
                SETTING_ENDPOINT_MODE,
                Some(SETTING_ENDPOINT_MODE),
            )?
            .as_deref()
            .unwrap_or(ENDPOINT_MODE_BASE_URL),
        )
        .to_string(),
        base_url: endpoint_value_or_default(
            get_model_setting(db, model, SETTING_BASE_URL, Some(SETTING_BASE_URL))?,
            &defaults.base_url,
        ),
        generation_url: endpoint_value_or_default(
            get_model_setting(
                db,
                model,
                SETTING_GENERATION_URL,
                Some(SETTING_GENERATION_URL),
            )?,
            &defaults.generation_url,
        ),
        edit_url: endpoint_value_or_default(
            get_model_setting(db, model, SETTING_EDIT_URL, Some(SETTING_EDIT_URL))?,
            &defaults.edit_url,
        ),
    })
}

fn default_provider_profile_for_model(
    db: &Database,
    model: &str,
) -> Result<ModelProviderProfile, AppError> {
    let normalized_model = crate::model_registry::require_image_model(model)?.id;
    Ok(ModelProviderProfile {
        id: DEFAULT_PROVIDER_ID.to_string(),
        name: DEFAULT_PROVIDER_NAME.to_string(),
        api_key: read_legacy_model_api_key(db, normalized_model)?.unwrap_or_default(),
        endpoint_settings: read_legacy_model_endpoint_settings(db, normalized_model)?,
    })
}

fn normalize_provider_profiles_state(
    model: &str,
    state: ModelProviderProfilesState,
) -> Result<ModelProviderProfilesState, AppError> {
    let normalized_model = crate::model_registry::require_image_model(model)?.id;
    let mut seen = std::collections::HashSet::new();
    let mut profiles = Vec::with_capacity(state.profiles.len());

    for profile in state.profiles {
        let id = profile.id.trim().to_string();
        if id.is_empty() {
            return Err(AppError::Validation {
                message: "Provider id cannot be empty.".to_string(),
            });
        }
        if !seen.insert(id.clone()) {
            return Err(AppError::Validation {
                message: format!("Duplicate provider id: {id}"),
            });
        }

        profiles.push(ModelProviderProfile {
            id,
            name: normalize_provider_name(&profile.name),
            api_key: normalize_api_key_for_storage(&profile.api_key),
            endpoint_settings: normalize_provider_endpoint_settings(
                normalized_model,
                &profile.endpoint_settings,
            ),
        });
    }

    let active_provider_id = if profiles
        .iter()
        .any(|profile| profile.id == state.active_provider_id)
    {
        state.active_provider_id
    } else {
        profiles
            .first()
            .map(|profile| profile.id.clone())
            .unwrap_or_default()
    };

    Ok(ModelProviderProfilesState {
        active_provider_id,
        profiles,
    })
}

pub(crate) fn read_model_provider_profiles_state(
    db: &Database,
    model: &str,
) -> Result<ModelProviderProfilesState, AppError> {
    let normalized_model = crate::model_registry::require_image_model(model)?.id;
    let stored_profiles =
        db.read_credential_json(&model_provider_profiles_key(normalized_model))?;
    let profiles = match stored_profiles.as_deref() {
        Some(value) => serde_json::from_str::<Vec<ModelProviderProfile>>(value).map_err(|e| {
            AppError::Database {
                message: format!("Deserialize provider profiles failed: {}", e),
            }
        })?,
        None => vec![default_provider_profile_for_model(db, normalized_model)?],
    };
    let active_provider_id = db
        .get_setting(&model_active_provider_key(normalized_model))?
        .unwrap_or_else(|| DEFAULT_PROVIDER_ID.to_string());

    normalize_provider_profiles_state(
        normalized_model,
        ModelProviderProfilesState {
            active_provider_id,
            profiles,
        },
    )
}

#[cfg(test)]
pub(crate) fn save_model_provider_profiles_state(
    db: &Database,
    model: &str,
    state: ModelProviderProfilesState,
) -> Result<ModelProviderProfilesState, AppError> {
    save_model_provider_profiles_with_kept_ids(db, model, state, &std::collections::HashSet::new())
}

fn save_model_provider_metadata_state(
    db: &Database,
    model: &str,
    state: ModelProviderProfilesState,
) -> Result<ModelProviderProfilesState, AppError> {
    let kept_ids = state
        .profiles
        .iter()
        .map(|profile| profile.id.clone())
        .collect();
    save_model_provider_profiles_with_kept_ids(db, model, state, &kept_ids)
}

fn save_model_provider_profiles_with_kept_ids(
    db: &Database,
    model: &str,
    state: ModelProviderProfilesState,
    kept_ids: &std::collections::HashSet<String>,
) -> Result<ModelProviderProfilesState, AppError> {
    let normalized_model = crate::model_registry::require_image_model(model)?.id;
    let state = normalize_provider_profiles_state(normalized_model, state)?;
    let profiles_json = serde_json::to_string(&state.profiles).map_err(|e| AppError::Database {
        message: format!("Serialize provider profiles failed: {}", e),
    })?;

    db.write_credential_json_with_kept_ids(
        &model_provider_profiles_key(normalized_model),
        &profiles_json,
        kept_ids,
    )?;
    db.set_setting(
        &model_active_provider_key(normalized_model),
        &state.active_provider_id,
    )?;

    // Keep resolves against the latest reference, not necessarily the snapshot
    // used to assemble this update. Return the committed credential status.
    read_model_provider_profiles_state(db, normalized_model)
}

/// An explicit selection remains bound to its saved profile if the active
/// provider changes while a job waits in the queue.
pub(crate) fn resolve_profile(
    db: &Database,
    model: &str,
    selected_id: Option<&str>,
) -> Result<ModelProviderProfile, AppError> {
    let model = crate::model_registry::require_image_model(model)?.id;
    let state = read_model_provider_profiles_state(db, model)?;
    let selected_id = selected_id.unwrap_or(&state.active_provider_id);
    state
        .profiles
        .into_iter()
        .find(|p| p.id == selected_id)
        .ok_or_else(|| AppError::ProviderProfileNotFound {
            model: model.to_owned(),
        })
}

pub(crate) fn current_image_model(db: &Database) -> Result<String, AppError> {
    if let Some(model) = db.get_setting(SETTING_IMAGE_MODEL)? {
        return Ok(normalize_image_model(&model).to_string());
    }
    // Older installations may have configured the implicit GPT Image 2 default.
    for key in [
        SETTING_API_KEY.to_string(),
        SETTING_BASE_URL.to_string(),
        model_provider_profiles_key(ENGINE_GPT_IMAGE_2),
        model_setting_key(ENGINE_GPT_IMAGE_2, SETTING_API_KEY),
    ] {
        if db.get_setting(&key)?.is_some() {
            return Ok(ENGINE_GPT_IMAGE_2.to_string());
        }
    }
    Ok(crate::model_catalog::CATALOG.default_image_model.clone())
}

fn read_endpoint_settings(db: &Database) -> Result<EndpointSettings, AppError> {
    let model = current_image_model(db)?;
    read_model_endpoint_settings(db, &model)
}

pub(crate) fn read_model_endpoint_settings(
    db: &Database,
    model: &str,
) -> Result<EndpointSettings, AppError> {
    Ok(resolve_profile(db, model, None)?.endpoint_settings)
}

pub(crate) fn read_model_api_key(db: &Database, model: &str) -> Result<Option<String>, AppError> {
    let profile = resolve_profile(db, model, None)?;
    Ok(Some(profile.api_key).filter(|key| !key.trim().is_empty()))
}

pub(crate) fn save_model_api_key_value(
    db: &Database,
    model: &str,
    key: &str,
) -> Result<(), AppError> {
    let normalized_model = crate::model_registry::require_image_model(model)?.id;
    let key = normalize_api_key_for_storage(key);
    let mut state = read_model_provider_profiles_state(db, normalized_model)?;
    let kept_ids = state
        .profiles
        .iter()
        .filter(|profile| profile.id != state.active_provider_id)
        .map(|profile| profile.id.clone())
        .collect();
    for profile in &mut state.profiles {
        if profile.id == state.active_provider_id {
            profile.api_key = key.clone();
        }
    }
    save_model_provider_profiles_with_kept_ids(db, normalized_model, state, &kept_ids)?;

    Ok(())
}

pub(crate) fn save_model_endpoint_settings_value(
    db: &Database,
    model: &str,
    mode: &str,
    base_url: &str,
    generation_url: &str,
    edit_url: &str,
) -> Result<(), AppError> {
    let normalized_model = crate::model_registry::require_image_model(model)?.id;
    let endpoint_settings = normalize_provider_endpoint_settings(
        normalized_model,
        &EndpointSettings {
            mode: mode.to_string(),
            base_url: base_url.to_string(),
            generation_url: generation_url.to_string(),
            edit_url: edit_url.to_string(),
        },
    );
    let mut state = read_model_provider_profiles_state(db, normalized_model)?;
    for profile in &mut state.profiles {
        if profile.id == state.active_provider_id {
            profile.endpoint_settings = endpoint_settings.clone();
        }
    }
    save_model_provider_metadata_state(db, normalized_model, state)?;

    if normalized_model == ENGINE_GPT_IMAGE_2 {
        db.set_setting(SETTING_ENDPOINT_MODE, &endpoint_settings.mode)?;
        db.set_setting(SETTING_BASE_URL, &endpoint_settings.base_url)?;
        db.set_setting(SETTING_GENERATION_URL, &endpoint_settings.generation_url)?;
        db.set_setting(SETTING_EDIT_URL, &endpoint_settings.edit_url)?;
        set_model_setting(
            db,
            normalized_model,
            SETTING_ENDPOINT_MODE,
            &endpoint_settings.mode,
        )?;
        set_model_setting(
            db,
            normalized_model,
            SETTING_BASE_URL,
            &endpoint_settings.base_url,
        )?;
        set_model_setting(
            db,
            normalized_model,
            SETTING_GENERATION_URL,
            &endpoint_settings.generation_url,
        )?;
        set_model_setting(
            db,
            normalized_model,
            SETTING_EDIT_URL,
            &endpoint_settings.edit_url,
        )?;
    }

    Ok(())
}

pub(crate) fn save_api_key(db: &Database, key: String) -> Result<(), AppError> {
    log::info!("Saving API key");
    let model = current_image_model(db)?;
    save_model_api_key_value(db, &model, &key)
}

pub(crate) fn get_api_key(db: &Database) -> Result<CredentialStatus, AppError> {
    let model = current_image_model(db)?;
    Ok(CredentialStatus::from_secret(
        &read_model_api_key(db, &model)?.unwrap_or_default(),
    ))
}

pub(crate) fn save_base_url(db: &Database, url: String) -> Result<(), AppError> {
    log::info!("Saving provider base URL");
    let model = current_image_model(db)?;
    let settings = read_model_endpoint_settings(db, &model)?;
    save_model_endpoint_settings_value(
        db,
        &model,
        &settings.mode,
        &url,
        &settings.generation_url,
        &settings.edit_url,
    )
}

pub(crate) fn get_base_url(db: &Database) -> Result<String, AppError> {
    Ok(read_model_endpoint_settings(db, &current_image_model(db)?)?.base_url)
}

pub(crate) fn get_endpoint_settings(db: &Database) -> Result<EndpointSettings, AppError> {
    read_endpoint_settings(db)
}

pub(crate) fn save_endpoint_settings(
    db: &Database,
    mode: String,
    base_url: String,
    generation_url: String,
    edit_url: String,
) -> Result<(), AppError> {
    let model = current_image_model(db)?;
    let defaults = default_endpoint_settings_for_model(&model);
    let mode = normalize_endpoint_mode(&mode);
    let base_url = endpoint_value_or_default(Some(base_url), &defaults.base_url);
    let generation_url = endpoint_value_or_default(Some(generation_url), &defaults.generation_url);
    let edit_url = if defaults.generation_url == defaults.edit_url {
        generation_url.clone()
    } else {
        endpoint_value_or_default(Some(edit_url), &defaults.edit_url)
    };

    save_model_endpoint_settings_value(db, &model, mode, &base_url, &generation_url, &edit_url)
}

pub(crate) fn get_model_api_key(
    db: &Database,
    model: String,
) -> Result<CredentialStatus, AppError> {
    Ok(CredentialStatus::from_secret(
        &read_model_api_key(db, &model)?.unwrap_or_default(),
    ))
}

pub(crate) fn save_model_api_key(
    db: &Database,
    model: String,
    key: String,
) -> Result<(), AppError> {
    save_model_api_key_value(db, &model, &key)
}

pub(crate) fn get_model_endpoint_settings(
    db: &Database,
    model: String,
) -> Result<EndpointSettings, AppError> {
    read_model_endpoint_settings(db, &model)
}

pub(crate) fn save_model_endpoint_settings(
    db: &Database,
    model: String,
    mode: String,
    base_url: String,
    generation_url: String,
    edit_url: String,
) -> Result<(), AppError> {
    save_model_endpoint_settings_value(db, &model, &mode, &base_url, &generation_url, &edit_url)
}

pub(crate) fn get_model_provider_profiles(
    db: &Database,
    model: String,
) -> Result<ProviderProfilesDto, AppError> {
    read_model_provider_profiles_state(db, &model).map(Into::into)
}

pub(crate) fn save_model_provider_profiles(
    db: &Database,
    model: String,
    active_provider_id: String,
    profiles: Vec<ProviderProfileWrite>,
) -> Result<ProviderProfilesDto, AppError> {
    save_provider_profile_updates(db, &model, active_provider_id, profiles)
}

pub(crate) fn save_provider_profile_updates(
    db: &Database,
    model: &str,
    active_provider_id: String,
    profiles: Vec<ProviderProfileWrite>,
) -> Result<ProviderProfilesDto, AppError> {
    let kept_ids = profiles
        .iter()
        .filter(|p| matches!(p.credential_update, CredentialUpdate::Keep))
        .map(|p| p.id.clone())
        .collect();
    let previous = read_model_provider_profiles_state(db, model)?;
    let profiles = profiles
        .into_iter()
        .map(|p| p.apply(&previous.profiles))
        .collect::<Result<Vec<_>, _>>()?;
    save_model_provider_profiles_with_kept_ids(
        db,
        model,
        ModelProviderProfilesState {
            active_provider_id,
            profiles,
        },
        &kept_ids,
    )
    .map(Into::into)
}

pub(crate) fn create_model_provider_profile(
    db: &Database,
    model: String,
    name: String,
) -> Result<ProviderProfilesDto, AppError> {
    let normalized_model = crate::model_registry::require_image_model(&model)?.id;
    let mut state = read_model_provider_profiles_state(db, normalized_model)?;
    let provider_id = uuid::Uuid::new_v4().to_string();
    state.profiles.push(ModelProviderProfile {
        id: provider_id,
        name: normalize_provider_name(&name),
        api_key: String::new(),
        endpoint_settings: default_endpoint_settings_for_model(normalized_model),
    });
    save_model_provider_metadata_state(db, normalized_model, state).map(Into::into)
}

pub(crate) fn delete_model_provider_profile(
    db: &Database,
    model: String,
    provider_id: String,
) -> Result<ProviderProfilesDto, AppError> {
    let normalized_model = crate::model_registry::require_image_model(&model)?.id;
    let mut state = read_model_provider_profiles_state(db, normalized_model)?;
    state.profiles.retain(|profile| profile.id != provider_id);
    if !state
        .profiles
        .iter()
        .any(|profile| profile.id == state.active_provider_id)
    {
        state.active_provider_id = state
            .profiles
            .first()
            .map(|profile| profile.id.clone())
            .unwrap_or_default();
    }
    save_model_provider_metadata_state(db, normalized_model, state).map(Into::into)
}

pub(crate) fn set_active_model_provider(
    db: &Database,
    model: String,
    provider_id: String,
) -> Result<ProviderProfilesDto, AppError> {
    let normalized_model = crate::model_registry::require_image_model(&model)?.id;
    let mut state = read_model_provider_profiles_state(db, normalized_model)?;
    if !state
        .profiles
        .iter()
        .any(|profile| profile.id == provider_id)
    {
        return Err(AppError::Validation {
            message: "Provider profile not found.".to_string(),
        });
    }
    state.active_provider_id = provider_id;
    save_model_provider_metadata_state(db, normalized_model, state).map(Into::into)
}

pub(crate) fn get_image_model(db: &Database) -> Result<String, AppError> {
    current_image_model(db)
}

pub(crate) fn save_image_model(db: &Database, model: String) -> Result<(), AppError> {
    db.set_setting(
        SETTING_IMAGE_MODEL,
        crate::model_registry::require_image_model(&model)?.id,
    )
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
