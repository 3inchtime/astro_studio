use super::*;
use crate::db::Database;
use crate::model_registry::model_setting_key;

fn temp_test_db(prefix: &str) -> (Database, std::path::PathBuf) {
    let db_path = std::env::temp_dir().join(format!("{prefix}-{}.sqlite", uuid::Uuid::new_v4()));
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
fn saving_provider_keeps_secret_out_of_database_bytes() {
    let dir = std::env::temp_dir().join(format!("vault-regression-{}", uuid::Uuid::new_v4()));
    let path = dir.join("test.db");
    let db = Database::open(&path).unwrap();
    db.run_migrations().unwrap();
    save_model_api_key_value(&db, ENGINE_GPT_IMAGE_2, "vault-test-distinct-secret-321").unwrap();
    let conn = db.conn.lock().unwrap();
    let values: Vec<String> = conn
        .prepare("SELECT value FROM settings")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(!values.join(" ").contains("vault-test-distinct-secret-321"));
    drop(conn);
    drop(db);
    std::fs::remove_dir_all(dir).unwrap();
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

    save_model_api_key_value(&db, ENGINE_GPT_IMAGE_2, " Bearer sk-proj-valid-token\n").unwrap();

    assert_eq!(
        read_model_api_key(&db, ENGINE_GPT_IMAGE_2).unwrap(),
        Some("sk-proj-valid-token".to_string())
    );
    assert_eq!(db.get_setting(SETTING_API_KEY).unwrap(), None);

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
fn exact_provider_api_key_lookup_ignores_active_profile_switch() {
    let (db, db_path) = temp_test_db("astro-studio-provider-exact-key-test");
    let state = ModelProviderProfilesState {
        active_provider_id: "provider-a".to_string(),
        profiles: vec![
            ModelProviderProfile {
                id: "provider-a".to_string(),
                name: "Provider A".to_string(),
                api_key: "sk-a-current".to_string(),
                endpoint_settings: default_endpoint_settings_for_model(ENGINE_NANO_BANANA),
            },
            ModelProviderProfile {
                id: "provider-b".to_string(),
                name: "Provider B".to_string(),
                api_key: "sk-b-current".to_string(),
                endpoint_settings: default_endpoint_settings_for_model(ENGINE_NANO_BANANA),
            },
        ],
    };
    save_model_provider_profiles_state(&db, ENGINE_NANO_BANANA, state).unwrap();

    let mut switched = read_model_provider_profiles_state(&db, ENGINE_NANO_BANANA).unwrap();
    switched.active_provider_id = "provider-b".to_string();
    save_model_provider_profiles_state(&db, ENGINE_NANO_BANANA, switched).unwrap();

    assert_eq!(
        resolve_profile(&db, GEMINI_MODEL_NANO_BANANA, Some("provider-a"))
            .unwrap()
            .api_key,
        "sk-a-current"
    );

    drop(db);
    remove_temp_test_db(db_path);
}

#[test]
fn exact_provider_api_key_lookup_rejects_missing_profile_without_fallback() {
    let (db, db_path) = temp_test_db("astro-studio-provider-missing-key-test");
    let state = ModelProviderProfilesState {
        active_provider_id: "provider-a".to_string(),
        profiles: vec![ModelProviderProfile {
            id: "provider-a".to_string(),
            name: "Provider A".to_string(),
            api_key: "sk-active".to_string(),
            endpoint_settings: default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2),
        }],
    };
    save_model_provider_profiles_state(&db, ENGINE_GPT_IMAGE_2, state).unwrap();

    let error = resolve_profile(&db, ENGINE_GPT_IMAGE_2, Some("missing-provider"))
        .expect_err("missing profile must not fall back to the active profile");

    assert!(matches!(
        error,
        AppError::ProviderProfileNotFound { model }
            if model == ENGINE_GPT_IMAGE_2
    ));

    drop(db);
    remove_temp_test_db(db_path);
}

#[test]
fn exact_provider_lookup_keeps_empty_key_on_selected_profile() {
    let (db, db_path) = temp_test_db("astro-studio-provider-empty-key-test");
    let state = ModelProviderProfilesState {
        active_provider_id: "provider-with-key".to_string(),
        profiles: vec![
            ModelProviderProfile {
                id: "provider-empty".to_string(),
                name: "Provider Empty".to_string(),
                api_key: "   ".to_string(),
                endpoint_settings: default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2),
            },
            ModelProviderProfile {
                id: "provider-with-key".to_string(),
                name: "Provider With Key".to_string(),
                api_key: "sk-active".to_string(),
                endpoint_settings: default_endpoint_settings_for_model(ENGINE_GPT_IMAGE_2),
            },
        ],
    };
    save_model_provider_profiles_state(&db, ENGINE_GPT_IMAGE_2, state).unwrap();

    assert!(
        resolve_profile(&db, ENGINE_GPT_IMAGE_2, Some("provider-empty"))
            .unwrap()
            .api_key
            .trim()
            .is_empty()
    );

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

#[test]
fn retired_official_google_endpoints_migrate_without_changing_custom_profiles() {
    let defaults = default_endpoint_settings_for_model(ENGINE_NANO_BANANA_PRO);
    let old = "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-pro-image-preview:generateContent";
    let settings = EndpointSettings {
        mode: "full_url".into(),
        generation_url: old.into(),
        edit_url: old.into(),
        ..defaults
    };
    let state = normalize_provider_profiles_state(
        ENGINE_NANO_BANANA_PRO,
        ModelProviderProfilesState {
            active_provider_id: "company".into(),
            profiles: vec![ModelProviderProfile {
                id: "company".into(),
                name: "Company".into(),
                api_key: "test-key".into(),
                endpoint_settings: settings.clone(),
            }],
        },
    )
    .unwrap();
    assert_eq!(state.active_provider_id, "company");
    assert_eq!(state.profiles[0].api_key, "test-key");
    assert_eq!(state.profiles[0].name, "Company");
    assert_eq!(
        state.profiles[0].endpoint_settings.generation_url,
        old.replace("-preview", "")
    );
    let custom = EndpointSettings {
        generation_url: "https://custom.test/gemini-3-pro-image-preview:generateContent".into(),
        ..settings
    };
    assert_eq!(
        normalize_provider_endpoint_settings(ENGINE_NANO_BANANA_PRO, &custom).generation_url,
        custom.generation_url
    );
}

#[test]
fn metadata_only_updates_do_not_restore_a_replaced_or_cleared_credential() {
    let db_path =
        std::env::temp_dir().join(format!("astro-metadata-keep-{}.db", uuid::Uuid::new_v4()));
    let db = Database::open(&db_path).unwrap();
    db.run_migrations().unwrap();
    save_model_api_key_value(&db, ENGINE_GPT_IMAGE_2, "original-secret").unwrap();
    let mut stale = read_model_provider_profiles_state(&db, ENGINE_GPT_IMAGE_2).unwrap();
    stale.profiles[0].name = "Metadata edit".into();
    for updated in ["replacement-secret", ""] {
        save_model_api_key_value(&db, ENGINE_GPT_IMAGE_2, updated).unwrap();
        let result =
            save_model_provider_metadata_state(&db, ENGINE_GPT_IMAGE_2, stale.clone()).unwrap();
        assert_eq!(
            read_model_api_key(&db, ENGINE_GPT_IMAGE_2)
                .unwrap()
                .unwrap_or_default(),
            updated
        );
        assert_eq!(result.profiles[0].api_key, updated);
    }
    drop(db);
    remove_temp_test_db(db_path);
}

#[test]
fn only_fresh_installations_default_to_flare() {
    let path = std::env::temp_dir().join(format!(
        "astro-model-default-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let db = Database::open(&path).unwrap();
    db.run_migrations().unwrap();
    assert_eq!(current_image_model(&db).unwrap(), ENGINE_GPT_IMAGE_26_FLARE);
    db.set_setting(SETTING_API_KEY, "legacy-key").unwrap();
    assert_eq!(current_image_model(&db).unwrap(), ENGINE_GPT_IMAGE_2);
    db.set_setting(SETTING_IMAGE_MODEL, ENGINE_NANO_BANANA)
        .unwrap();
    assert_eq!(current_image_model(&db).unwrap(), ENGINE_NANO_BANANA);
    drop(db);
    let _ = std::fs::remove_file(path);
}
