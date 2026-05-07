# Model Provider Profiles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add user-defined provider profiles per image model, with one active provider used by generation and edit requests.

**Architecture:** Store provider profile state as model-scoped JSON in the existing `settings` table, while keeping legacy model API key and endpoint commands mapped to the active provider. The frontend will expose provider profile management in the existing Settings model tab and keep the Generate page using the selected model only.

**Tech Stack:** Tauri 2 IPC, Rust, rusqlite key/value settings, React 19, TypeScript, Vitest, Testing Library, Tailwind CSS v4, lucide-react.

---

## File Structure

- Modify `src/types/index.ts`: add `ModelProviderProfile` and `ModelProviderProfilesState`.
- Modify `src/lib/api.ts`: add provider profile IPC wrappers.
- Modify `src/lib/api.test.ts`: verify provider profile IPC payloads.
- Create `src/lib/modelProviderProfiles.ts`: frontend-only helpers for default provider state, active provider selection, profile updates, and deletion.
- Create `src/lib/modelProviderProfiles.test.ts`: unit tests for frontend provider state helpers.
- Modify `src/components/settings/ModelSettingsPanel.tsx`: replace single key/endpoint controls with provider list plus selected provider editor.
- Create `src/components/settings/ModelSettingsPanel.test.tsx`: component-level UI tests for provider rows and editor actions.
- Modify `src/pages/SettingsPage.tsx`: load, edit, save, activate, create, and delete provider profiles for the selected model.
- Modify `src/pages/SettingsPage.test.tsx`: integration tests for provider profile loading, stale responses, save, create, delete, and activation.
- Modify `src/locales/*.json`: add provider management copy.
- Modify `src-tauri/src/models.rs`: add provider profile structs and settings key constants.
- Modify `src-tauri/src/lib.rs`: add provider profile helpers, IPC commands, legacy compatibility mapping, and active-provider generation resolution.

## Task 1: Frontend Types And IPC Wrappers

**Files:**
- Modify: `src/types/index.ts`
- Modify: `src/lib/api.ts`
- Modify: `src/lib/api.test.ts`

- [ ] **Step 1: Write the failing API wrapper test**

Add these imports to the existing import list in `src/lib/api.test.ts`:

```ts
  createModelProviderProfile,
  deleteModelProviderProfile,
  getModelProviderProfiles,
  saveModelProviderProfiles,
  setActiveModelProvider,
```

Add this test block after `describe("api log commands", ...)`:

```ts
describe("api model provider profile commands", () => {
  beforeEach(() => {
    tauriApi.invoke.mockReset();
  });

  it("wraps model provider profile IPC commands", async () => {
    const state = {
      active_provider_id: "provider-1",
      profiles: [
        {
          id: "provider-1",
          name: "OpenAI Official",
          api_key: "sk-provider-1",
          endpoint_settings: {
            mode: "base_url",
            base_url: "https://api.openai.com/v1",
            generation_url: "https://api.openai.com/v1/images/generations",
            edit_url: "https://api.openai.com/v1/images/edits",
          },
        },
      ],
    };

    tauriApi.invoke.mockResolvedValue(state);

    await getModelProviderProfiles("gpt-image-2");
    await saveModelProviderProfiles("gpt-image-2", state);
    await createModelProviderProfile("gpt-image-2", "Company Gateway");
    await deleteModelProviderProfile("gpt-image-2", "provider-1");
    await setActiveModelProvider("gpt-image-2", "provider-1");

    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      1,
      "get_model_provider_profiles",
      { model: "gpt-image-2" },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      2,
      "save_model_provider_profiles",
      {
        model: "gpt-image-2",
        activeProviderId: "provider-1",
        profiles: state.profiles,
      },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      3,
      "create_model_provider_profile",
      { model: "gpt-image-2", name: "Company Gateway" },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      4,
      "delete_model_provider_profile",
      { model: "gpt-image-2", providerId: "provider-1" },
    );
    expect(tauriApi.invoke).toHaveBeenNthCalledWith(
      5,
      "set_active_model_provider",
      { model: "gpt-image-2", providerId: "provider-1" },
    );
  });
});
```

- [ ] **Step 2: Run the API test to verify it fails**

Run:

```bash
npm test -- src/lib/api.test.ts
```

Expected: FAIL because the new wrapper exports do not exist.

- [ ] **Step 3: Add shared TypeScript types**

Add this block after `EndpointSettings` in `src/types/index.ts`:

```ts
export interface ModelProviderProfile {
  id: string;
  name: string;
  api_key: string;
  endpoint_settings: EndpointSettings;
}

export interface ModelProviderProfilesState {
  active_provider_id: string;
  profiles: ModelProviderProfile[];
}
```

- [ ] **Step 4: Add API wrappers**

Update the type imports in `src/lib/api.ts`:

```ts
  ModelProviderProfile,
  ModelProviderProfilesState,
```

Add these functions after `saveModelEndpointSettings`:

```ts
export async function getModelProviderProfiles(
  model: ImageModel,
): Promise<ModelProviderProfilesState> {
  return invoke("get_model_provider_profiles", { model });
}

export async function saveModelProviderProfiles(
  model: ImageModel,
  state: ModelProviderProfilesState,
): Promise<ModelProviderProfilesState> {
  return invoke("save_model_provider_profiles", {
    model,
    activeProviderId: state.active_provider_id,
    profiles: state.profiles,
  });
}

export async function createModelProviderProfile(
  model: ImageModel,
  name: string,
): Promise<ModelProviderProfilesState> {
  return invoke("create_model_provider_profile", { model, name });
}

export async function deleteModelProviderProfile(
  model: ImageModel,
  providerId: string,
): Promise<ModelProviderProfilesState> {
  return invoke("delete_model_provider_profile", { model, providerId });
}

export async function setActiveModelProvider(
  model: ImageModel,
  providerId: string,
): Promise<ModelProviderProfilesState> {
  return invoke("set_active_model_provider", { model, providerId });
}

export type { ModelProviderProfile, ModelProviderProfilesState };
```

- [ ] **Step 5: Run the API test to verify it passes**

Run:

```bash
npm test -- src/lib/api.test.ts
```

Expected: PASS for `src/lib/api.test.ts`.

- [ ] **Step 6: Commit**

```bash
git add src/types/index.ts src/lib/api.ts src/lib/api.test.ts
git commit -m "feat: add provider profile IPC wrappers"
```

## Task 2: Backend Provider Profile State Helpers

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing backend helper tests**

Add these tests inside the existing `#[cfg(test)] mod tests` in `src-tauri/src/lib.rs`:

```rust
    fn temp_test_db(prefix: &str) -> (Database, std::path::PathBuf) {
        let db_path = std::env::temp_dir().join(format!(
            "{prefix}-{}.sqlite",
            uuid::Uuid::new_v4()
        ));
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
```

- [ ] **Step 2: Run backend tests to verify they fail**

Run:

```bash
cargo test provider_profiles --lib
```

Expected: FAIL because `ModelProviderProfile`, `ModelProviderProfilesState`, provider constants, and helper functions are undefined.

- [ ] **Step 3: Add Rust model structs and constants**

In `src-tauri/src/models.rs`, add constants near the existing settings constants:

```rust
pub const SETTING_MODEL_PROVIDER_PROFILES_PREFIX: &str = "model_provider_profiles";
pub const SETTING_MODEL_ACTIVE_PROVIDER_PREFIX: &str = "model_active_provider";
pub const DEFAULT_PROVIDER_ID: &str = "default";
pub const DEFAULT_PROVIDER_NAME: &str = "Default";
pub const NEW_PROVIDER_NAME: &str = "New Provider";
```

Update `EndpointSettings` to derive `PartialEq`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EndpointSettings {
    pub mode: String,
    pub base_url: String,
    pub generation_url: String,
    pub edit_url: String,
}
```

Add provider structs after `EndpointSettings`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelProviderProfile {
    pub id: String,
    pub name: String,
    pub api_key: String,
    pub endpoint_settings: EndpointSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelProviderProfilesState {
    pub active_provider_id: String,
    pub profiles: Vec<ModelProviderProfile>,
}
```

- [ ] **Step 4: Add backend provider profile helpers**

In `src-tauri/src/lib.rs`, rename the current `read_model_endpoint_settings` function to `read_legacy_model_endpoint_settings`, and update existing call sites temporarily:

```rust
fn read_legacy_model_endpoint_settings(
    db: &Database,
    model: &str,
) -> Result<EndpointSettings, String> {
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
```

Rename the current `read_model_api_key` to `read_legacy_model_api_key`:

```rust
fn read_legacy_model_api_key(db: &Database, model: &str) -> Result<Option<String>, String> {
    Ok(
        get_model_setting(db, model, SETTING_API_KEY, Some(SETTING_API_KEY))?
            .map(|key| normalize_api_key_for_storage(&key)),
    )
}
```

Add these helpers after `normalize_api_key_for_storage`:

```rust
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

fn normalize_provider_name(name: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        "Provider".to_string()
    } else {
        name.to_string()
    }
}

fn normalize_provider_endpoint_settings(model: &str, settings: &EndpointSettings) -> EndpointSettings {
    let defaults = default_endpoint_settings_for_model(model);
    let mode = normalize_endpoint_mode(&settings.mode).to_string();
    let base_url = endpoint_value_or_default(Some(settings.base_url.clone()), &defaults.base_url);
    let generation_url =
        endpoint_value_or_default(Some(settings.generation_url.clone()), &defaults.generation_url);
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

fn default_provider_profile_for_model(
    db: &Database,
    model: &str,
) -> Result<ModelProviderProfile, String> {
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
) -> Result<ModelProviderProfilesState, String> {
    let normalized_model = normalize_image_model(model);
    let mut seen = std::collections::HashSet::new();
    let mut profiles = Vec::with_capacity(state.profiles.len());

    for profile in state.profiles {
        let id = profile.id.trim().to_string();
        if id.is_empty() {
            return Err("Provider id cannot be empty.".to_string());
        }
        if !seen.insert(id.clone()) {
            return Err(format!("Duplicate provider id: {id}"));
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

    if profiles.is_empty() {
        return Err("At least one provider profile is required.".to_string());
    }

    let active_provider_id = if profiles
        .iter()
        .any(|profile| profile.id == state.active_provider_id)
    {
        state.active_provider_id
    } else {
        profiles[0].id.clone()
    };

    Ok(ModelProviderProfilesState {
        active_provider_id,
        profiles,
    })
}

fn read_model_provider_profiles_state(
    db: &Database,
    model: &str,
) -> Result<ModelProviderProfilesState, String> {
    let normalized_model = normalize_image_model(model);
    let stored_profiles = db.get_setting(&model_provider_profiles_key(normalized_model))?;
    let profiles = stored_profiles
        .as_deref()
        .and_then(|value| serde_json::from_str::<Vec<ModelProviderProfile>>(value).ok())
        .filter(|profiles| !profiles.is_empty())
        .unwrap_or_else(|| vec![default_provider_profile_for_model(db, normalized_model).unwrap()]);
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

fn save_model_provider_profiles_state(
    db: &Database,
    model: &str,
    state: ModelProviderProfilesState,
) -> Result<ModelProviderProfilesState, String> {
    let normalized_model = normalize_image_model(model);
    let state = normalize_provider_profiles_state(normalized_model, state)?;
    let profiles_json = serde_json::to_string(&state.profiles)
        .map_err(|e| format!("Serialize provider profiles failed: {}", e))?;

    db.set_setting(&model_provider_profiles_key(normalized_model), &profiles_json)?;
    db.set_setting(
        &model_active_provider_key(normalized_model),
        &state.active_provider_id,
    )?;

    Ok(state)
}

fn active_provider_profile_for_model(
    db: &Database,
    model: &str,
) -> Result<ModelProviderProfile, String> {
    let state = read_model_provider_profiles_state(db, model)?;
    state
        .profiles
        .into_iter()
        .find(|profile| profile.id == state.active_provider_id)
        .ok_or_else(|| "Active provider profile not found.".to_string())
}
```

- [ ] **Step 5: Run backend helper tests**

Run:

```bash
cargo test provider_profiles --lib
```

Expected: PASS for the three provider profile helper tests.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/models.rs src-tauri/src/lib.rs
git commit -m "feat: add backend provider profile state"
```

## Task 3: Backend Compatibility Commands And Active Provider Resolution

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing compatibility tests**

Add these tests inside the existing `#[cfg(test)] mod tests` in `src-tauri/src/lib.rs`:

```rust
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
        assert_eq!(read_model_api_key(&db, ENGINE_GPT_IMAGE_2).unwrap(), Some("sk-active".to_string()));
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

        assert_eq!(
            resolve_image_endpoint_url_for_model(
                &db,
                ENGINE_GPT_IMAGE_2,
                ImageEndpointKind::Generate,
            )
            .unwrap(),
            "https://provider-b.example/v1/images/generations"
        );

        drop(db);
        remove_temp_test_db(db_path);
    }
```

- [ ] **Step 2: Run compatibility tests to verify they fail**

Run:

```bash
cargo test active_provider --lib
```

Expected: FAIL because `read_model_api_key`, `save_model_api_key_value`, and endpoint resolution still use legacy single-model settings.

- [ ] **Step 3: Map compatibility helpers to the active provider**

In `src-tauri/src/lib.rs`, replace `read_model_api_key` with:

```rust
fn read_model_api_key(db: &Database, model: &str) -> Result<Option<String>, String> {
    let profile = active_provider_profile_for_model(db, model)?;
    Ok(Some(profile.api_key).filter(|key| !key.trim().is_empty()))
}
```

Add a new active endpoint reader:

```rust
fn read_model_endpoint_settings(db: &Database, model: &str) -> Result<EndpointSettings, String> {
    Ok(active_provider_profile_for_model(db, model)?.endpoint_settings)
}
```

Update `read_endpoint_settings` to call the active reader:

```rust
fn read_endpoint_settings(db: &Database) -> Result<EndpointSettings, String> {
    let model = current_image_model(db)?;
    read_model_endpoint_settings(db, model)
}
```

Replace `save_model_api_key_value` with active-profile updating logic:

```rust
fn save_model_api_key_value(db: &Database, model: &str, key: &str) -> Result<(), String> {
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
```

Replace `save_model_endpoint_settings_value` with active-profile updating logic:

```rust
fn save_model_endpoint_settings_value(
    db: &Database,
    model: &str,
    mode: &str,
    base_url: &str,
    generation_url: &str,
    edit_url: &str,
) -> Result<(), String> {
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
```

- [ ] **Step 4: Add provider profile IPC commands**

Add command functions near the existing settings commands:

```rust
#[tauri::command]
fn get_model_provider_profiles(
    db: tauri::State<'_, Database>,
    model: String,
) -> Result<ModelProviderProfilesState, String> {
    read_model_provider_profiles_state(db.inner(), &model)
}

#[tauri::command]
fn save_model_provider_profiles(
    db: tauri::State<'_, Database>,
    model: String,
    active_provider_id: String,
    profiles: Vec<ModelProviderProfile>,
) -> Result<ModelProviderProfilesState, String> {
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
fn create_model_provider_profile(
    db: tauri::State<'_, Database>,
    model: String,
    name: String,
) -> Result<ModelProviderProfilesState, String> {
    let normalized_model = normalize_image_model(&model);
    let mut state = read_model_provider_profiles_state(db.inner(), normalized_model)?;
    let provider_id = uuid::Uuid::new_v4().to_string();
    state.active_provider_id = provider_id.clone();
    state.profiles.push(ModelProviderProfile {
        id: provider_id,
        name: normalize_provider_name(&name),
        api_key: String::new(),
        endpoint_settings: default_endpoint_settings_for_model(normalized_model),
    });
    save_model_provider_profiles_state(db.inner(), normalized_model, state)
}

#[tauri::command]
fn delete_model_provider_profile(
    db: tauri::State<'_, Database>,
    model: String,
    provider_id: String,
) -> Result<ModelProviderProfilesState, String> {
    let normalized_model = normalize_image_model(&model);
    let mut state = read_model_provider_profiles_state(db.inner(), normalized_model)?;
    if state.profiles.len() <= 1 {
        return Err("At least one provider profile is required.".to_string());
    }
    state.profiles.retain(|profile| profile.id != provider_id);
    if !state
        .profiles
        .iter()
        .any(|profile| profile.id == state.active_provider_id)
    {
        state.active_provider_id = state.profiles[0].id.clone();
    }
    save_model_provider_profiles_state(db.inner(), normalized_model, state)
}

#[tauri::command]
fn set_active_model_provider(
    db: tauri::State<'_, Database>,
    model: String,
    provider_id: String,
) -> Result<ModelProviderProfilesState, String> {
    let normalized_model = normalize_image_model(&model);
    let mut state = read_model_provider_profiles_state(db.inner(), normalized_model)?;
    if !state.profiles.iter().any(|profile| profile.id == provider_id) {
        return Err("Provider profile not found.".to_string());
    }
    state.active_provider_id = provider_id;
    save_model_provider_profiles_state(db.inner(), normalized_model, state)
}
```

Register the commands in the `tauri::generate_handler!` list:

```rust
            get_model_provider_profiles,
            save_model_provider_profiles,
            create_model_provider_profile,
            delete_model_provider_profile,
            set_active_model_provider,
```

- [ ] **Step 5: Update missing-key error text**

In `generate_image` and `edit_image`, change:

```rust
.ok_or_else(|| "API key not set. Please set it in Settings.".to_string())?;
```

to:

```rust
.ok_or_else(|| "API key not set for the active provider. Please set it in Settings.".to_string())?;
```

- [ ] **Step 6: Run backend tests**

Run:

```bash
cargo test --lib
```

Expected: all library tests pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: resolve image settings from active provider"
```

## Task 4: Frontend Provider State Helpers

**Files:**
- Create: `src/lib/modelProviderProfiles.ts`
- Create: `src/lib/modelProviderProfiles.test.ts`

- [ ] **Step 1: Write failing frontend helper tests**

Create `src/lib/modelProviderProfiles.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import {
  DEFAULT_PROVIDER_ID,
  activeProviderForState,
  defaultProviderProfilesStateForModel,
  removeProviderFromState,
  updateProviderInState,
} from "./modelProviderProfiles";

describe("model provider profile helpers", () => {
  it("builds a default provider state from model defaults", () => {
    expect(defaultProviderProfilesStateForModel("gpt-image-2")).toEqual({
      active_provider_id: DEFAULT_PROVIDER_ID,
      profiles: [
        {
          id: DEFAULT_PROVIDER_ID,
          name: "Default",
          api_key: "",
          endpoint_settings: {
            mode: "base_url",
            base_url: "https://api.openai.com/v1",
            generation_url: "https://api.openai.com/v1/images/generations",
            edit_url: "https://api.openai.com/v1/images/edits",
          },
        },
      ],
    });
  });

  it("updates one provider without changing the active provider", () => {
    const state = defaultProviderProfilesStateForModel("gpt-image-2");

    expect(
      updateProviderInState(state, DEFAULT_PROVIDER_ID, (profile) => ({
        ...profile,
        name: "OpenAI Official",
      })),
    ).toMatchObject({
      active_provider_id: DEFAULT_PROVIDER_ID,
      profiles: [{ name: "OpenAI Official" }],
    });
  });

  it("removes an active provider and activates the first remaining provider", () => {
    const state = {
      active_provider_id: "provider-b",
      profiles: [
        {
          ...defaultProviderProfilesStateForModel("gpt-image-2").profiles[0],
          id: "provider-a",
          name: "Provider A",
        },
        {
          ...defaultProviderProfilesStateForModel("gpt-image-2").profiles[0],
          id: "provider-b",
          name: "Provider B",
        },
      ],
    };

    expect(removeProviderFromState(state, "provider-b")).toMatchObject({
      active_provider_id: "provider-a",
      profiles: [{ id: "provider-a" }],
    });
  });

  it("returns the active provider or the first provider", () => {
    const state = {
      active_provider_id: "missing",
      profiles: defaultProviderProfilesStateForModel("gpt-image-2").profiles,
    };

    expect(activeProviderForState(state)?.id).toBe(DEFAULT_PROVIDER_ID);
  });
});
```

- [ ] **Step 2: Run helper tests to verify they fail**

Run:

```bash
npm test -- src/lib/modelProviderProfiles.test.ts
```

Expected: FAIL because `src/lib/modelProviderProfiles.ts` does not exist.

- [ ] **Step 3: Implement provider state helpers**

Create `src/lib/modelProviderProfiles.ts`:

```ts
import type {
  ImageModel,
  ModelProviderProfile,
  ModelProviderProfilesState,
} from "../types";
import { defaultEndpointSettingsForModel } from "./settingsEndpoints";

export const DEFAULT_PROVIDER_ID = "default";
export const DEFAULT_PROVIDER_NAME = "Default";
export const NEW_PROVIDER_NAME = "New Provider";

export function defaultProviderProfileForModel(
  model: ImageModel,
): ModelProviderProfile {
  return {
    id: DEFAULT_PROVIDER_ID,
    name: DEFAULT_PROVIDER_NAME,
    api_key: "",
    endpoint_settings: defaultEndpointSettingsForModel(model),
  };
}

export function defaultProviderProfilesStateForModel(
  model: ImageModel,
): ModelProviderProfilesState {
  return {
    active_provider_id: DEFAULT_PROVIDER_ID,
    profiles: [defaultProviderProfileForModel(model)],
  };
}

export function activeProviderForState(
  state: ModelProviderProfilesState,
): ModelProviderProfile | undefined {
  return (
    state.profiles.find((profile) => profile.id === state.active_provider_id) ??
    state.profiles[0]
  );
}

export function providerForState(
  state: ModelProviderProfilesState,
  providerId: string,
): ModelProviderProfile | undefined {
  return state.profiles.find((profile) => profile.id === providerId);
}

export function updateProviderInState(
  state: ModelProviderProfilesState,
  providerId: string,
  update: (profile: ModelProviderProfile) => ModelProviderProfile,
): ModelProviderProfilesState {
  return {
    ...state,
    profiles: state.profiles.map((profile) =>
      profile.id === providerId ? update(profile) : profile,
    ),
  };
}

export function removeProviderFromState(
  state: ModelProviderProfilesState,
  providerId: string,
): ModelProviderProfilesState {
  if (state.profiles.length <= 1) {
    return state;
  }

  const profiles = state.profiles.filter((profile) => profile.id !== providerId);
  const activeProviderStillExists = profiles.some(
    (profile) => profile.id === state.active_provider_id,
  );

  return {
    active_provider_id: activeProviderStillExists
      ? state.active_provider_id
      : profiles[0].id,
    profiles,
  };
}
```

- [ ] **Step 4: Run helper tests**

Run:

```bash
npm test -- src/lib/modelProviderProfiles.test.ts
```

Expected: PASS for `src/lib/modelProviderProfiles.test.ts`.

- [ ] **Step 5: Commit**

```bash
git add src/lib/modelProviderProfiles.ts src/lib/modelProviderProfiles.test.ts
git commit -m "feat: add provider profile state helpers"
```

## Task 5: Provider Profile Settings Panel UI

**Files:**
- Create: `src/components/settings/ModelSettingsPanel.test.tsx`
- Modify: `src/components/settings/ModelSettingsPanel.tsx`

- [ ] **Step 1: Write failing component UI tests**

Create `src/components/settings/ModelSettingsPanel.test.tsx`:

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ModelSettingsPanel } from "./ModelSettingsPanel";
import type { ModelProviderProfilesState } from "../../types";

const baseEndpoint = {
  mode: "base_url" as const,
  base_url: "https://api.openai.com/v1",
  generation_url: "https://api.openai.com/v1/images/generations",
  edit_url: "https://api.openai.com/v1/images/edits",
};

const providerState: ModelProviderProfilesState = {
  active_provider_id: "provider-a",
  profiles: [
    {
      id: "provider-a",
      name: "OpenAI Official",
      api_key: "sk-openai",
      endpoint_settings: baseEndpoint,
    },
    {
      id: "provider-b",
      name: "Company Gateway",
      api_key: "sk-gateway",
      endpoint_settings: {
        ...baseEndpoint,
        mode: "full_url",
        generation_url: "https://gateway.example/generate",
        edit_url: "https://gateway.example/edit",
      },
    },
  ],
};

function renderPanel(overrides = {}) {
  const props = {
    t: ((key: string) => key) as never,
    imageModel: "gpt-image-2" as const,
    modelSaved: false,
    providerState,
    selectedProviderId: "provider-a",
    showKey: false,
    providerSaved: false,
    onSelectImageModel: vi.fn(),
    onSaveModel: vi.fn(),
    onSelectProvider: vi.fn(),
    onProviderNameChange: vi.fn(),
    onProviderApiKeyChange: vi.fn(),
    onShowKeyChange: vi.fn(),
    onProviderEndpointModeChange: vi.fn(),
    onProviderBaseUrlChange: vi.fn(),
    onProviderGenerationUrlChange: vi.fn(),
    onProviderEditUrlChange: vi.fn(),
    onCreateProvider: vi.fn(),
    onDeleteProvider: vi.fn(),
    onSetActiveProvider: vi.fn(),
    onSaveProvider: vi.fn(),
    ...overrides,
  };

  render(<ModelSettingsPanel {...props} />);
  return props;
}

describe("ModelSettingsPanel provider profiles", () => {
  it("renders provider rows and the selected provider editor", () => {
    renderPanel();

    expect(screen.getByText("settings.providers")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Select OpenAI Official provider" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Select Company Gateway provider" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByDisplayValue("OpenAI Official")).toBeInTheDocument();
    expect(screen.getByDisplayValue("https://api.openai.com/v1")).toBeInTheDocument();
  });

  it("routes provider actions through callbacks", () => {
    const props = renderPanel({ selectedProviderId: "provider-b" });

    fireEvent.click(screen.getByRole("button", { name: "settings.newProvider" }));
    fireEvent.click(screen.getByRole("button", { name: "Use Company Gateway provider" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete Company Gateway provider" }));
    fireEvent.change(screen.getByDisplayValue("Company Gateway"), {
      target: { value: "Renamed Gateway" },
    });
    fireEvent.click(screen.getByRole("button", { name: "settings.saveProvider" }));

    expect(props.onCreateProvider).toHaveBeenCalled();
    expect(props.onSetActiveProvider).toHaveBeenCalledWith("provider-b");
    expect(props.onDeleteProvider).toHaveBeenCalledWith("provider-b");
    expect(props.onProviderNameChange).toHaveBeenCalledWith("Renamed Gateway");
    expect(props.onSaveProvider).toHaveBeenCalled();
  });

  it("disables provider deletion when only one provider remains", () => {
    renderPanel({
      providerState: {
        active_provider_id: "provider-a",
        profiles: [providerState.profiles[0]],
      },
    });

    expect(
      screen.getByRole("button", { name: "Delete OpenAI Official provider" }),
    ).toBeDisabled();
  });
});
```

- [ ] **Step 2: Run the component test to verify it fails**

Run:

```bash
npm test -- src/components/settings/ModelSettingsPanel.test.tsx
```

Expected: FAIL because the component still expects single API key and endpoint props.

- [ ] **Step 3: Update `ModelSettingsPanel` props**

Replace the single key/endpoint props with provider props:

```ts
  providerState: ModelProviderProfilesState;
  selectedProviderId: string;
  showKey: boolean;
  providerSaved: boolean;
  onSelectProvider: (providerId: string) => void;
  onProviderNameChange: (name: string) => void;
  onProviderApiKeyChange: (apiKey: string) => void;
  onShowKeyChange: (showKey: boolean) => void;
  onProviderEndpointModeChange: (mode: EndpointMode) => void;
  onProviderBaseUrlChange: (url: string) => void;
  onProviderGenerationUrlChange: (url: string) => void;
  onProviderEditUrlChange: (url: string) => void;
  onCreateProvider: () => void;
  onDeleteProvider: (providerId: string) => void;
  onSetActiveProvider: (providerId: string) => void;
  onSaveProvider: () => void;
```

Keep existing model props:

```ts
  imageModel: ImageModel;
  modelSaved: boolean;
  onSelectImageModel: (model: ImageModel) => void;
  onSaveModel: () => void;
```

- [ ] **Step 4: Implement provider list plus editor**

In `ModelSettingsPanel.tsx`, import:

```ts
import { Check, Cpu, Eye, EyeOff, Globe, Key, Plus, Trash2 } from "lucide-react";
import type {
  EndpointMode,
  ImageModel,
  ModelProviderProfile,
} from "../../types";
```

Derive the selected provider at the start of the component:

```ts
  const selectedProvider =
    providerState.profiles.find((provider) => provider.id === selectedProviderId) ??
    providerState.profiles[0];
  const endpointSettings = selectedProvider.endpoint_settings;
  const apiKey = selectedProvider.api_key;
  const displayKey = showKey ? apiKey : (apiKey ? maskKey(apiKey) : "");
  const canDeleteProvider = providerState.profiles.length > 1;
```

Move `maskKey` from `SettingsPage.tsx` into `ModelSettingsPanel.tsx` or a shared helper:

```ts
function maskKey(key: string): string {
  if (key.length <= 8) return "sk-****";
  return key.slice(0, 3) + "..." + key.slice(-4);
}
```

Replace the old API Key and Endpoint sections with a providers area matching the spec:

```tsx
<div className="border-t border-border-subtle" />
<div className="grid gap-4 p-5 lg:grid-cols-[260px_minmax(0,1fr)] lg:items-start lg:gap-6">
  <div className="space-y-3">
    <div className="flex items-start justify-between gap-3">
      <div className="flex items-start gap-3">
        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[10px] border border-primary/10 bg-primary/5">
          <Globe size={14} className="text-primary" strokeWidth={2} />
        </div>
        <div>
          <h4 className="text-[13px] font-semibold text-foreground">{t("settings.providers")}</h4>
          <p className="mt-0.5 text-[11px] leading-relaxed text-muted/60">{t("settings.providersDesc")}</p>
        </div>
      </div>
      <button
        type="button"
        onClick={onCreateProvider}
        aria-label={t("settings.newProvider")}
        className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[8px] border border-border-subtle text-muted transition-all hover:border-border hover:text-foreground"
      >
        <Plus size={14} />
      </button>
    </div>

    <div className="grid gap-2">
      {providerState.profiles.map((provider) => {
        const selected = provider.id === selectedProvider.id;
        const active = provider.id === providerState.active_provider_id;
        return (
          <div
            key={provider.id}
            role="button"
            tabIndex={0}
            aria-pressed={selected}
            aria-label={`Select ${provider.name} provider`}
            onClick={() => onSelectProvider(provider.id)}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                onSelectProvider(provider.id);
              }
            }}
            className={`group rounded-[10px] border p-3 text-left transition-all ${
              selected
                ? "border-primary/30 bg-primary/6 shadow-card"
                : "border-border-subtle bg-subtle/20 hover:border-border hover:bg-subtle/35"
            }`}
          >
            <div className="flex items-start justify-between gap-3">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <span className={`h-2 w-2 rounded-full ${active ? "bg-primary" : "bg-muted/30"}`} />
                  <span className="truncate text-[12px] font-semibold text-foreground">{provider.name}</span>
                  {active && (
                    <span className="rounded-[6px] border border-primary/15 bg-primary/8 px-1.5 py-0.5 text-[10px] font-medium text-primary">
                      {t("settings.activeProvider")}
                    </span>
                  )}
                </div>
                <p className="mt-1 text-[10.5px] font-medium text-muted/55">
                  {provider.endpoint_settings.mode === "base_url"
                    ? t("settings.endpointBaseUrlMode")
                    : t("settings.endpointFullUrlMode")}
                </p>
                <p className="mt-1 truncate font-mono text-[10px] text-muted/45">
                  {provider.endpoint_settings.mode === "base_url"
                    ? provider.endpoint_settings.base_url
                    : provider.endpoint_settings.generation_url}
                </p>
              </div>
              <div className="flex shrink-0 items-center gap-1 opacity-100 sm:opacity-0 sm:transition-opacity sm:group-hover:opacity-100 sm:group-focus-within:opacity-100">
                {!active && (
                  <button
                    type="button"
                    aria-label={`Use ${provider.name} provider`}
                    onClick={(event) => {
                      event.stopPropagation();
                      onSetActiveProvider(provider.id);
                    }}
                    className="flex h-7 w-7 items-center justify-center rounded-[7px] text-muted/50 hover:bg-surface hover:text-primary"
                  >
                    <Check size={13} />
                  </button>
                )}
                <button
                  type="button"
                  disabled={!canDeleteProvider}
                  aria-label={`Delete ${provider.name} provider`}
                  onClick={(event) => {
                    event.stopPropagation();
                    if (canDeleteProvider) onDeleteProvider(provider.id);
                  }}
                  className={`flex h-7 w-7 items-center justify-center rounded-[7px] ${
                    canDeleteProvider
                      ? "text-muted/45 hover:bg-surface hover:text-error"
                      : "cursor-not-allowed text-muted/20"
                  }`}
                >
                  <Trash2 size={13} />
                </button>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  </div>

  <div className="min-w-0 space-y-4">
    <label className="grid gap-1.5">
      <span className="text-[11px] font-medium text-muted/70">{t("settings.providerName")}</span>
      <input
        type="text"
        value={selectedProvider.name}
        onChange={(event) => onProviderNameChange(event.target.value)}
        className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-subtle/30 px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
      />
    </label>

    <label className="grid gap-1.5">
      <span className="text-[11px] font-medium text-muted/70">{t("settings.apiKey")}</span>
      <div className="relative">
        <input
          type={showKey ? "text" : "password"}
          value={displayKey}
          onChange={(event) => onProviderApiKeyChange(event.target.value)}
          onFocus={() => { if (!showKey) onShowKeyChange(true); }}
          placeholder={t("settings.apiKeyPlaceholder")}
          className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-subtle/30 px-3 pr-9 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
        />
        <button
          type="button"
          onClick={() => onShowKeyChange(!showKey)}
          title={showKey ? t("settings.hideKey") : t("settings.showKey")}
          aria-label={showKey ? t("settings.hideKey") : t("settings.showKey")}
          className="absolute right-2.5 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-[6px] text-muted/40 transition-colors hover:bg-subtle hover:text-muted"
        >
          {showKey ? <EyeOff size={13} /> : <Eye size={13} />}
        </button>
      </div>
    </label>

    <div className="grid gap-2 rounded-[10px] border border-border-subtle bg-subtle/20 p-1 sm:grid-cols-2">
      {(["base_url", "full_url"] as EndpointMode[]).map((mode) => (
        <button
          key={mode}
          type="button"
          onClick={() => onProviderEndpointModeChange(mode)}
          className={`h-[34px] rounded-[8px] px-3 text-[12px] font-medium transition-all ${
            endpointSettings.mode === mode
              ? "bg-surface text-foreground shadow-card"
              : "text-muted/60 hover:text-foreground"
          }`}
        >
          {t(mode === "base_url" ? "settings.endpointBaseUrlMode" : "settings.endpointFullUrlMode")}
        </button>
      ))}
    </div>

    {endpointSettings.mode === "base_url" ? (
      <input
        type="text"
        value={endpointSettings.base_url}
        onChange={(event) => onProviderBaseUrlChange(event.target.value)}
        placeholder={defaultBaseUrlForModel(imageModel)}
        className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-subtle/30 px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
      />
    ) : (
      <div className="grid gap-2">
        <label className="grid gap-1.5">
          <span className="text-[11px] font-medium text-muted/70">{t("settings.generationUrl")}</span>
          <input
            type="text"
            value={endpointSettings.generation_url}
            onChange={(event) => onProviderGenerationUrlChange(event.target.value)}
            placeholder={defaultGenerationUrlForModel(imageModel)}
            className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-subtle/30 px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
          />
        </label>
        {modelSupportsEdit(imageModel) && !usesSharedEditEndpoint(imageModel) && (
          <label className="grid gap-1.5">
            <span className="text-[11px] font-medium text-muted/70">{t("settings.editUrl")}</span>
            <input
              type="text"
              value={endpointSettings.edit_url}
              onChange={(event) => onProviderEditUrlChange(event.target.value)}
              placeholder={defaultEditUrlForModel(imageModel)}
              className="h-[38px] w-full rounded-[10px] border border-border-subtle bg-subtle/30 px-3 text-[12px] text-foreground transition-all duration-200 placeholder:text-muted/40 focus:border-primary/25 focus:bg-surface focus:shadow-card focus:outline-none"
            />
          </label>
        )}
      </div>
    )}

    <div className="flex justify-end gap-2">
      <motion.button
        type="button"
        onClick={onSaveProvider}
        whileTap={{ scale: 0.97 }}
        className="flex h-[38px] shrink-0 items-center justify-center gap-1.5 rounded-[10px] border border-border-subtle px-4 text-[12px] font-medium text-muted transition-all hover:border-border hover:text-foreground lg:min-w-[116px]"
      >
        {providerSaved ? (<><Check size={13} className="text-success" /><span className="text-success">{t("settings.saved")}</span></>) : t("settings.saveProvider")}
      </motion.button>
      {selectedProvider.id !== providerState.active_provider_id && (
        <motion.button
          type="button"
          onClick={() => onSetActiveProvider(selectedProvider.id)}
          whileTap={{ scale: 0.97 }}
          className="flex h-[38px] shrink-0 items-center justify-center gap-1.5 rounded-[10px] border border-primary/20 bg-primary/8 px-4 text-[12px] font-medium text-primary transition-all hover:border-primary/30 hover:bg-primary/10 lg:min-w-[104px]"
        >
          <Check size={13} />
          {t("settings.activateProvider")}
        </motion.button>
      )}
    </div>
  </div>
</div>
```

Implement the editor form with the same input classes currently used for API key and endpoint. Bind values to:

```tsx
selectedProvider.name
displayKey
endpointSettings.mode
endpointSettings.base_url
endpointSettings.generation_url
endpointSettings.edit_url
```

Use callbacks:

```tsx
onProviderNameChange(e.target.value)
onProviderApiKeyChange(e.target.value)
onProviderEndpointModeChange(mode)
onProviderBaseUrlChange(e.target.value)
onProviderGenerationUrlChange(e.target.value)
onProviderEditUrlChange(e.target.value)
onSaveProvider()
onSetActiveProvider(selectedProvider.id)
```

- [ ] **Step 5: Run component test**

Run:

```bash
npm test -- src/components/settings/ModelSettingsPanel.test.tsx
```

Expected: PASS for component provider profile UI tests.

- [ ] **Step 6: Commit**

```bash
git add src/components/settings/ModelSettingsPanel.tsx src/components/settings/ModelSettingsPanel.test.tsx
git commit -m "feat: add provider profile settings UI"
```

## Task 6: Settings Page Provider Profile Integration

**Files:**
- Modify: `src/pages/SettingsPage.tsx`
- Modify: `src/pages/SettingsPage.test.tsx`

- [ ] **Step 1: Update settings page mocks and write failing integration tests**

In `src/pages/SettingsPage.test.tsx`, add new mock functions:

```ts
const getModelProviderProfiles = vi.fn();
const saveModelProviderProfiles = vi.fn();
const createModelProviderProfile = vi.fn();
const deleteModelProviderProfile = vi.fn();
const setActiveModelProvider = vi.fn();
```

Add them to the mocked `../lib/api` module:

```ts
  getModelProviderProfiles: (...args: unknown[]) =>
    getModelProviderProfiles(...args),
  saveModelProviderProfiles: (...args: unknown[]) =>
    saveModelProviderProfiles(...args),
  createModelProviderProfile: (...args: unknown[]) =>
    createModelProviderProfile(...args),
  deleteModelProviderProfile: (...args: unknown[]) =>
    deleteModelProviderProfile(...args),
  setActiveModelProvider: (...args: unknown[]) =>
    setActiveModelProvider(...args),
```

Reset them in `beforeEach`, and add a default provider state:

```ts
const openAiProviderState = {
  active_provider_id: "openai-official",
  profiles: [
    {
      id: "openai-official",
      name: "OpenAI Official",
      api_key: "openai-key",
      endpoint_settings: {
        mode: "base_url",
        base_url: "https://api.openai.com/v1",
        generation_url: "https://api.openai.com/v1/images/generations",
        edit_url: "https://api.openai.com/v1/images/edits",
      },
    },
    {
      id: "company-gateway",
      name: "Company Gateway",
      api_key: "gateway-key",
      endpoint_settings: {
        mode: "full_url",
        base_url: "https://gateway.example/v1",
        generation_url: "https://gateway.example/generate",
        edit_url: "https://gateway.example/edit",
      },
    },
  ],
};

const geminiProviderState = {
  active_provider_id: "gemini-official",
  profiles: [
    {
      id: "gemini-official",
      name: "Gemini Official",
      api_key: "gemini-key",
      endpoint_settings: {
        mode: "base_url",
        base_url: "https://generativelanguage.googleapis.com/v1beta/models",
        generation_url:
          "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent",
        edit_url:
          "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent",
      },
    },
  ],
};

getModelProviderProfiles.mockImplementation(async (model: string) =>
  model === "nano-banana" ? geminiProviderState : openAiProviderState,
);
saveModelProviderProfiles.mockImplementation(async (_model: string, state: unknown) => state);
createModelProviderProfile.mockResolvedValue({
  ...openAiProviderState,
  active_provider_id: "new-provider",
  profiles: [
    ...openAiProviderState.profiles,
    {
      id: "new-provider",
      name: "New Provider",
      api_key: "",
      endpoint_settings: openAiProviderState.profiles[0].endpoint_settings,
    },
  ],
});
deleteModelProviderProfile.mockResolvedValue({
  active_provider_id: "openai-official",
  profiles: [openAiProviderState.profiles[0]],
});
setActiveModelProvider.mockImplementation(async (_model: string, providerId: string) => ({
  ...openAiProviderState,
  active_provider_id: providerId,
}));
```

Add these tests:

```tsx
  it("loads provider profiles for the selected model", async () => {
    render(
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    expect(await screen.findByText("OpenAI Official")).toBeInTheDocument();
    expect(screen.getByText("Company Gateway")).toBeInTheDocument();
    expect(getModelProviderProfiles).toHaveBeenCalledWith("gpt-image-2");
  });

  it("saves edits to the selected provider profile", async () => {
    render(
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));
    await screen.findByText("OpenAI Official");
    fireEvent.click(screen.getByRole("button", { name: "Select Company Gateway provider" }));
    fireEvent.change(screen.getByDisplayValue("Company Gateway"), {
      target: { value: "Renamed Gateway" },
    });
    fireEvent.click(screen.getByRole("button", { name: "settings.saveProvider" }));

    await waitFor(() => {
      expect(saveModelProviderProfiles).toHaveBeenCalledWith(
        "gpt-image-2",
        expect.objectContaining({
          active_provider_id: "openai-official",
          profiles: expect.arrayContaining([
            expect.objectContaining({
              id: "company-gateway",
              name: "Renamed Gateway",
            }),
          ]),
        }),
      );
    });
  });

  it("creates, activates, and deletes provider profiles through the profile API", async () => {
    render(
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));
    await screen.findByText("OpenAI Official");
    fireEvent.click(screen.getByRole("button", { name: "settings.newProvider" }));

    await waitFor(() => {
      expect(createModelProviderProfile).toHaveBeenCalledWith(
        "gpt-image-2",
        "New Provider",
      );
      expect(screen.getByDisplayValue("New Provider")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Select Company Gateway provider" }));
    fireEvent.click(screen.getByRole("button", { name: "Use Company Gateway provider" }));
    await waitFor(() => {
      expect(setActiveModelProvider).toHaveBeenCalledWith(
        "gpt-image-2",
        "company-gateway",
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Delete Company Gateway provider" }));
    await waitFor(() => {
      expect(deleteModelProviderProfile).toHaveBeenCalledWith(
        "gpt-image-2",
        "company-gateway",
      );
    });
  });
```

- [ ] **Step 2: Run settings page tests to verify they fail**

Run:

```bash
npm test -- src/pages/SettingsPage.test.tsx
```

Expected: FAIL because `SettingsPage` still uses single model key and endpoint APIs.

- [ ] **Step 3: Integrate provider profile state in `SettingsPage`**

Replace single connection state:

```ts
const [apiKey, setApiKey] = useState("");
const [keySaved, setKeySaved] = useState(false);
const [endpointMode, setEndpointMode] = useState<EndpointMode>("base_url");
const [baseUrl, setBaseUrl] = useState(DEFAULT_MODEL_ENTRY.connectionDefaults.baseUrl);
const [generationUrl, setGenerationUrl] = useState(
  DEFAULT_MODEL_ENTRY.connectionDefaults.generationUrl,
);
const [editUrl, setEditUrl] = useState(DEFAULT_MODEL_ENTRY.connectionDefaults.editUrl);
const [urlSaved, setUrlSaved] = useState(false);
```

with:

```ts
const [providerState, setProviderState] = useState<ModelProviderProfilesState>(() =>
  defaultProviderProfilesStateForModel(DEFAULT_MODEL),
);
const [selectedProviderId, setSelectedProviderId] = useState(DEFAULT_PROVIDER_ID);
const [providerSaved, setProviderSaved] = useState(false);
```

Import helpers:

```ts
import {
  DEFAULT_PROVIDER_ID,
  NEW_PROVIDER_NAME,
  defaultProviderProfilesStateForModel,
  providerForState,
  removeProviderFromState,
  updateProviderInState,
} from "../lib/modelProviderProfiles";
```

Import APIs:

```ts
  createModelProviderProfile,
  deleteModelProviderProfile,
  getModelProviderProfiles,
  saveModelProviderProfiles,
  setActiveModelProvider,
```

Remove `maskKey` from `SettingsPage.tsx`; it now lives in the panel.

- [ ] **Step 4: Load provider state on model change**

Replace the effect that calls `getModelApiKey` and `getModelEndpointSettings` with:

```ts
useEffect(() => {
  let cancelled = false;
  const defaults = defaultProviderProfilesStateForModel(imageModel);

  setProviderState(defaults);
  setSelectedProviderId(defaults.active_provider_id);
  setShowKey(false);
  setProviderSaved(false);

  getModelProviderProfiles(imageModel).then((state) => {
    if (cancelled) {
      return;
    }

    setProviderState(state);
    setSelectedProviderId(state.active_provider_id);
    setShowKey(false);
  }).catch(() => {
    if (cancelled) {
      return;
    }

    setProviderState(defaults);
    setSelectedProviderId(defaults.active_provider_id);
    setShowKey(false);
  });

  return () => {
    cancelled = true;
  };
}, [imageModel]);
```

- [ ] **Step 5: Add provider edit handlers**

Add these functions inside `SettingsPage`:

```ts
function updateSelectedProvider(
  update: (provider: ModelProviderProfile) => ModelProviderProfile,
) {
  setProviderState((current) =>
    updateProviderInState(current, selectedProviderId, update),
  );
  setProviderSaved(false);
}

async function handleSaveProvider() {
  const modelAtSaveStart = imageModel;
  const stateAtSaveStart = providerState;

  const savedState = await saveModelProviderProfiles(
    modelAtSaveStart,
    stateAtSaveStart,
  );
  if (imageModelRef.current !== modelAtSaveStart) {
    return;
  }

  setProviderState(savedState);
  setSelectedProviderId(
    providerForState(savedState, selectedProviderId)?.id ??
      savedState.active_provider_id,
  );
  setProviderSaved(true);
  setTimeout(() => setProviderSaved(false), 2000);
}

async function handleCreateProvider() {
  const modelAtCreateStart = imageModel;
  const savedState = await createModelProviderProfile(
    modelAtCreateStart,
    NEW_PROVIDER_NAME,
  );
  if (imageModelRef.current !== modelAtCreateStart) {
    return;
  }

  setProviderState(savedState);
  setSelectedProviderId(savedState.active_provider_id);
  setProviderSaved(false);
}

async function handleDeleteProvider(providerId: string) {
  const optimisticState = removeProviderFromState(providerState, providerId);
  setProviderState(optimisticState);
  setSelectedProviderId(
    providerForState(optimisticState, selectedProviderId)?.id ??
      optimisticState.active_provider_id,
  );

  const modelAtDeleteStart = imageModel;
  const savedState = await deleteModelProviderProfile(modelAtDeleteStart, providerId);
  if (imageModelRef.current !== modelAtDeleteStart) {
    return;
  }

  setProviderState(savedState);
  setSelectedProviderId(
    providerForState(savedState, selectedProviderId)?.id ??
      savedState.active_provider_id,
  );
}

async function handleSetActiveProvider(providerId: string) {
  const modelAtActivateStart = imageModel;
  const savedState = await setActiveModelProvider(
    modelAtActivateStart,
    providerId,
  );
  if (imageModelRef.current !== modelAtActivateStart) {
    return;
  }

  setProviderState(savedState);
  setSelectedProviderId(providerId);
  setProviderSaved(false);
}
```

Use `ModelProviderProfile` in the type imports from `../types`.

- [ ] **Step 6: Pass provider props into `ModelSettingsPanel`**

Replace old connection props in the `ModelSettingsPanel` call with:

```tsx
providerState={providerState}
selectedProviderId={selectedProviderId}
providerSaved={providerSaved}
onSelectProvider={(providerId) => {
  setSelectedProviderId(providerId);
  setShowKey(false);
  setProviderSaved(false);
}}
onProviderNameChange={(name) => {
  updateSelectedProvider((provider) => ({ ...provider, name }));
}}
onProviderApiKeyChange={(apiKey) => {
  updateSelectedProvider((provider) => ({ ...provider, api_key: apiKey }));
}}
onShowKeyChange={setShowKey}
onProviderEndpointModeChange={(mode) => {
  updateSelectedProvider((provider) => ({
    ...provider,
    endpoint_settings: { ...provider.endpoint_settings, mode },
  }));
}}
onProviderBaseUrlChange={(url) => {
  updateSelectedProvider((provider) => ({
    ...provider,
    endpoint_settings: { ...provider.endpoint_settings, base_url: url },
  }));
}}
onProviderGenerationUrlChange={(url) => {
  updateSelectedProvider((provider) => ({
    ...provider,
    endpoint_settings: { ...provider.endpoint_settings, generation_url: url },
  }));
}}
onProviderEditUrlChange={(url) => {
  updateSelectedProvider((provider) => ({
    ...provider,
    endpoint_settings: { ...provider.endpoint_settings, edit_url: url },
  }));
}}
onCreateProvider={() => void handleCreateProvider()}
onDeleteProvider={(providerId) => void handleDeleteProvider(providerId)}
onSetActiveProvider={(providerId) => void handleSetActiveProvider(providerId)}
onSaveProvider={() => void handleSaveProvider()}
```

- [ ] **Step 7: Run settings page tests**

Run:

```bash
npm test -- src/pages/SettingsPage.test.tsx
```

Expected: PASS for settings page tests.

- [ ] **Step 8: Commit**

```bash
git add src/pages/SettingsPage.tsx src/pages/SettingsPage.test.tsx
git commit -m "feat: manage provider profiles in settings"
```

## Task 7: Localization, Build, And Full Verification

**Files:**
- Modify: `src/locales/en.json`
- Modify: `src/locales/zh-CN.json`
- Modify: `src/locales/zh-TW.json`
- Modify: `src/locales/ja.json`
- Modify: `src/locales/ko.json`
- Modify: `src/locales/fr.json`
- Modify: `src/locales/de.json`
- Modify: `src/locales/es.json`

- [ ] **Step 1: Add provider localization keys**

Add these keys near the existing settings model config keys in every locale file. Use localized text where available; use the English value as a fallback if a locale cannot be confidently translated.

English values:

```json
"settings.providers": "Providers",
"settings.providersDesc": "Custom service providers for the selected model",
"settings.providerName": "Provider name",
"settings.newProvider": "New Provider",
"settings.activateProvider": "Use Provider",
"settings.activeProvider": "Active",
"settings.deleteProvider": "Delete Provider",
"settings.saveProvider": "Save Provider"
```

Simplified Chinese values:

```json
"settings.providers": "服务商配置",
"settings.providersDesc": "当前模型的自定义服务商",
"settings.providerName": "服务商名称",
"settings.newProvider": "新增服务商",
"settings.activateProvider": "启用服务商",
"settings.activeProvider": "当前启用",
"settings.deleteProvider": "删除服务商",
"settings.saveProvider": "保存服务商"
```

- [ ] **Step 2: Run all frontend tests**

Run:

```bash
npm test
```

Expected: all Vitest tests pass.

- [ ] **Step 3: Run frontend build**

Run:

```bash
npm run build
```

Expected: TypeScript and Vite build pass.

- [ ] **Step 4: Run backend tests**

Run:

```bash
cargo test
```

Expected: all Rust tests pass.

- [ ] **Step 5: Run backend build**

Run:

```bash
cargo build
```

Expected: Rust build passes.

- [ ] **Step 6: Optional UI smoke check**

Run:

```bash
npm run dev
```

Open the Vite URL, navigate to Settings, select Model Configuration, and verify:

- model cards still render
- provider rows render below the model cards
- selecting a provider updates the editor
- creating a provider selects `New Provider`
- deleting is disabled when one provider remains
- endpoint mode switches without layout shifting

- [ ] **Step 7: Commit verification and localization**

```bash
git add src/locales src
git commit -m "chore: verify provider profile settings"
```

## Self-Review Checklist

- Spec coverage: backend JSON profiles, active provider id, lazy legacy default, compatibility commands, settings UI draft, no provider templates, and no Generate-page provider selector are all covered by tasks.
- Placeholder scan: no TBD/TODO steps; every task includes concrete tests, commands, and implementation snippets.
- Type consistency: frontend uses `ModelProviderProfile`, `ModelProviderProfilesState`, `active_provider_id`, `api_key`, and `endpoint_settings`; backend uses the same serialized field names.
