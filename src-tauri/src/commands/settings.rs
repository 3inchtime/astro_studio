use crate::db::Database;
use crate::error::AppError;
use crate::model_registry::{
    default_endpoint_settings_for_model, endpoint_value_or_default,
    is_gemini_model, legacy_model_setting_ids, model_setting_key,
    normalize_endpoint_mode, normalize_image_model,
};
use crate::models::*;
use tauri::State;

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
    let base_url = endpoint_value_or_default(Some(settings.base_url.clone()), &defaults.base_url);
    let generation_url = endpoint_value_or_default(
        Some(settings.generation_url.clone()),
        &defaults.generation_url,
    );
    let edit_url = if is_gemini_model(model) {
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
            return Ok(db.get_setting(legacy_key)?);
        }
    }

    Ok(None)
}

fn set_model_setting(db: &Database, model: &str, suffix: &str, value: &str) -> Result<(), AppError> {
    Ok(db.set_setting(&model_setting_key(model, suffix), value)?)
}

fn read_legacy_model_api_key(db: &Database, model: &str) -> Result<Option<String>, AppError> {
    Ok(
        get_model_setting(db, model, SETTING_API_KEY, Some(SETTING_API_KEY))?
            .map(|key| normalize_api_key_for_storage(&key)),
    )
}

fn read_legacy_model_endpoint_settings(
    db: &Database,
    model: &str,
) -> Result<EndpointSettings, AppError> {
    let defaults = default_endpoint_settings_for_model(model);

    Ok(EndpointSettings {
        mode: normalize_endpoint_mode(
            get_model_setting(db, model, SETTING_ENDPOINT_MODE, Some(SETTING_ENDPOINT_MODE))?
                .as_deref()
                .unwrap_or(ENDPOINT_MODE_BASE_URL),
        )
        .to_string(),
        base_url: endpoint_value_or_default(
            get_model_setting(db, model, SETTING_BASE_URL, Some(SETTING_BASE_URL))?,
            &defaults.base_url,
        ),
        generation_url: endpoint_value_or_default(
            get_model_setting(db, model, SETTING_GENERATION_URL, Some(SETTING_GENERATION_URL))?,
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
    let normalized_model = normalize_image_model(model);
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
    let normalized_model = normalize_image_model(model);
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
    let normalized_model = normalize_image_model(model);
    let stored_profiles = db.get_setting(&model_provider_profiles_key(normalized_model))?;
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

pub(crate) fn save_model_provider_profiles_state(
    db: &Database,
    model: &str,
    state: ModelProviderProfilesState,
) -> Result<ModelProviderProfilesState, AppError> {
    let normalized_model = normalize_image_model(model);
    let state = normalize_provider_profiles_state(normalized_model, state)?;
    let profiles_json = serde_json::to_string(&state.profiles)
        .map_err(|e| AppError::Database {
            message: format!("Serialize provider profiles failed: {}", e),
        })?;

    db.set_setting(&model_provider_profiles_key(normalized_model), &profiles_json)?;
    db.set_setting(&model_active_provider_key(normalized_model), &state.active_provider_id)?;

    Ok(state)
}

pub(crate) fn active_provider_profile_for_model(
    db: &Database,
    model: &str,
) -> Result<ModelProviderProfile, AppError> {
    let state = read_model_provider_profiles_state(db, model)?;
    state
        .profiles
        .into_iter()
        .find(|profile| profile.id == state.active_provider_id)
        .ok_or_else(|| AppError::ProviderProfileNotFound {
            model: model.to_string(),
        })
}

// ── Commands ─────────────────────────────────────────────────────────────────

fn current_image_model(db: &Database) -> Result<String, AppError> {
    Ok(normalize_image_model(
        db.get_setting(SETTING_IMAGE_MODEL)?
            .as_deref()
            .unwrap_or(DEFAULT_IMAGE_MODEL),
    )
    .to_string())
}

fn read_endpoint_settings(db: &Database) -> Result<EndpointSettings, AppError> {
    let model = current_image_model(db)?;
    read_model_endpoint_settings(db, &model)
}

pub(crate) fn read_model_endpoint_settings(
    db: &Database,
    model: &str,
) -> Result<EndpointSettings, AppError> {
    Ok(active_provider_profile_for_model(db, model)?.endpoint_settings)
}

pub(crate) fn read_model_api_key(
    db: &Database,
    model: &str,
) -> Result<Option<String>, AppError> {
    let profile = active_provider_profile_for_model(db, model)?;
    Ok(Some(profile.api_key).filter(|key| !key.trim().is_empty()))
}

pub(crate) fn save_model_api_key_value(
    db: &Database,
    model: &str,
    key: &str,
) -> Result<(), AppError> {
    let normalized_model = normalize_image_model(model);
    let key = normalize_api_key_for_storage(key);
    let mut state = read_model_provider_profiles_state(db, normalized_model)?;
    for profile in &mut state.profiles {
        if profile.id == state.active_provider_id {
            profile.api_key = key.clone();
        }
    }
    save_model_provider_profiles_state(db, normalized_model, state)?;

    if normalized_model == ENGINE_GPT_IMAGE_2 {
        db.set_setting(SETTING_API_KEY, &key)?;
        set_model_setting(db, normalized_model, SETTING_API_KEY, &key)?;
    }
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
    let normalized_model = normalize_image_model(model);
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
    save_model_provider_profiles_state(db, normalized_model, state)?;

    if normalized_model == ENGINE_GPT_IMAGE_2 {
        db.set_setting(SETTING_ENDPOINT_MODE, &endpoint_settings.mode)?;
        db.set_setting(SETTING_BASE_URL, &endpoint_settings.base_url)?;
        db.set_setting(SETTING_GENERATION_URL, &endpoint_settings.generation_url)?;
        db.set_setting(SETTING_EDIT_URL, &endpoint_settings.edit_url)?;
        set_model_setting(db, normalized_model, SETTING_ENDPOINT_MODE, &endpoint_settings.mode)?;
        set_model_setting(db, normalized_model, SETTING_BASE_URL, &endpoint_settings.base_url)?;
        set_model_setting(
            db,
            normalized_model,
            SETTING_GENERATION_URL,
            &endpoint_settings.generation_url,
        )?;
        set_model_setting(db, normalized_model, SETTING_EDIT_URL, &endpoint_settings.edit_url)?;
    }

    Ok(())
}

#[tauri::command]
pub(crate) fn save_api_key(db: State<'_, Database>, key: String) -> Result<(), AppError> {
    log::info!("Saving API key");
    let model = current_image_model(db.inner())?;
    save_model_api_key_value(db.inner(), &model, &key)
}

#[tauri::command]
pub(crate) fn get_api_key(db: State<'_, Database>) -> Result<Option<String>, AppError> {
    let model = current_image_model(db.inner())?;
    read_model_api_key(db.inner(), &model)
}

#[tauri::command]
pub(crate) fn save_base_url(db: State<'_, Database>, url: String) -> Result<(), AppError> {
    log::info!("Saving base URL: {}", url);
    let model = current_image_model(db.inner())?;
    let settings = read_model_endpoint_settings(db.inner(), &model)?;
    save_model_endpoint_settings_value(
        db.inner(),
        &model,
        &settings.mode,
        &url,
        &settings.generation_url,
        &settings.edit_url,
    )
}

#[tauri::command]
pub(crate) fn get_base_url(db: State<'_, Database>) -> Result<String, AppError> {
    Ok(read_model_endpoint_settings(db.inner(), &current_image_model(db.inner())?)?.base_url)
}

#[tauri::command]
pub(crate) fn get_endpoint_settings(db: State<'_, Database>) -> Result<EndpointSettings, AppError> {
    read_endpoint_settings(db.inner())
}

#[tauri::command]
pub(crate) fn save_endpoint_settings(
    db: State<'_, Database>,
    mode: String,
    base_url: String,
    generation_url: String,
    edit_url: String,
) -> Result<(), AppError> {
    let model = current_image_model(db.inner())?;
    let defaults = default_endpoint_settings_for_model(&model);
    let mode = normalize_endpoint_mode(&mode);
    let base_url = endpoint_value_or_default(Some(base_url), &defaults.base_url);
    let generation_url =
        endpoint_value_or_default(Some(generation_url), &defaults.generation_url);
    let edit_url = if is_gemini_model(&model) {
        generation_url.clone()
    } else {
        endpoint_value_or_default(Some(edit_url), &defaults.edit_url)
    };

    save_model_endpoint_settings_value(
        db.inner(),
        &model,
        mode,
        &base_url,
        &generation_url,
        &edit_url,
    )
}

#[tauri::command]
pub(crate) fn get_model_api_key(
    db: State<'_, Database>,
    model: String,
) -> Result<Option<String>, AppError> {
    read_model_api_key(db.inner(), &model)
}

#[tauri::command]
pub(crate) fn save_model_api_key(
    db: State<'_, Database>,
    model: String,
    key: String,
) -> Result<(), AppError> {
    save_model_api_key_value(db.inner(), &model, &key)
}

#[tauri::command]
pub(crate) fn get_model_endpoint_settings(
    db: State<'_, Database>,
    model: String,
) -> Result<EndpointSettings, AppError> {
    read_model_endpoint_settings(db.inner(), &model)
}

#[tauri::command]
pub(crate) fn save_model_endpoint_settings(
    db: State<'_, Database>,
    model: String,
    mode: String,
    base_url: String,
    generation_url: String,
    edit_url: String,
) -> Result<(), AppError> {
    save_model_endpoint_settings_value(db.inner(), &model, &mode, &base_url, &generation_url, &edit_url)
}

#[tauri::command]
pub(crate) fn get_model_provider_profiles(
    db: State<'_, Database>,
    model: String,
) -> Result<ModelProviderProfilesState, AppError> {
    read_model_provider_profiles_state(db.inner(), &model)
}

#[tauri::command]
pub(crate) fn save_model_provider_profiles(
    db: State<'_, Database>,
    model: String,
    active_provider_id: String,
    profiles: Vec<ModelProviderProfile>,
) -> Result<ModelProviderProfilesState, AppError> {
    save_model_provider_profiles_state(
        db.inner(),
        &model,
        ModelProviderProfilesState {
            active_provider_id,
            profiles,
        },
    )
}

#[tauri::command]
pub(crate) fn create_model_provider_profile(
    db: State<'_, Database>,
    model: String,
    name: String,
) -> Result<ModelProviderProfilesState, AppError> {
    let normalized_model = normalize_image_model(&model);
    let mut state = read_model_provider_profiles_state(db.inner(), normalized_model)?;
    let provider_id = uuid::Uuid::new_v4().to_string();
    state.profiles.push(ModelProviderProfile {
        id: provider_id,
        name: normalize_provider_name(&name),
        api_key: String::new(),
        endpoint_settings: default_endpoint_settings_for_model(normalized_model),
    });
    save_model_provider_profiles_state(db.inner(), normalized_model, state)
}

#[tauri::command]
pub(crate) fn delete_model_provider_profile(
    db: State<'_, Database>,
    model: String,
    provider_id: String,
) -> Result<ModelProviderProfilesState, AppError> {
    let normalized_model = normalize_image_model(&model);
    let mut state = read_model_provider_profiles_state(db.inner(), normalized_model)?;
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
    save_model_provider_profiles_state(db.inner(), normalized_model, state)
}

#[tauri::command]
pub(crate) fn set_active_model_provider(
    db: State<'_, Database>,
    model: String,
    provider_id: String,
) -> Result<ModelProviderProfilesState, AppError> {
    let normalized_model = normalize_image_model(&model);
    let mut state = read_model_provider_profiles_state(db.inner(), normalized_model)?;
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
    save_model_provider_profiles_state(db.inner(), normalized_model, state)
}

#[tauri::command]
pub(crate) fn get_font_size(db: State<'_, Database>) -> Result<String, AppError> {
    Ok(crate::normalize_font_size(
        db.get_setting(SETTING_FONT_SIZE)?
            .as_deref()
            .unwrap_or(DEFAULT_FONT_SIZE),
    )
    .to_string())
}

#[tauri::command]
pub(crate) fn save_font_size(db: State<'_, Database>, font_size: String) -> Result<(), AppError> {
    Ok(db.set_setting(
        SETTING_FONT_SIZE,
        crate::normalize_font_size(&font_size),
    )?)
}

#[tauri::command]
pub(crate) fn get_image_model(db: State<'_, Database>) -> Result<String, AppError> {
    Ok(normalize_image_model(
        db.get_setting(SETTING_IMAGE_MODEL)?
            .as_deref()
            .unwrap_or(DEFAULT_IMAGE_MODEL),
    )
    .to_string())
}

#[tauri::command]
pub(crate) fn save_image_model(db: State<'_, Database>, model: String) -> Result<(), AppError> {
    Ok(db.set_setting(SETTING_IMAGE_MODEL, normalize_image_model(&model))?)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::model_registry::model_setting_key;

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
    fn normalize_api_key_for_storage_removes_paste_artifacts() {
        assert_eq!(
            normalize_api_key_for_storage("  sk-proj-valid-token\n"),
            "sk-proj-valid-token"
        );
        assert_eq!(
            normalize_api_key_for_storage("Bearer sk-proj-valid-token"),
            "sk-proj-valid-token"
        );
        assert_eq!(
            normalize_api_key_for_storage("bearer\tsk-proj-valid-token  "),
            "sk-proj-valid-token"
        );
    }

    #[test]
    fn save_model_api_key_value_stores_normalized_key() {
        let db_path = std::env::temp_dir().join(format!(
            "astro-studio-api-key-test-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let db = Database::open(&db_path).unwrap();
        db.run_migrations().unwrap();

        save_model_api_key_value(&db, ENGINE_GPT_IMAGE_2, " Bearer sk-proj-valid-token\n")
            .unwrap();

        assert_eq!(
            read_legacy_model_api_key(&db, ENGINE_GPT_IMAGE_2).unwrap(),
            Some("sk-proj-valid-token".to_string())
        );
        assert_eq!(
            db.get_setting(SETTING_API_KEY).unwrap(),
            Some("sk-proj-valid-token".to_string())
        );

        drop(db);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn read_model_api_key_normalizes_legacy_stored_key() {
        let db_path = std::env::temp_dir().join(format!(
            "astro-studio-legacy-api-key-test-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
        let db = Database::open(&db_path).unwrap();
        db.run_migrations().unwrap();
        db.set_setting(
            &model_setting_key(ENGINE_GPT_IMAGE_2, SETTING_API_KEY),
            " Bearer sk-proj-valid-token\n",
        )
        .unwrap();

        assert_eq!(
            read_legacy_model_api_key(&db, ENGINE_GPT_IMAGE_2).unwrap(),
            Some("sk-proj-valid-token".to_string())
        );

        drop(db);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    #[test]
    fn provider_profiles_default_to_legacy_model_settings() {
        let (db, db_path) = temp_test_db("astro-studio-provider-default-test");
        db.set_setting(
            &model_setting_key(ENGINE_GPT_IMAGE_2, SETTING_API_KEY),
            "Bearer sk-legacy",
        )
        .unwrap();
        db.set_setting(
            &model_setting_key(ENGINE_GPT_IMAGE_2, SETTING_BASE_URL),
            "https://proxy.example/v1",
        )
        .unwrap();

        let state = read_model_provider_profiles_state(&db, ENGINE_GPT_IMAGE_2).unwrap();

        assert_eq!(state.active_provider_id, DEFAULT_PROVIDER_ID);
        assert_eq!(state.profiles.len(), 1);
        assert_eq!(state.profiles[0].id, DEFAULT_PROVIDER_ID);
        assert_eq!(state.profiles[0].name, DEFAULT_PROVIDER_NAME);
        assert_eq!(state.profiles[0].api_key, "sk-legacy");
        assert_eq!(
            state.profiles[0].endpoint_settings.base_url,
            "https://proxy.example/v1"
        );

        drop(db);
        remove_temp_test_db(db_path);
    }

    #[test]
    fn provider_profiles_fall_back_to_first_profile_when_active_is_missing() {
        let (db, db_path) = temp_test_db("astro-studio-provider-active-test");
        let profiles = vec![
            ModelProviderProfile {
                id: "provider-a".to_string(),
                name: "Provider A".to_string(),
                api_key: "sk-a".to_string(),
                endpoint_settings: default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2),
            },
            ModelProviderProfile {
                id: "provider-b".to_string(),
                name: "Provider B".to_string(),
                api_key: "sk-b".to_string(),
                endpoint_settings: default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2),
            },
        ];
        db.set_setting(
            &model_provider_profiles_key(ENGINE_GPT_IMAGE_2),
            &serde_json::to_string(&profiles).unwrap(),
        )
        .unwrap();
        db.set_setting(
            &model_active_provider_key(ENGINE_GPT_IMAGE_2),
            "missing-provider",
        )
        .unwrap();

        let state = read_model_provider_profiles_state(&db, ENGINE_GPT_IMAGE_2).unwrap();

        assert_eq!(state.active_provider_id, "provider-a");

        drop(db);
        remove_temp_test_db(db_path);
    }

    #[test]
    fn saving_provider_profiles_normalizes_keys_and_gemini_edit_url() {
        let (db, db_path) = temp_test_db("astro-studio-provider-save-test");
        let state = ModelProviderProfilesState {
            active_provider_id: "gemini-provider".to_string(),
            profiles: vec![ModelProviderProfile {
                id: "gemini-provider".to_string(),
                name: "  Gemini Gateway  ".to_string(),
                api_key: " Bearer gemini-key\n".to_string(),
                endpoint_settings: EndpointSettings {
                    mode: ENDPOINT_MODE_FULL_URL.to_string(),
                    base_url: " ".to_string(),
                    generation_url: " https://gateway.example/generate ".to_string(),
                    edit_url: " https://gateway.example/ignored-edit ".to_string(),
                },
            }],
        };

        save_model_provider_profiles_state(&db, ENGINE_NANO_BANANA, state).unwrap();
        let saved = read_model_provider_profiles_state(&db, ENGINE_NANO_BANANA).unwrap();

        assert_eq!(saved.active_provider_id, "gemini-provider");
        assert_eq!(saved.profiles[0].name, "Gemini Gateway");
        assert_eq!(saved.profiles[0].api_key, "gemini-key");
        assert_eq!(
            saved.profiles[0].endpoint_settings.base_url,
            DEFAULT_GEMINI_MODELS_URL
        );
        assert_eq!(
            saved.profiles[0].endpoint_settings.edit_url,
            "https://gateway.example/generate"
        );

        drop(db);
        remove_temp_test_db(db_path);
    }

    #[test]
    fn legacy_api_key_helpers_update_active_provider_profile() {
        let (db, db_path) = temp_test_db("astro-studio-provider-key-compat-test");
        let state = ModelProviderProfilesState {
            active_provider_id: "provider-b".to_string(),
            profiles: vec![
                ModelProviderProfile {
                    id: "provider-a".to_string(),
                    name: "Provider A".to_string(),
                    api_key: "sk-a".to_string(),
                    endpoint_settings: default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2),
                },
                ModelProviderProfile {
                    id: "provider-b".to_string(),
                    name: "Provider B".to_string(),
                    api_key: "sk-b".to_string(),
                    endpoint_settings: default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2),
                },
            ],
        };
        save_model_provider_profiles_state(&db, ENGINE_GPT_IMAGE_2, state).unwrap();

        save_model_api_key_value(&db, ENGINE_GPT_IMAGE_2, "Bearer sk-active").unwrap();

        let saved = read_model_provider_profiles_state(&db, ENGINE_GPT_IMAGE_2).unwrap();
        assert_eq!(
            read_model_api_key(&db, ENGINE_GPT_IMAGE_2).unwrap(),
            Some("sk-active".to_string())
        );
        assert_eq!(saved.profiles[0].api_key, "sk-a");
        assert_eq!(saved.profiles[1].api_key, "sk-active");

        drop(db);
        remove_temp_test_db(db_path);
    }

    #[test]
    fn endpoint_resolution_uses_active_provider_profile() {
        let (db, db_path) = temp_test_db("astro-studio-provider-endpoint-compat-test");
        let mut provider_a_settings = default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2);
        provider_a_settings.base_url = "https://provider-a.example/v1".to_string();
        let mut provider_b_settings = default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2);
        provider_b_settings.base_url = "https://provider-b.example/v1".to_string();

        let state = ModelProviderProfilesState {
            active_provider_id: "provider-b".to_string(),
            profiles: vec![
                ModelProviderProfile {
                    id: "provider-a".to_string(),
                    name: "Provider A".to_string(),
                    api_key: "sk-a".to_string(),
                    endpoint_settings: provider_a_settings,
                },
                ModelProviderProfile {
                    id: "provider-b".to_string(),
                    name: "Provider B".to_string(),
                    api_key: "sk-b".to_string(),
                    endpoint_settings: provider_b_settings,
                },
            ],
        };
        save_model_provider_profiles_state(&db, ENGINE_GPT_IMAGE_2, state).unwrap();

        let settings = read_model_endpoint_settings(&db, ENGINE_GPT_IMAGE_2).unwrap();
        assert_eq!(settings.base_url, "https://provider-b.example/v1");

        drop(db);
        remove_temp_test_db(db_path);
    }

    #[test]
    fn deleting_the_last_provider_is_allowed_and_clears_active_id() {
        let (db, db_path) = temp_test_db("astro-studio-provider-delete-empty-test");
        let state = ModelProviderProfilesState {
            active_provider_id: "provider-a".to_string(),
            profiles: vec![ModelProviderProfile {
                id: "provider-a".to_string(),
                name: "Provider A".to_string(),
                api_key: "sk-a".to_string(),
                endpoint_settings: default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2),
            }],
        };
        save_model_provider_profiles_state(&db, ENGINE_GPT_IMAGE_2, state).unwrap();

        let mut state = read_model_provider_profiles_state(&db, ENGINE_GPT_IMAGE_2).unwrap();
        state.profiles.retain(|profile| profile.id != "provider-a");
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
        let saved = save_model_provider_profiles_state(&db, ENGINE_GPT_IMAGE_2, state).unwrap();

        assert!(saved.profiles.is_empty());
        assert_eq!(saved.active_provider_id, "");

        drop(db);
        remove_temp_test_db(db_path);
    }
}
