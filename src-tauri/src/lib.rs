mod api_gateway;
mod config;
mod db;
mod file_manager;
mod models;
mod runtime_logs;

use api_gateway::ImageEngine;
use chrono::{SecondsFormat, Utc};
use db::Database;
use models::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use tauri::{Emitter, Manager};

const DEFAULT_CONVERSATION_TITLE: &str = "New Conversation";
const RECOVERY_STATE_REQUESTING: &str = "requesting";
const RECOVERY_STATE_RESPONSE_READY: &str = "response_ready";
const RECOVERY_KIND_GENERATE: &str = "generate";
const RECOVERY_KIND_EDIT: &str = "edit";
const INTERRUPTED_GENERATION_MESSAGE: &str =
    "This task was interrupted because Astro Studio closed before the response was saved.";

fn current_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn format_log_clear_cutoff(now: chrono::DateTime<Utc>, days: u32) -> String {
    (now - chrono::Duration::days(days as i64)).to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn active_generation_filter(alias: &str) -> String {
    format!("{alias}.deleted_at IS NULL")
}

fn deleted_generation_filter(alias: &str) -> String {
    format!("{alias}.deleted_at IS NOT NULL")
}

fn trash_cutoff_timestamp(retention_days: u32) -> String {
    (Utc::now() - chrono::Duration::days(retention_days as i64))
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn normalize_font_size(font_size: &str) -> &'static str {
    match font_size {
        "small" => "small",
        "large" => "large",
        _ => DEFAULT_FONT_SIZE,
    }
}

fn normalize_image_model(model: &str) -> &'static str {
    match model {
        ENGINE_GPT_IMAGE_2 => ENGINE_GPT_IMAGE_2,
        ENGINE_NANO_BANANA | GEMINI_MODEL_NANO_BANANA => ENGINE_NANO_BANANA,
        ENGINE_NANO_BANANA_2 | GEMINI_MODEL_NANO_BANANA_2 => ENGINE_NANO_BANANA_2,
        ENGINE_NANO_BANANA_PRO | GEMINI_MODEL_NANO_BANANA_PRO => ENGINE_NANO_BANANA_PRO,
        _ => DEFAULT_IMAGE_MODEL,
    }
}

fn is_gemini_model(model: &str) -> bool {
    matches!(
        normalize_image_model(model),
        ENGINE_NANO_BANANA | ENGINE_NANO_BANANA_2 | ENGINE_NANO_BANANA_PRO
    )
}

fn gemini_provider_model_id(model: &str) -> &'static str {
    match normalize_image_model(model) {
        ENGINE_NANO_BANANA => GEMINI_MODEL_NANO_BANANA,
        ENGINE_NANO_BANANA_2 => GEMINI_MODEL_NANO_BANANA_2,
        ENGINE_NANO_BANANA_PRO => GEMINI_MODEL_NANO_BANANA_PRO,
        _ => GEMINI_MODEL_NANO_BANANA,
    }
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

fn sanitize_request_options_for_model(
    model: &str,
    mut options: GptImageRequestOptions,
) -> GptImageRequestOptions {
    if is_gemini_model(model) {
        options.quality = DEFAULT_IMAGE_QUALITY.to_string();
        options.background = DEFAULT_IMAGE_BACKGROUND.to_string();
        options.output_format = DEFAULT_OUTPUT_FORMAT.to_string();
        options.output_compression = DEFAULT_OUTPUT_COMPRESSION;
        options.moderation = DEFAULT_IMAGE_MODERATION.to_string();
        options.input_fidelity = DEFAULT_INPUT_FIDELITY.to_string();
    }

    options
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ImageEndpointKind {
    Generate,
    Edit,
}

fn normalize_endpoint_mode(mode: &str) -> &'static str {
    match mode {
        ENDPOINT_MODE_FULL_URL => ENDPOINT_MODE_FULL_URL,
        _ => ENDPOINT_MODE_BASE_URL,
    }
}

fn endpoint_value_or_default(value: Option<String>, default_value: &str) -> String {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_value.to_string())
}

fn build_image_endpoint_url(base_url: &str, kind: ImageEndpointKind) -> String {
    let path = match kind {
        ImageEndpointKind::Generate => "images/generations",
        ImageEndpointKind::Edit => "images/edits",
    };
    format!("{}/{}", base_url.trim_end_matches('/'), path)
}

fn default_endpoint_settings_for_model(model: &str) -> EndpointSettings {
    let model = normalize_image_model(model);

    if is_gemini_model(model) {
        let provider_model = gemini_provider_model_id(model);
        let generation_url =
            format!("{DEFAULT_GEMINI_MODELS_URL}/{provider_model}:generateContent");
        return EndpointSettings {
            mode: ENDPOINT_MODE_BASE_URL.to_string(),
            base_url: DEFAULT_GEMINI_MODELS_URL.to_string(),
            generation_url: generation_url.clone(),
            edit_url: generation_url,
        };
    }

    EndpointSettings {
        mode: ENDPOINT_MODE_BASE_URL.to_string(),
        base_url: DEFAULT_BASE_URL.to_string(),
        generation_url: DEFAULT_GENERATION_URL.to_string(),
        edit_url: DEFAULT_EDIT_URL.to_string(),
    }
}

fn model_setting_key(model: &str, suffix: &str) -> String {
    format!("model_config::{}::{}", normalize_image_model(model), suffix)
}

fn legacy_model_setting_ids(model: &str) -> &'static [&'static str] {
    match normalize_image_model(model) {
        ENGINE_NANO_BANANA => &[GEMINI_MODEL_NANO_BANANA],
        ENGINE_NANO_BANANA_2 => &[GEMINI_MODEL_NANO_BANANA_2],
        ENGINE_NANO_BANANA_PRO => &[GEMINI_MODEL_NANO_BANANA_PRO],
        _ => &[],
    }
}

fn get_model_setting(
    db: &Database,
    model: &str,
    suffix: &str,
    legacy_key: Option<&str>,
) -> Result<Option<String>, String> {
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

fn set_model_setting(db: &Database, model: &str, suffix: &str, value: &str) -> Result<(), String> {
    db.set_setting(&model_setting_key(model, suffix), value)
}

fn current_image_model(db: &Database) -> Result<&'static str, String> {
    Ok(normalize_image_model(
        db.get_setting(SETTING_IMAGE_MODEL)?
            .as_deref()
            .unwrap_or(DEFAULT_IMAGE_MODEL),
    ))
}

fn normalize_gemini_endpoint_url(endpoint_url: &str, model: &str) -> String {
    let endpoint = endpoint_url.trim().trim_end_matches('/');
    let model = gemini_provider_model_id(model);

    if endpoint.ends_with(":generateContent") {
        endpoint.to_string()
    } else if endpoint.ends_with(model) {
        format!("{endpoint}:generateContent")
    } else {
        format!("{endpoint}/{model}:generateContent")
    }
}

fn read_endpoint_settings(db: &Database) -> Result<EndpointSettings, String> {
    let model = current_image_model(db)?;
    read_model_endpoint_settings(db, model)
}

fn read_model_endpoint_settings(db: &Database, model: &str) -> Result<EndpointSettings, String> {
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

fn image_endpoint_url_for_model_settings(
    model: &str,
    settings: &EndpointSettings,
    kind: ImageEndpointKind,
) -> String {
    let model = normalize_image_model(model);

    if settings.mode == ENDPOINT_MODE_FULL_URL {
        if is_gemini_model(model) {
            let endpoint = match kind {
                ImageEndpointKind::Generate => settings.generation_url.clone(),
                ImageEndpointKind::Edit => {
                    if settings.edit_url.trim().is_empty() {
                        settings.generation_url.clone()
                    } else {
                        settings.edit_url.clone()
                    }
                }
            };
            return normalize_gemini_endpoint_url(&endpoint, model);
        }

        return match kind {
            ImageEndpointKind::Generate => settings.generation_url.clone(),
            ImageEndpointKind::Edit => settings.edit_url.clone(),
        };
    }

    if is_gemini_model(model) {
        return normalize_gemini_endpoint_url(&settings.base_url, model);
    }

    build_image_endpoint_url(&settings.base_url, kind)
}

fn resolve_image_endpoint_url_for_model(
    db: &Database,
    model: &str,
    kind: ImageEndpointKind,
) -> Result<String, String> {
    let settings = read_model_endpoint_settings(db, model)?;
    Ok(image_endpoint_url_for_model_settings(
        model, &settings, kind,
    ))
}

fn read_model_api_key(db: &Database, model: &str) -> Result<Option<String>, String> {
    get_model_setting(db, model, SETTING_API_KEY, Some(SETTING_API_KEY))
}

fn save_model_api_key_value(db: &Database, model: &str, key: &str) -> Result<(), String> {
    set_model_setting(db, model, SETTING_API_KEY, key)?;
    if normalize_image_model(model) == ENGINE_GPT_IMAGE_2 {
        db.set_setting(SETTING_API_KEY, key)?;
    }
    Ok(())
}

fn save_model_endpoint_settings_value(
    db: &Database,
    model: &str,
    mode: &str,
    base_url: &str,
    generation_url: &str,
    edit_url: &str,
) -> Result<(), String> {
    let defaults = default_endpoint_settings_for_model(model);
    let normalized_model = normalize_image_model(model);
    let mode = normalize_endpoint_mode(mode);
    let base_url = endpoint_value_or_default(Some(base_url.to_string()), &defaults.base_url);
    let generation_url =
        endpoint_value_or_default(Some(generation_url.to_string()), &defaults.generation_url);
    let edit_url = if is_gemini_model(normalized_model) {
        generation_url.clone()
    } else {
        endpoint_value_or_default(Some(edit_url.to_string()), &defaults.edit_url)
    };

    set_model_setting(db, normalized_model, SETTING_ENDPOINT_MODE, mode)?;
    set_model_setting(db, normalized_model, SETTING_BASE_URL, &base_url)?;
    set_model_setting(
        db,
        normalized_model,
        SETTING_GENERATION_URL,
        &generation_url,
    )?;
    set_model_setting(db, normalized_model, SETTING_EDIT_URL, &edit_url)?;

    if normalized_model == ENGINE_GPT_IMAGE_2 {
        db.set_setting(SETTING_ENDPOINT_MODE, mode)?;
        db.set_setting(SETTING_BASE_URL, &base_url)?;
        db.set_setting(SETTING_GENERATION_URL, &generation_url)?;
        db.set_setting(SETTING_EDIT_URL, &edit_url)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_mode_builds_image_endpoint_paths() {
        assert_eq!(
            build_image_endpoint_url("https://api.example.test/v1/", ImageEndpointKind::Generate),
            "https://api.example.test/v1/images/generations"
        );
        assert_eq!(
            build_image_endpoint_url("https://api.example.test/v1", ImageEndpointKind::Edit),
            "https://api.example.test/v1/images/edits"
        );
    }

    #[test]
    fn endpoint_mode_normalizes_to_supported_values() {
        assert_eq!(
            normalize_endpoint_mode(ENDPOINT_MODE_FULL_URL),
            ENDPOINT_MODE_FULL_URL
        );
        assert_eq!(
            normalize_endpoint_mode("unsupported"),
            ENDPOINT_MODE_BASE_URL
        );
    }

    #[test]
    fn full_url_mode_uses_separate_generation_and_edit_urls() {
        let settings = EndpointSettings {
            mode: ENDPOINT_MODE_FULL_URL.to_string(),
            base_url: "https://unused.example.test/v1".to_string(),
            generation_url: "https://gateway.example.test/create".to_string(),
            edit_url: "https://gateway.example.test/edit".to_string(),
        };

        assert_eq!(
            image_endpoint_url_for_model_settings(
                ENGINE_GPT_IMAGE_2,
                &settings,
                ImageEndpointKind::Generate,
            ),
            "https://gateway.example.test/create"
        );
        assert_eq!(
            image_endpoint_url_for_model_settings(
                ENGINE_GPT_IMAGE_2,
                &settings,
                ImageEndpointKind::Edit,
            ),
            "https://gateway.example.test/edit"
        );
    }

    #[test]
    fn normalize_image_model_accepts_gemini_nanobanana_models() {
        assert_eq!(
            normalize_image_model(ENGINE_NANO_BANANA),
            ENGINE_NANO_BANANA
        );
        assert_eq!(
            normalize_image_model(GEMINI_MODEL_NANO_BANANA),
            ENGINE_NANO_BANANA
        );
        assert_eq!(
            normalize_image_model(ENGINE_NANO_BANANA_PRO),
            ENGINE_NANO_BANANA_PRO
        );
        assert_eq!(
            normalize_image_model(ENGINE_NANO_BANANA_2),
            ENGINE_NANO_BANANA_2
        );
        assert_eq!(
            normalize_image_model(GEMINI_MODEL_NANO_BANANA_2),
            ENGINE_NANO_BANANA_2
        );
        assert_eq!(
            normalize_image_model(GEMINI_MODEL_NANO_BANANA_PRO),
            ENGINE_NANO_BANANA_PRO
        );
    }

    #[test]
    fn gemini_base_url_mode_builds_generate_content_endpoint() {
        let settings = EndpointSettings {
            mode: ENDPOINT_MODE_BASE_URL.to_string(),
            base_url: DEFAULT_GEMINI_MODELS_URL.to_string(),
            generation_url: String::new(),
            edit_url: String::new(),
        };

        assert_eq!(
            image_endpoint_url_for_model_settings(
                ENGINE_NANO_BANANA,
                &settings,
                ImageEndpointKind::Generate,
            ),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent"
        );
        assert_eq!(
            image_endpoint_url_for_model_settings(
                ENGINE_NANO_BANANA,
                &settings,
                ImageEndpointKind::Edit,
            ),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent"
        );
    }

    #[test]
    fn gemini_models_drop_unsupported_request_controls() {
        let options = sanitize_request_options_for_model(
            ENGINE_NANO_BANANA,
            GptImageRequestOptions {
                size: "1536x1024".to_string(),
                quality: "high".to_string(),
                background: "transparent".to_string(),
                output_format: "webp".to_string(),
                output_compression: 75,
                moderation: "low".to_string(),
                input_fidelity: "low".to_string(),
                stream: false,
                partial_images: 0,
                image_count: 3,
            },
        );

        assert_eq!(options.size, "1536x1024");
        assert_eq!(options.image_count, 3);
        assert_eq!(options.quality, DEFAULT_IMAGE_QUALITY);
        assert_eq!(options.background, DEFAULT_IMAGE_BACKGROUND);
        assert_eq!(options.output_format, DEFAULT_OUTPUT_FORMAT);
        assert_eq!(options.output_compression, DEFAULT_OUTPUT_COMPRESSION);
        assert_eq!(options.moderation, DEFAULT_IMAGE_MODERATION);
        assert_eq!(options.input_fidelity, DEFAULT_INPUT_FIDELITY);
    }

    #[test]
    fn log_clear_cutoff_uses_database_timestamp_format() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-28T12:30:45Z")
            .unwrap()
            .with_timezone(&Utc);

        assert_eq!(format_log_clear_cutoff(now, 0), "2026-04-28T12:30:45Z");
        assert_eq!(format_log_clear_cutoff(now, 7), "2026-04-21T12:30:45Z");
    }
}

#[cfg(all(debug_assertions, target_os = "macos"))]
fn set_macos_development_app_icon() {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    static APP_ICON: &[u8] = include_bytes!("../icons/icon.png");

    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let app = NSApplication::sharedApplication(mtm);
    let data = NSData::with_bytes(APP_ICON);

    if let Some(app_icon) = NSImage::initWithData(NSImage::alloc(), &data) {
        unsafe { app.setApplicationIconImage(Some(&app_icon)) };
        log::info!("Applied macOS development app icon from icons/icon.png");
    } else {
        log::warn!("Failed to load macOS development app icon from icons/icon.png");
    }
}

// --- Settings commands ---

#[tauri::command]
fn save_api_key(db: tauri::State<'_, Database>, key: String) -> Result<(), String> {
    log::info!("Saving API key");
    let model = current_image_model(db.inner())?;
    save_model_api_key_value(db.inner(), model, &key)
}

#[tauri::command]
fn get_api_key(db: tauri::State<'_, Database>) -> Result<Option<String>, String> {
    let model = current_image_model(db.inner())?;
    read_model_api_key(db.inner(), model)
}

#[tauri::command]
fn save_base_url(db: tauri::State<'_, Database>, url: String) -> Result<(), String> {
    log::info!("Saving base URL: {}", url);
    let model = current_image_model(db.inner())?;
    let settings = read_model_endpoint_settings(db.inner(), model)?;
    save_model_endpoint_settings_value(
        db.inner(),
        model,
        &settings.mode,
        &url,
        &settings.generation_url,
        &settings.edit_url,
    )
}

#[tauri::command]
fn get_base_url(db: tauri::State<'_, Database>) -> Result<String, String> {
    Ok(read_model_endpoint_settings(db.inner(), current_image_model(db.inner())?)?.base_url)
}

#[tauri::command]
fn get_endpoint_settings(db: tauri::State<'_, Database>) -> Result<EndpointSettings, String> {
    read_endpoint_settings(db.inner())
}

#[tauri::command]
fn save_endpoint_settings(
    db: tauri::State<'_, Database>,
    mode: String,
    base_url: String,
    generation_url: String,
    edit_url: String,
) -> Result<(), String> {
    let model = current_image_model(db.inner())?;
    let defaults = default_endpoint_settings_for_model(model);
    let mode = normalize_endpoint_mode(&mode);
    let base_url = endpoint_value_or_default(Some(base_url), &defaults.base_url);
    let generation_url = endpoint_value_or_default(Some(generation_url), &defaults.generation_url);
    let edit_url = if is_gemini_model(model) {
        generation_url.clone()
    } else {
        endpoint_value_or_default(Some(edit_url), &defaults.edit_url)
    };

    log::info!(
        "Saving endpoint settings: mode={}, base_url={}, generation_url={}, edit_url={}",
        mode,
        base_url,
        generation_url,
        edit_url
    );

    save_model_endpoint_settings_value(
        db.inner(),
        model,
        mode,
        &base_url,
        &generation_url,
        &edit_url,
    )
}

#[tauri::command]
fn get_model_api_key(
    db: tauri::State<'_, Database>,
    model: String,
) -> Result<Option<String>, String> {
    read_model_api_key(db.inner(), &model)
}

#[tauri::command]
fn save_model_api_key(
    db: tauri::State<'_, Database>,
    model: String,
    key: String,
) -> Result<(), String> {
    save_model_api_key_value(db.inner(), &model, &key)
}

#[tauri::command]
fn get_model_endpoint_settings(
    db: tauri::State<'_, Database>,
    model: String,
) -> Result<EndpointSettings, String> {
    read_model_endpoint_settings(db.inner(), &model)
}

#[tauri::command]
fn save_model_endpoint_settings(
    db: tauri::State<'_, Database>,
    model: String,
    mode: String,
    base_url: String,
    generation_url: String,
    edit_url: String,
) -> Result<(), String> {
    save_model_endpoint_settings_value(
        db.inner(),
        &model,
        &mode,
        &base_url,
        &generation_url,
        &edit_url,
    )
}

#[tauri::command]
fn get_font_size(db: tauri::State<'_, Database>) -> Result<String, String> {
    Ok(normalize_font_size(
        db.get_setting(SETTING_FONT_SIZE)?
            .as_deref()
            .unwrap_or(DEFAULT_FONT_SIZE),
    )
    .to_string())
}

#[tauri::command]
fn save_font_size(db: tauri::State<'_, Database>, font_size: String) -> Result<(), String> {
    db.set_setting(SETTING_FONT_SIZE, normalize_font_size(&font_size))
}

#[tauri::command]
fn get_image_model(db: tauri::State<'_, Database>) -> Result<String, String> {
    Ok(normalize_image_model(
        db.get_setting(SETTING_IMAGE_MODEL)?
            .as_deref()
            .unwrap_or(DEFAULT_IMAGE_MODEL),
    )
    .to_string())
}

#[tauri::command]
fn save_image_model(db: tauri::State<'_, Database>, model: String) -> Result<(), String> {
    db.set_setting(SETTING_IMAGE_MODEL, normalize_image_model(&model))
}

struct PendingGenerationRecovery {
    generation_id: String,
    created_at: String,
    output_format: String,
    request_state: Option<String>,
    response_file: Option<String>,
}

fn create_processing_generation(
    conn: &Connection,
    generation_id: &str,
    prompt: &str,
    model: &str,
    size: &str,
    quality: &str,
    conversation_id: &str,
    created_at: &str,
    request_kind: &str,
    output_format: &str,
) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO generations (id, prompt, engine, size, quality, status, error_message, conversation_id, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, 'processing', NULL, ?6, ?7)",
        params![
            generation_id,
            prompt,
            model,
            size,
            quality,
            conversation_id,
            created_at
        ],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO generation_recoveries (generation_id, request_kind, request_state, output_format, response_file, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?5)",
        params![
            generation_id,
            request_kind,
            RECOVERY_STATE_REQUESTING,
            output_format,
            created_at
        ],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

fn update_generation_recovery_response(
    conn: &Connection,
    generation_id: &str,
    response_file: &str,
) -> Result<(), String> {
    conn.execute(
        "UPDATE generation_recoveries SET request_state = ?1, response_file = ?2, updated_at = ?3 WHERE generation_id = ?4",
        params![
            RECOVERY_STATE_RESPONSE_READY,
            response_file,
            current_timestamp(),
            generation_id
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn set_generation_failed(
    conn: &Connection,
    generation_id: &str,
    error_message: &str,
    clear_recovery: bool,
) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE generations SET status = 'failed', error_message = ?1 WHERE id = ?2",
        params![error_message, generation_id],
    )
    .map_err(|e| e.to_string())?;
    if clear_recovery {
        tx.execute(
            "DELETE FROM generation_recoveries WHERE generation_id = ?1",
            params![generation_id],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

fn save_generation_images(
    app: &tauri::AppHandle,
    db: &Database,
    generation_id: &str,
    created_at: &str,
    output_format: &str,
    images_data: &[Vec<u8>],
) -> Result<Vec<GeneratedImage>, String> {
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let fm = file_manager::FileManager::new(app_data_dir);
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "DELETE FROM images WHERE generation_id = ?1",
        params![generation_id],
    )
    .map_err(|e| e.to_string())?;

    let mut saved_images = Vec::with_capacity(images_data.len());
    for (i, data) in images_data.iter().enumerate() {
        let img_id = format!("{}_{}", generation_id, i);
        let saved = fm.save_image_at(&img_id, data, output_format, Some(created_at))?;

        tx.execute(
            "INSERT INTO images (id, generation_id, file_path, thumbnail_path, width, height, file_size, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![img_id, generation_id, saved.file_path, saved.thumbnail_path, saved.width, saved.height, saved.file_size, created_at],
        ).map_err(|e| e.to_string())?;

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
    .map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM generation_recoveries WHERE generation_id = ?1",
        params![generation_id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;

    Ok(saved_images)
}

async fn recover_interrupted_generations(
    app: &tauri::AppHandle,
    db: &Database,
    engine: &api_gateway::GptImageEngine,
) -> Result<(), String> {
    {
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM generation_recoveries WHERE generation_id IN (SELECT id FROM generations WHERE status != 'processing')",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    let pending = {
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn
            .prepare(
                "SELECT g.id, g.created_at, COALESCE(r.output_format, ?1), r.request_state, r.response_file
                 FROM generations g
                 LEFT JOIN generation_recoveries r ON r.generation_id = g.id
                 WHERE g.status = 'processing'",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![DEFAULT_OUTPUT_FORMAT], |row| {
                Ok(PendingGenerationRecovery {
                    generation_id: row.get(0)?,
                    created_at: row.get(1)?,
                    output_format: row.get(2)?,
                    request_state: row.get(3)?,
                    response_file: row.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.filter_map(|row| row.ok()).collect::<Vec<_>>()
    };

    for recovery in pending {
        let Some(request_state) = recovery.request_state.as_deref() else {
            let conn = db.conn.lock().map_err(|e| e.to_string())?;
            set_generation_failed(
                &conn,
                &recovery.generation_id,
                INTERRUPTED_GENERATION_MESSAGE,
                true,
            )?;
            continue;
        };

        if request_state == RECOVERY_STATE_RESPONSE_READY {
            let Some(response_file) = recovery.response_file.as_deref() else {
                let conn = db.conn.lock().map_err(|e| e.to_string())?;
                set_generation_failed(
                    &conn,
                    &recovery.generation_id,
                    "The API response was received, but the recovery payload could not be found.",
                    true,
                )?;
                continue;
            };

            let body_text = match std::fs::read_to_string(response_file) {
                Ok(body_text) => body_text,
                Err(error) => {
                    let conn = db.conn.lock().map_err(|e| e.to_string())?;
                    set_generation_failed(
                        &conn,
                        &recovery.generation_id,
                        &format!(
                            "The API response was received, but Astro Studio could not reopen the saved payload: {}",
                            error
                        ),
                        true,
                    )?;
                    continue;
                }
            };

            let decoded_images = match engine.decode_images_from_response(&body_text).await {
                Ok(decoded_images) => decoded_images,
                Err(error) => {
                    let conn = db.conn.lock().map_err(|e| e.to_string())?;
                    set_generation_failed(
                        &conn,
                        &recovery.generation_id,
                        &format!(
                            "The API response was received, but Astro Studio could not recover the image data: {}",
                            error
                        ),
                        true,
                    )?;
                    continue;
                }
            };

            match save_generation_images(
                app,
                db,
                &recovery.generation_id,
                &recovery.created_at,
                &recovery.output_format,
                &decoded_images,
            ) {
                Ok(saved_images) => {
                    log::info!(
                        "[{}] Recovered {} image(s) from saved API response",
                        recovery.generation_id,
                        saved_images.len()
                    );
                    let _ = db.insert_log(
                        "generation",
                        "info",
                        &format!(
                            "Recovered after restart — {} image(s) saved",
                            saved_images.len()
                        ),
                        Some(&recovery.generation_id),
                        Some(&serde_json::json!({"image_count": saved_images.len()}).to_string()),
                        None,
                    );
                }
                Err(error) => {
                    log::error!(
                        "[{}] Failed to recover saved API response: {}",
                        recovery.generation_id,
                        error
                    );
                }
            }

            continue;
        }

        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        set_generation_failed(
            &conn,
            &recovery.generation_id,
            INTERRUPTED_GENERATION_MESSAGE,
            true,
        )?;
    }

    Ok(())
}

// --- Generation commands ---

#[tauri::command]
async fn generate_image(
    app: tauri::AppHandle,
    db: tauri::State<'_, Database>,
    engine_state: tauri::State<'_, api_gateway::GptImageEngine>,
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
) -> Result<GenerateResult, String> {
    let mut options = image_request_options(
        size,
        quality,
        background,
        output_format,
        output_compression,
        moderation,
        None,
        image_count,
    );
    let generation_id = uuid::Uuid::new_v4().to_string();
    let created_at = current_timestamp();

    let conversation_id = {
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        resolve_conversation_id(&conn, conversation_id.as_deref(), &prompt)?
    };
    let model = normalize_image_model(
        model
            .as_deref()
            .or(db.get_setting(SETTING_IMAGE_MODEL)?.as_deref())
            .unwrap_or(DEFAULT_IMAGE_MODEL),
    )
    .to_string();
    options = sanitize_request_options_for_model(&model, options);

    log::info!(
        "[{}] Generating image — model: {}, prompt: {:?}, size: {}, quality: {}, background: {}, output_format: {}, output_compression: {}, moderation: {}, stream: {}, partial_images: {}, image_count: {}",
        generation_id,
        model,
        prompt,
        options.size,
        options.quality,
        options.background,
        options.output_format,
        options.output_compression,
        options.moderation,
        options.stream,
        options.partial_images,
        options.image_count
    );

    let _ = db.insert_log(
        "generation", "info",
        &format!(
            "Started — prompt: {:?}, size: {}, quality: {}, background: {}, output_format: {}, image_count: {}",
            prompt, options.size, options.quality, options.background, options.output_format, options.image_count
        ),
        Some(&generation_id),
        Some(&serde_json::json!({
            "model": &model,
            "prompt": prompt,
            "size": &options.size,
            "quality": &options.quality,
            "background": &options.background,
            "output_format": &options.output_format,
            "output_compression": options.output_compression,
            "moderation": &options.moderation,
            "stream": options.stream,
            "partial_images": options.partial_images,
            "image_count": options.image_count,
            "conversation_id": conversation_id
        }).to_string()),
        None,
    );
    let api_key = read_model_api_key(db.inner(), &model)?
        .ok_or_else(|| "API key not set. Please set it in Settings.".to_string())?;
    let endpoint_url =
        resolve_image_endpoint_url_for_model(db.inner(), &model, ImageEndpointKind::Generate)?;
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

    {
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        create_processing_generation(
            &conn,
            &generation_id,
            &prompt,
            &model,
            &options.size,
            &options.quality,
            &conversation_id,
            &created_at,
            RECOVERY_KIND_GENERATE,
            &options.output_format,
        )?;
    }

    let _ = app.emit(
        "generation:progress",
        serde_json::json!({
            "generation_id": generation_id,
            "status": "processing"
        }),
    );

    let result = engine_state
        .generate(
            &generation_id,
            &model,
            &api_key,
            &endpoint_url,
            &prompt,
            &options,
            Some(&db),
            Some(&app_data_dir),
        )
        .await;

    match result {
        Ok(engine_response) => {
            log::info!(
                "[{}] API returned {} image(s), saving to disk...",
                generation_id,
                engine_response.images.len()
            );

            if let Some(response_file) = engine_response.response_file.as_deref() {
                let conn = db.conn.lock().map_err(|e| e.to_string())?;
                update_generation_recovery_response(&conn, &generation_id, response_file)?;
            }

            let saved_images = save_generation_images(
                &app,
                db.inner(),
                &generation_id,
                &created_at,
                &options.output_format,
                &engine_response.images,
            )?;

            log::info!(
                "[{}] Generation completed — {} image(s) saved",
                generation_id,
                saved_images.len()
            );

            let _ = db.insert_log(
                "generation",
                "info",
                &format!("Completed — {} image(s) saved", saved_images.len()),
                Some(&generation_id),
                Some(&serde_json::json!({"image_count": saved_images.len()}).to_string()),
                None,
            );

            let _ = app.emit(
                "generation:complete",
                serde_json::json!({
                    "generation_id": generation_id,
                    "status": "completed"
                }),
            );

            Ok(GenerateResult {
                generation_id,
                conversation_id,
                images: saved_images,
            })
        }
        Err(e) => {
            log::error!("[{}] Generation failed: {}", generation_id, e);

            let _ = db.insert_log(
                "generation",
                "error",
                &format!("Failed: {}", &e),
                Some(&generation_id),
                None,
                None,
            );

            let conn = db.conn.lock().map_err(|e| e.to_string())?;
            set_generation_failed(&conn, &generation_id, &e, true)?;

            let _ = app.emit(
                "generation:failed",
                serde_json::json!({
                    "generation_id": generation_id,
                    "error": &e
                }),
            );

            Err(e)
        }
    }
}

#[tauri::command]
async fn edit_image(
    app: tauri::AppHandle,
    db: tauri::State<'_, Database>,
    engine_state: tauri::State<'_, api_gateway::GptImageEngine>,
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
) -> Result<GenerateResult, String> {
    if source_image_paths.is_empty() {
        return Err("Please select at least one source image.".to_string());
    }

    let mut options = image_request_options(
        size,
        quality,
        background,
        output_format,
        output_compression,
        moderation,
        input_fidelity,
        image_count,
    );
    let generation_id = uuid::Uuid::new_v4().to_string();
    let created_at = current_timestamp();

    let conversation_id = {
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        resolve_conversation_id(&conn, conversation_id.as_deref(), &prompt)?
    };
    let model = normalize_image_model(
        model
            .as_deref()
            .or(db.get_setting(SETTING_IMAGE_MODEL)?.as_deref())
            .unwrap_or(DEFAULT_IMAGE_MODEL),
    )
    .to_string();
    options = sanitize_request_options_for_model(&model, options);

    log::info!(
        "[{}] Editing image — model: {}, prompt: {:?}, source_images: {}, size: {}, quality: {}, background: {}, input_fidelity: {}, output_format: {}, output_compression: {}, moderation: {}, stream: {}, partial_images: {}, image_count: {}",
        generation_id,
        model,
        prompt,
        source_image_paths.len(),
        options.size,
        options.quality,
        options.background,
        options.input_fidelity,
        options.output_format,
        options.output_compression,
        options.moderation,
        options.stream,
        options.partial_images,
        options.image_count
    );

    let _ = db.insert_log(
        "generation",
        "info",
        &format!(
            "Edit started — prompt: {:?}, source_images: {}, size: {}, quality: {}, output_format: {}, image_count: {}",
            prompt,
            source_image_paths.len(),
            options.size,
            options.quality,
            options.output_format,
            options.image_count
        ),
        Some(&generation_id),
        Some(
            &serde_json::json!({
                "model": &model,
                "prompt": prompt,
                "source_image_paths": source_image_paths.clone(),
                "size": &options.size,
                "quality": &options.quality,
                "background": &options.background,
                "input_fidelity": &options.input_fidelity,
                "output_format": &options.output_format,
                "output_compression": options.output_compression,
                "moderation": &options.moderation,
                "stream": options.stream,
                "partial_images": options.partial_images,
                "image_count": options.image_count,
                "conversation_id": conversation_id
            })
            .to_string(),
        ),
        None,
    );
    let api_key = read_model_api_key(db.inner(), &model)?
        .ok_or_else(|| "API key not set. Please set it in Settings.".to_string())?;
    let endpoint_url =
        resolve_image_endpoint_url_for_model(db.inner(), &model, ImageEndpointKind::Edit)?;
    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;

    {
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        create_processing_generation(
            &conn,
            &generation_id,
            &prompt,
            &model,
            &options.size,
            &options.quality,
            &conversation_id,
            &created_at,
            RECOVERY_KIND_EDIT,
            &options.output_format,
        )?;
    }

    let _ = app.emit(
        "generation:progress",
        serde_json::json!({
            "generation_id": generation_id,
            "status": "processing"
        }),
    );

    let result = engine_state
        .edit(
            &generation_id,
            &model,
            &api_key,
            &endpoint_url,
            &prompt,
            &source_image_paths,
            &options,
            Some(&db),
            Some(&app_data_dir),
        )
        .await;

    match result {
        Ok(engine_response) => {
            log::info!(
                "[{}] Edit returned {} image(s), saving to disk...",
                generation_id,
                engine_response.images.len()
            );

            if let Some(response_file) = engine_response.response_file.as_deref() {
                let conn = db.conn.lock().map_err(|e| e.to_string())?;
                update_generation_recovery_response(&conn, &generation_id, response_file)?;
            }

            let saved_images = save_generation_images(
                &app,
                db.inner(),
                &generation_id,
                &created_at,
                &options.output_format,
                &engine_response.images,
            )?;

            let _ = db.insert_log(
                "generation",
                "info",
                &format!("Edit completed — {} image(s) saved", saved_images.len()),
                Some(&generation_id),
                Some(&serde_json::json!({"image_count": saved_images.len()}).to_string()),
                None,
            );

            let _ = app.emit(
                "generation:complete",
                serde_json::json!({
                    "generation_id": generation_id,
                    "status": "completed"
                }),
            );

            Ok(GenerateResult {
                generation_id,
                conversation_id,
                images: saved_images,
            })
        }
        Err(e) => {
            log::error!("[{}] Image edit failed: {}", generation_id, e);

            let _ = db.insert_log(
                "generation",
                "error",
                &format!("Edit failed: {}", &e),
                Some(&generation_id),
                None,
                None,
            );

            let conn = db.conn.lock().map_err(|e| e.to_string())?;
            set_generation_failed(&conn, &generation_id, &e, true)?;

            let _ = app.emit(
                "generation:failed",
                serde_json::json!({
                    "generation_id": generation_id,
                    "error": &e
                }),
            );

            Err(e)
        }
    }
}

fn row_to_generation(row: &rusqlite::Row) -> rusqlite::Result<Generation> {
    Ok(Generation {
        id: row.get("id")?,
        prompt: row.get("prompt")?,
        engine: row.get("engine")?,
        size: row.get("size")?,
        quality: row.get("quality")?,
        status: row.get("status")?,
        error_message: row.get("error_message")?,
        created_at: row.get("created_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn row_to_prompt_favorite(row: &rusqlite::Row) -> rusqlite::Result<PromptFavorite> {
    Ok(PromptFavorite {
        id: row.get("id")?,
        prompt: row.get("prompt")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn row_to_image(row: &rusqlite::Row) -> rusqlite::Result<GeneratedImage> {
    Ok(GeneratedImage {
        id: row.get("id")?,
        generation_id: row.get("generation_id")?,
        file_path: row.get("file_path")?,
        thumbnail_path: row.get("thumbnail_path")?,
        width: row.get("width")?,
        height: row.get("height")?,
        file_size: row.get("file_size")?,
    })
}

fn generation_results_with_images(
    conn: &Connection,
    generations: Vec<Generation>,
) -> Result<Vec<GenerationResult>, String> {
    let mut image_map = images_for_generations(conn, &generations)?;
    Ok(generations
        .into_iter()
        .map(|generation| {
            let images = image_map.remove(&generation.id).unwrap_or_default();
            GenerationResult { generation, images }
        })
        .collect())
}

fn images_for_generations(
    conn: &Connection,
    generations: &[Generation],
) -> Result<HashMap<String, Vec<GeneratedImage>>, String> {
    let gen_ids: Vec<&str> = generations.iter().map(|g| g.id.as_str()).collect();
    let mut image_map: HashMap<String, Vec<GeneratedImage>> = HashMap::new();

    if gen_ids.is_empty() {
        return Ok(image_map);
    }

    let placeholders: Vec<&str> = gen_ids.iter().map(|_| "?").collect();
    let sql = format!(
        "SELECT id, generation_id, file_path, thumbnail_path, width, height, file_size \
         FROM images WHERE generation_id IN ({}) ORDER BY created_at ASC",
        placeholders.join(",")
    );
    let params: Vec<&dyn rusqlite::types::ToSql> = gen_ids
        .iter()
        .map(|id| id as &dyn rusqlite::types::ToSql)
        .collect();
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let images = stmt
        .query_map(params.as_slice(), row_to_image)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok());

    for img in images {
        image_map
            .entry(img.generation_id.clone())
            .or_default()
            .push(img);
    }

    Ok(image_map)
}

fn permanently_delete_generation_files_and_records(
    app: &tauri::AppHandle,
    conn: &Connection,
    generation_id: &str,
) -> Result<(), String> {
    let mut stmt = conn
        .prepare("SELECT file_path, thumbnail_path FROM images WHERE generation_id = ?1")
        .map_err(|e| e.to_string())?;
    let paths: Vec<(String, String)> = stmt
        .query_map(params![generation_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let app_data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let fm = file_manager::FileManager::new(app_data_dir);
    for (file, thumb) in &paths {
        let _ = fm.delete_image(file, thumb);
    }

    conn.execute(
        "DELETE FROM images WHERE generation_id = ?1",
        params![generation_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM generations WHERE id = ?1",
        params![generation_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn purge_trashed_generations(
    app: &tauri::AppHandle,
    db: &Database,
    retention_days: u32,
) -> Result<u64, String> {
    let cutoff = trash_cutoff_timestamp(retention_days);
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id FROM generations WHERE deleted_at IS NOT NULL AND deleted_at <= ?1")
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = stmt
        .query_map(params![cutoff], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    for id in &ids {
        permanently_delete_generation_files_and_records(app, &conn, id)?;
    }

    Ok(ids.len() as u64)
}

#[tauri::command]
fn search_generations(
    db: tauri::State<'_, Database>,
    query: Option<String>,
    page: Option<i32>,
    only_deleted: Option<bool>,
) -> Result<SearchResult, String> {
    let page = page.unwrap_or(1);
    let page_size = DEFAULT_PAGE_SIZE;
    let offset = (page - 1) * page_size;
    let only_deleted = only_deleted.unwrap_or(false);

    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let generation_filter = if only_deleted {
        deleted_generation_filter("g")
    } else {
        active_generation_filter("g")
    };

    let (count, generations) = if let Some(q) = &query {
        if !q.is_empty() {
            let pattern = format!("%{}%", q);
            let count: i32 = conn
                .query_row(
                    &format!(
                        "SELECT COUNT(*) FROM generations g WHERE {} AND g.prompt LIKE ?1",
                        generation_filter
                    ),
                    params![pattern],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;

            let mut stmt = conn
                .prepare(&format!(
                    "SELECT g.* FROM generations g WHERE {} AND g.prompt LIKE ?1 ORDER BY g.created_at DESC LIMIT ?2 OFFSET ?3",
                    generation_filter
                ))
                .map_err(|e| e.to_string())?;
            let gens = stmt
                .query_map(params![pattern, page_size, offset], row_to_generation)
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect::<Vec<_>>();

            (count, gens)
        } else {
            fetch_all_generations(&conn, page_size, offset, only_deleted)?
        }
    } else {
        fetch_all_generations(&conn, page_size, offset, only_deleted)?
    };

    let results = generation_results_with_images(&conn, generations)?;

    Ok(SearchResult {
        generations: results,
        total: count,
        page,
        page_size,
    })
}

fn fetch_all_generations(
    conn: &rusqlite::Connection,
    limit: i32,
    offset: i32,
    only_deleted: bool,
) -> Result<(i32, Vec<Generation>), String> {
    let generation_filter = if only_deleted {
        deleted_generation_filter("g")
    } else {
        active_generation_filter("g")
    };
    let count: i32 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM generations g WHERE {}",
                generation_filter
            ),
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(&format!(
            "SELECT g.* FROM generations g WHERE {} ORDER BY g.created_at DESC LIMIT ?1 OFFSET ?2",
            generation_filter
        ))
        .map_err(|e| e.to_string())?;
    let gens = stmt
        .query_map(params![limit, offset], row_to_generation)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok((count, gens))
}

#[tauri::command]
fn delete_generation(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    log::info!("Moving generation {} to trash", id);
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE generations SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![current_timestamp(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn restore_generation(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    log::info!("Restoring generation {}", id);
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE generations SET deleted_at = NULL WHERE id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn permanently_delete_generation(
    app: tauri::AppHandle,
    db: tauri::State<'_, Database>,
    id: String,
) -> Result<(), String> {
    log::info!("Permanently deleting generation {}", id);
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    permanently_delete_generation_files_and_records(&app, &conn, &id)
}

#[tauri::command]
fn create_conversation(
    db: tauri::State<'_, Database>,
    title: Option<String>,
) -> Result<Conversation, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let conv_id = uuid::Uuid::new_v4().to_string();
    let timestamp = current_timestamp();
    let title = title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_CONVERSATION_TITLE);

    conn.execute(
        "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params![conv_id, title, &timestamp, &timestamp],
    )
    .map_err(|e| e.to_string())?;

    fetch_conversation(&conn, &conv_id)
}

fn resolve_conversation_id(
    conn: &rusqlite::Connection,
    conversation_id: Option<&str>,
    prompt: &str,
) -> Result<String, String> {
    if let Some(conv_id) = conversation_id.map(str::trim).filter(|id| !id.is_empty()) {
        let exists = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
                params![conv_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?
            != 0;

        if !exists {
            return create_new_conversation(conn, prompt);
        }

        let generation_count = conn
            .query_row(
                "SELECT COUNT(*) FROM generations WHERE conversation_id = ?1 AND deleted_at IS NULL",
                params![conv_id],
                |row| row.get::<_, i64>(0),
            )
            .ok();

        if let Some(count) = generation_count {
            if count == 0 {
                let timestamp = current_timestamp();
                conn.execute(
                    "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
                    params![conversation_title_from_prompt(prompt), timestamp, conv_id],
                )
                .map_err(|e| e.to_string())?;
                return Ok(conv_id.to_string());
            }

            let timestamp = current_timestamp();
            conn.execute(
                "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
                params![timestamp, conv_id],
            )
            .map_err(|e| e.to_string())?;
            return Ok(conv_id.to_string());
        }
    }

    create_new_conversation(conn, prompt)
}

fn create_new_conversation(conn: &rusqlite::Connection, prompt: &str) -> Result<String, String> {
    let conv_id = uuid::Uuid::new_v4().to_string();
    let title = conversation_title_from_prompt(prompt);
    let timestamp = current_timestamp();
    conn.execute(
        "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params![conv_id, title, &timestamp, &timestamp],
    )
    .map_err(|e| e.to_string())?;
    Ok(conv_id)
}

fn conversation_title_from_prompt(prompt: &str) -> String {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return DEFAULT_CONVERSATION_TITLE.to_string();
    }

    if prompt.chars().count() > 40 {
        prompt.chars().take(40).collect::<String>() + "..."
    } else {
        prompt.to_string()
    }
}

fn conversations_base_sql() -> String {
    "SELECT c.id, c.title, c.created_at, c.updated_at, \
     (SELECT COUNT(*) FROM generations WHERE conversation_id = c.id AND deleted_at IS NULL) as generation_count, \
     (SELECT g.created_at FROM generations g \
      WHERE g.conversation_id = c.id AND g.deleted_at IS NULL ORDER BY g.created_at DESC LIMIT 1) as latest_generation_at, \
     (SELECT i.thumbnail_path FROM generations g JOIN images i ON i.generation_id = g.id \
      WHERE g.conversation_id = c.id AND g.deleted_at IS NULL ORDER BY g.created_at DESC LIMIT 1) as latest_thumbnail \
     FROM conversations c \
     WHERE EXISTS (SELECT 1 FROM generations g WHERE g.conversation_id = c.id AND g.deleted_at IS NULL) \
        OR NOT EXISTS (SELECT 1 FROM generations g WHERE g.conversation_id = c.id) \
     ORDER BY c.updated_at DESC"
        .to_string()
}

fn row_to_conversation(row: &rusqlite::Row) -> rusqlite::Result<Conversation> {
    Ok(Conversation {
        id: row.get("id")?,
        title: row.get("title")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        generation_count: row.get("generation_count")?,
        latest_generation_at: row.get("latest_generation_at")?,
        latest_thumbnail: row.get("latest_thumbnail")?,
    })
}

fn fetch_conversation(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Result<Conversation, String> {
    conn.query_row(
        "SELECT c.id, c.title, c.created_at, c.updated_at, \
         (SELECT COUNT(*) FROM generations WHERE conversation_id = c.id AND deleted_at IS NULL) as generation_count, \
         (SELECT g.created_at FROM generations g \
          WHERE g.conversation_id = c.id AND g.deleted_at IS NULL ORDER BY g.created_at DESC LIMIT 1) as latest_generation_at, \
         (SELECT i.thumbnail_path FROM generations g JOIN images i ON i.generation_id = g.id \
          WHERE g.conversation_id = c.id AND g.deleted_at IS NULL ORDER BY g.created_at DESC LIMIT 1) as latest_thumbnail \
         FROM conversations c WHERE c.id = ?1",
        params![conversation_id],
        row_to_conversation,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_conversations(
    db: tauri::State<'_, Database>,
    query: Option<String>,
) -> Result<Vec<Conversation>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    // Backfill: assign orphaned generations to new conversations
    let mut orphans: Vec<(String, String, String)> = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT id, prompt, created_at FROM generations WHERE conversation_id IS NULL ORDER BY created_at ASC")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        for row in rows.flatten() {
            orphans.push(row);
        }
    }

    for (gen_id, prompt, created_at) in &orphans {
        let conv_id = uuid::Uuid::new_v4().to_string();
        let title: String = if prompt.chars().count() > 40 {
            prompt.chars().take(40).collect::<String>() + "..."
        } else {
            prompt.clone()
        };
        conn.execute(
            "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![conv_id, title, created_at, created_at],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE generations SET conversation_id = ?1 WHERE id = ?2",
            params![conv_id, gen_id],
        )
        .map_err(|e| e.to_string())?;
    }

    // Now query conversations
    let (sql, query_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(q) =
        &query
    {
        if !q.is_empty() {
            let pattern = format!("%{}%", q);
            (
                "SELECT c.id, c.title, c.created_at, c.updated_at, \
                 (SELECT COUNT(*) FROM generations WHERE conversation_id = c.id AND deleted_at IS NULL) as generation_count, \
                 (SELECT g.created_at FROM generations g \
                  WHERE g.conversation_id = c.id AND g.deleted_at IS NULL ORDER BY g.created_at DESC LIMIT 1) as latest_generation_at, \
                 (SELECT i.thumbnail_path FROM generations g JOIN images i ON i.generation_id = g.id \
                  WHERE g.conversation_id = c.id AND g.deleted_at IS NULL ORDER BY g.created_at DESC LIMIT 1) as latest_thumbnail \
                 FROM conversations c \
                 WHERE c.title LIKE ?1 AND (
                   EXISTS (SELECT 1 FROM generations g WHERE g.conversation_id = c.id AND g.deleted_at IS NULL) \
                   OR NOT EXISTS (SELECT 1 FROM generations g WHERE g.conversation_id = c.id)
                 ) \
                 ORDER BY c.updated_at DESC".to_string(),
                vec![Box::new(pattern)],
            )
        } else {
            (conversations_base_sql(), vec![])
        }
    } else {
        (conversations_base_sql(), vec![])
    };

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let params_refs: Vec<&dyn rusqlite::types::ToSql> =
        query_params.iter().map(|p| p.as_ref()).collect();
    let conversations = stmt
        .query_map(params_refs.as_slice(), row_to_conversation)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(conversations)
}

#[tauri::command]
fn get_conversation_generations(
    db: tauri::State<'_, Database>,
    conversation_id: String,
) -> Result<Vec<GenerationResult>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT * FROM generations WHERE conversation_id = ?1 AND deleted_at IS NULL ORDER BY created_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let generations: Vec<Generation> = stmt
        .query_map(params![conversation_id], row_to_generation)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let results = generation_results_with_images(&conn, generations)?;

    Ok(results)
}

// --- Lightbox commands ---

#[tauri::command]
fn copy_image_to_clipboard(image_path: String) -> Result<(), String> {
    let data = std::fs::read(&image_path).map_err(|e| format!("Read image failed: {}", e))?;
    let img = image::load_from_memory(&data).map_err(|e| format!("Decode image failed: {}", e))?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();

    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("Clipboard access failed: {}", e))?;
    clipboard
        .set_image(arboard::ImageData {
            width: w as usize,
            height: h as usize,
            bytes: std::borrow::Cow::Owned(rgba.into_raw()),
        })
        .map_err(|e| format!("Copy to clipboard failed: {}", e))?;

    Ok(())
}

#[tauri::command]
async fn save_image_to_file(image_path: String) -> Result<(), String> {
    let file_name = std::path::Path::new(&image_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image.png");

    let save_path = rfd::AsyncFileDialog::new()
        .set_file_name(file_name)
        .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
        .save_file()
        .await
        .ok_or_else(|| "Save cancelled".to_string())?;

    let save_path = save_path.path().to_path_buf();
    tokio::task::spawn_blocking(move || {
        std::fs::copy(&image_path, &save_path)
            .map(|_| ())
            .map_err(|e| format!("Save failed: {}", e))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn pick_source_images() -> Result<Vec<String>, String> {
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

// --- Prompt favorite commands ---

#[tauri::command]
fn create_prompt_favorite(
    db: tauri::State<'_, Database>,
    prompt: String,
) -> Result<PromptFavorite, String> {
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("Prompt cannot be empty".to_string());
    }

    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let timestamp = current_timestamp();

    if let Some(existing) = conn
        .query_row(
            "SELECT id, prompt, created_at, updated_at FROM prompt_favorites WHERE prompt = ?1 COLLATE NOCASE",
            params![&prompt],
            row_to_prompt_favorite,
        )
        .optional()
        .map_err(|e| e.to_string())?
    {
        conn.execute(
            "UPDATE prompt_favorites SET prompt = ?1, updated_at = ?2 WHERE id = ?3",
            params![&prompt, &timestamp, &existing.id],
        )
        .map_err(|e| e.to_string())?;

        return Ok(PromptFavorite {
            prompt,
            updated_at: timestamp,
            ..existing
        });
    }

    let id = uuid::Uuid::new_v4().to_string();
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO prompt_favorites (id, prompt, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![&id, &prompt, &timestamp],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT OR IGNORE INTO prompt_folder_favorites (folder_id, prompt_favorite_id, added_at) VALUES ('default', ?1, ?2)",
        params![&id, &timestamp],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;

    Ok(PromptFavorite {
        id,
        prompt,
        created_at: timestamp.clone(),
        updated_at: timestamp,
    })
}

#[tauri::command]
fn get_prompt_favorites(
    db: tauri::State<'_, Database>,
    query: Option<String>,
    folder_id: Option<String>,
) -> Result<Vec<PromptFavorite>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let query = query
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let folder_id = folder_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    match (query, folder_id) {
        (Some(query), Some(folder_id)) => {
            let pattern = format!("%{}%", query);
            let mut stmt = conn
                .prepare(
                    "SELECT pf.id, pf.prompt, pf.created_at, pf.updated_at
                     FROM prompt_favorites pf
                     JOIN prompt_folder_favorites pff ON pff.prompt_favorite_id = pf.id
                     WHERE pff.folder_id = ?1 AND pf.prompt LIKE ?2
                     ORDER BY pff.added_at DESC, pf.updated_at DESC",
                )
                .map_err(|e| e.to_string())?;
            let favorites = stmt
                .query_map(params![folder_id, pattern], row_to_prompt_favorite)
                .map_err(|e| e.to_string())?
                .filter_map(|row| row.ok())
                .collect();
            Ok(favorites)
        }
        (None, Some(folder_id)) => {
            let mut stmt = conn
                .prepare(
                    "SELECT pf.id, pf.prompt, pf.created_at, pf.updated_at
                     FROM prompt_favorites pf
                     JOIN prompt_folder_favorites pff ON pff.prompt_favorite_id = pf.id
                     WHERE pff.folder_id = ?1
                     ORDER BY pff.added_at DESC, pf.updated_at DESC",
                )
                .map_err(|e| e.to_string())?;
            let favorites = stmt
                .query_map(params![folder_id], row_to_prompt_favorite)
                .map_err(|e| e.to_string())?
                .filter_map(|row| row.ok())
                .collect();
            Ok(favorites)
        }
        (Some(query), None) => {
            let pattern = format!("%{}%", query);
            let mut stmt = conn
                .prepare(
                    "SELECT id, prompt, created_at, updated_at FROM prompt_favorites
                     WHERE prompt LIKE ?1
                     ORDER BY updated_at DESC, created_at DESC",
                )
                .map_err(|e| e.to_string())?;
            let favorites = stmt
                .query_map(params![pattern], row_to_prompt_favorite)
                .map_err(|e| e.to_string())?
                .filter_map(|row| row.ok())
                .collect();
            Ok(favorites)
        }
        (None, None) => {
            let mut stmt = conn
                .prepare(
                    "SELECT id, prompt, created_at, updated_at FROM prompt_favorites
                     ORDER BY updated_at DESC, created_at DESC",
                )
                .map_err(|e| e.to_string())?;
            let favorites = stmt
                .query_map([], row_to_prompt_favorite)
                .map_err(|e| e.to_string())?
                .filter_map(|row| row.ok())
                .collect();
            Ok(favorites)
        }
    }
}

#[tauri::command]
fn delete_prompt_favorite(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM prompt_favorites WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn create_prompt_folder(db: tauri::State<'_, Database>, name: String) -> Result<Folder, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Folder name cannot be empty".to_string());
    }

    let id = uuid::Uuid::new_v4().to_string();
    let created_at = current_timestamp();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO prompt_folders (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![id, name, &created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(Folder {
        id,
        name,
        created_at,
    })
}

#[tauri::command]
fn rename_prompt_folder(
    db: tauri::State<'_, Database>,
    id: String,
    name: String,
) -> Result<(), String> {
    if id == "default" {
        return Err("Default folder cannot be renamed".to_string());
    }
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Folder name cannot be empty".to_string());
    }
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE prompt_folders SET name = ?1 WHERE id = ?2",
        params![name, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn delete_prompt_folder(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    if id == "default" {
        return Err("Default folder cannot be deleted".to_string());
    }
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM prompt_folders WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_prompt_folders(db: tauri::State<'_, Database>) -> Result<Vec<Folder>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, created_at FROM prompt_folders ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;
    let folders = stmt
        .query_map([], |row| {
            Ok(Folder {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|row| row.ok())
        .collect();
    Ok(folders)
}

#[tauri::command]
fn add_prompt_favorite_to_folders(
    db: tauri::State<'_, Database>,
    favorite_id: String,
    folder_ids: Vec<String>,
) -> Result<(), String> {
    let added_at = current_timestamp();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    for folder_id in &folder_ids {
        conn.execute(
            "INSERT OR IGNORE INTO prompt_folder_favorites (folder_id, prompt_favorite_id, added_at) VALUES (?1, ?2, ?3)",
            params![folder_id, favorite_id, &added_at],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn remove_prompt_favorite_from_folders(
    db: tauri::State<'_, Database>,
    favorite_id: String,
    folder_ids: Vec<String>,
) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    for folder_id in &folder_ids {
        conn.execute(
            "DELETE FROM prompt_folder_favorites WHERE folder_id = ?1 AND prompt_favorite_id = ?2",
            params![folder_id, favorite_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn get_prompt_favorite_folders(
    db: tauri::State<'_, Database>,
    favorite_id: String,
) -> Result<Vec<String>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT folder_id FROM prompt_folder_favorites WHERE prompt_favorite_id = ?1")
        .map_err(|e| e.to_string())?;
    let folder_ids = stmt
        .query_map(params![favorite_id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|row| row.ok())
        .collect();
    Ok(folder_ids)
}

// --- Folder commands ---

#[tauri::command]
fn create_folder(db: tauri::State<'_, Database>, name: String) -> Result<Folder, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = current_timestamp();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO folders (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![id, name, &created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(Folder {
        id,
        name,
        created_at,
    })
}

#[tauri::command]
fn rename_folder(db: tauri::State<'_, Database>, id: String, name: String) -> Result<(), String> {
    if id == "default" {
        return Err("默认收藏文件夹不可重命名".to_string());
    }
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE folders SET name = ?1 WHERE id = ?2",
        params![name, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn delete_folder(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    if id == "default" {
        return Err("默认收藏文件夹不可删除".to_string());
    }
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM folders WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_folders(db: tauri::State<'_, Database>) -> Result<Vec<Folder>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, created_at FROM folders ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;
    let folders = stmt
        .query_map([], |row| {
            Ok(Folder {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(folders)
}

#[tauri::command]
fn add_image_to_folders(
    db: tauri::State<'_, Database>,
    image_id: String,
    folder_ids: Vec<String>,
) -> Result<(), String> {
    let added_at = current_timestamp();
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    for fid in &folder_ids {
        conn.execute(
            "INSERT OR IGNORE INTO folder_images (folder_id, image_id, added_at) VALUES (?1, ?2, ?3)",
            params![fid, image_id, &added_at],
        ).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn remove_image_from_folders(
    db: tauri::State<'_, Database>,
    image_id: String,
    folder_ids: Vec<String>,
) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    for fid in &folder_ids {
        conn.execute(
            "DELETE FROM folder_images WHERE folder_id = ?1 AND image_id = ?2",
            params![fid, image_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn get_image_folders(
    db: tauri::State<'_, Database>,
    image_id: String,
) -> Result<Vec<String>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT folder_id FROM folder_images WHERE image_id = ?1")
        .map_err(|e| e.to_string())?;
    let folder_ids = stmt
        .query_map(params![image_id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(folder_ids)
}

#[tauri::command]
fn get_favorite_images(
    db: tauri::State<'_, Database>,
    folder_id: Option<String>,
    query: Option<String>,
    page: Option<i32>,
) -> Result<SearchResult, String> {
    let page = page.unwrap_or(1);
    let page_size = DEFAULT_PAGE_SIZE;
    let offset = (page - 1) * page_size;
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    // Build base query
    let (count, generations) = if let Some(fid) = &folder_id {
        let pattern = query.as_ref().map(|q| format!("%{}%", q));
        let (cnt, gens) = if let Some(p) = &pattern {
            let count: i32 = conn
                .query_row(
                    "SELECT COUNT(DISTINCT g.id) FROM generations g
                 JOIN images i ON i.generation_id = g.id
                 JOIN folder_images fi ON fi.image_id = i.id
                 WHERE g.deleted_at IS NULL AND fi.folder_id = ?1 AND g.prompt LIKE ?2",
                    params![fid, p],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            let mut stmt = conn
                .prepare(
                    "SELECT DISTINCT g.* FROM generations g
                 JOIN images i ON i.generation_id = g.id
                 JOIN folder_images fi ON fi.image_id = i.id
                 WHERE g.deleted_at IS NULL AND fi.folder_id = ?1 AND g.prompt LIKE ?2
                 ORDER BY fi.added_at DESC LIMIT ?3 OFFSET ?4",
                )
                .map_err(|e| e.to_string())?;
            let gens: Vec<Generation> = stmt
                .query_map(params![fid, p, page_size, offset], row_to_generation)
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect();
            (count, gens)
        } else {
            let count: i32 = conn
                .query_row(
                    "SELECT COUNT(DISTINCT g.id) FROM generations g
                 JOIN images i ON i.generation_id = g.id
                 JOIN folder_images fi ON fi.image_id = i.id
                 WHERE g.deleted_at IS NULL AND fi.folder_id = ?1",
                    params![fid],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            let mut stmt = conn
                .prepare(
                    "SELECT DISTINCT g.* FROM generations g
                 JOIN images i ON i.generation_id = g.id
                 JOIN folder_images fi ON fi.image_id = i.id
                 WHERE g.deleted_at IS NULL AND fi.folder_id = ?1
                 ORDER BY fi.added_at DESC LIMIT ?2 OFFSET ?3",
                )
                .map_err(|e| e.to_string())?;
            let gens = stmt
                .query_map(params![fid, page_size, offset], row_to_generation)
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect();
            (count, gens)
        };
        (cnt, gens)
    } else if let Some(q) = &query {
        if !q.is_empty() {
            let pattern = format!("%{}%", q);
            let count: i32 = conn
                .query_row(
                    "SELECT COUNT(DISTINCT g.id) FROM generations g
                 JOIN images i ON i.generation_id = g.id
                 JOIN folder_images fi ON fi.image_id = i.id
                 WHERE g.deleted_at IS NULL AND g.prompt LIKE ?1",
                    params![&pattern],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            let mut stmt = conn
                .prepare(
                    "SELECT DISTINCT g.* FROM generations g
                 JOIN images i ON i.generation_id = g.id
                 JOIN folder_images fi ON fi.image_id = i.id
                 WHERE g.deleted_at IS NULL AND g.prompt LIKE ?1
                 ORDER BY g.created_at DESC LIMIT ?2 OFFSET ?3",
                )
                .map_err(|e| e.to_string())?;
            let gens = stmt
                .query_map(params![&pattern, page_size, offset], row_to_generation)
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect();
            (count, gens)
        } else {
            return get_all_favorites(&conn, page, page_size, offset);
        }
    } else {
        return get_all_favorites(&conn, page, page_size, offset);
    };

    let results = generation_results_with_images(&conn, generations)?;

    Ok(SearchResult {
        generations: results,
        total: count,
        page,
        page_size,
    })
}

fn get_all_favorites(
    conn: &Connection,
    page: i32,
    page_size: i32,
    offset: i32,
) -> Result<SearchResult, String> {
    let count: i32 = conn
        .query_row(
            "SELECT COUNT(DISTINCT g.id) FROM generations g
         JOIN images i ON i.generation_id = g.id
         JOIN folder_images fi ON fi.image_id = i.id
         WHERE g.deleted_at IS NULL",
            [],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT g.* FROM generations g
         JOIN images i ON i.generation_id = g.id
         JOIN folder_images fi ON fi.image_id = i.id
         WHERE g.deleted_at IS NULL
         ORDER BY g.created_at DESC LIMIT ?1 OFFSET ?2",
        )
        .map_err(|e| e.to_string())?;
    let generations: Vec<Generation> = stmt
        .query_map(params![page_size, offset], row_to_generation)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let results = generation_results_with_images(conn, generations)?;

    Ok(SearchResult {
        generations: results,
        total: count,
        page,
        page_size,
    })
}

// --- Log commands ---

#[tauri::command]
fn get_logs(
    db: tauri::State<'_, Database>,
    log_type: Option<String>,
    level: Option<String>,
    page: Option<i32>,
    page_size: Option<i32>,
) -> Result<LogSearchResult, String> {
    let page = page.unwrap_or(1);
    let page_size = page_size.unwrap_or(DEFAULT_PAGE_SIZE);
    let (logs, total) = db.search_logs(log_type.as_deref(), level.as_deref(), page, page_size)?;
    Ok(LogSearchResult {
        logs,
        total,
        page,
        page_size,
    })
}

#[tauri::command]
fn get_runtime_logs(limit: Option<usize>) -> Result<Vec<RuntimeLogEntry>, String> {
    Ok(runtime_logs::recent_logs(limit.unwrap_or(200)))
}

#[tauri::command]
fn get_log_detail(db: tauri::State<'_, Database>, id: String) -> Result<LogEntry, String> {
    db.get_log(&id)?.ok_or_else(|| "Log not found".to_string())
}

#[tauri::command]
fn read_log_response_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("Read failed: {}", e))
}

#[tauri::command]
fn clear_logs(db: tauri::State<'_, Database>, before_days: Option<u32>) -> Result<u64, String> {
    let days = before_days.unwrap_or(DEFAULT_LOG_RETENTION_DAYS);
    let before_str = format_log_clear_cutoff(Utc::now(), days);
    db.clear_logs_before(&before_str)
}

#[tauri::command]
fn get_log_settings(db: tauri::State<'_, Database>) -> Result<LogSettings, String> {
    let enabled = db
        .get_setting(SETTING_LOG_ENABLED)?
        .map(|v| v == "true")
        .unwrap_or(true);
    let retention_days = db
        .get_setting(SETTING_LOG_RETENTION_DAYS)?
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(DEFAULT_LOG_RETENTION_DAYS);
    Ok(LogSettings {
        enabled,
        retention_days,
    })
}

#[tauri::command]
fn save_log_settings(
    db: tauri::State<'_, Database>,
    enabled: bool,
    retention_days: u32,
) -> Result<(), String> {
    db.set_setting(SETTING_LOG_ENABLED, if enabled { "true" } else { "false" })?;
    db.set_setting(SETTING_LOG_RETENTION_DAYS, &retention_days.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_trash_settings(db: tauri::State<'_, Database>) -> Result<TrashSettings, String> {
    Ok(TrashSettings {
        retention_days: db.get_trash_retention_days()?,
    })
}

#[tauri::command]
fn save_trash_settings(
    app: tauri::AppHandle,
    db: tauri::State<'_, Database>,
    retention_days: u32,
) -> Result<(), String> {
    let retention_days = retention_days.max(1);
    db.set_setting(SETTING_TRASH_RETENTION_DAYS, &retention_days.to_string())?;
    let _ = purge_trashed_generations(&app, db.inner(), retention_days);
    Ok(())
}

// --- App entry point ---

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_config = config::AppConfig::load();
    config::init_logger(&app_config.log);

    let db_path = dirs::data_dir()
        .expect("Cannot determine app data directory")
        .join("astro-studio")
        .join("astro_studio.db");

    log::info!("Database path: {}", db_path.display());
    log::info!(
        "Config path: {}",
        config::AppConfig::config_path().display()
    );

    let database = Database::open(&db_path).expect("Failed to open database");
    database.run_migrations().expect("Failed to run migrations");

    let engine = api_gateway::GptImageEngine::new(&app_config.api);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_config)
        .manage(database)
        .manage(engine)
        .invoke_handler(tauri::generate_handler![
            save_api_key,
            get_api_key,
            get_model_api_key,
            save_model_api_key,
            save_base_url,
            get_base_url,
            get_endpoint_settings,
            save_endpoint_settings,
            get_model_endpoint_settings,
            save_model_endpoint_settings,
            get_font_size,
            save_font_size,
            get_image_model,
            save_image_model,
            generate_image,
            edit_image,
            search_generations,
            delete_generation,
            restore_generation,
            permanently_delete_generation,
            create_conversation,
            get_conversations,
            get_conversation_generations,
            copy_image_to_clipboard,
            save_image_to_file,
            pick_source_images,
            create_prompt_favorite,
            get_prompt_favorites,
            delete_prompt_favorite,
            create_prompt_folder,
            rename_prompt_folder,
            delete_prompt_folder,
            get_prompt_folders,
            add_prompt_favorite_to_folders,
            remove_prompt_favorite_from_folders,
            get_prompt_favorite_folders,
            create_folder,
            rename_folder,
            delete_folder,
            get_folders,
            add_image_to_folders,
            remove_image_from_folders,
            get_image_folders,
            get_favorite_images,
            get_logs,
            get_runtime_logs,
            get_log_detail,
            read_log_response_file,
            clear_logs,
            get_log_settings,
            save_log_settings,
            get_trash_settings,
            save_trash_settings,
        ])
        .setup(|app| {
            runtime_logs::attach_app_handle(app.handle().clone());

            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Cannot determine app data dir");
            let fm = file_manager::FileManager::new(app_data_dir);
            fm.ensure_dirs()?;

            {
                let db = app.state::<Database>();
                let engine = app.state::<api_gateway::GptImageEngine>();
                tauri::async_runtime::block_on(recover_interrupted_generations(
                    &app.handle(),
                    db.inner(),
                    engine.inner(),
                ))?;
            }

            // Startup log cleanup
            {
                let db = app.state::<Database>();
                let enabled = db
                    .get_setting(SETTING_LOG_ENABLED)
                    .ok()
                    .flatten()
                    .map(|v| v == "true")
                    .unwrap_or(true);
                if enabled {
                    let days = db
                        .get_setting(SETTING_LOG_RETENTION_DAYS)
                        .ok()
                        .flatten()
                        .and_then(|v| v.parse::<u32>().ok())
                        .unwrap_or(DEFAULT_LOG_RETENTION_DAYS);
                    let before = chrono::Local::now() - chrono::Duration::days(days as i64);
                    let before_str = before.format("%Y-%m-%d %H:%M:%S").to_string();
                    let _ = db.clear_logs_before(&before_str);
                }
            }

            {
                let db = app.state::<Database>();
                let retention_days = db
                    .get_trash_retention_days()
                    .unwrap_or(DEFAULT_TRASH_RETENTION_DAYS);
                let _ = purge_trashed_generations(&app.handle(), db.inner(), retention_days);
            }

            #[cfg(target_os = "windows")]
            {
                use window_vibrancy::apply_mica;
                let window = app.get_webview_window("main").unwrap();
                let _ = apply_mica(&window, None);
            }
            #[cfg(target_os = "macos")]
            {
                use window_vibrancy::apply_vibrancy;
                use window_vibrancy::NSVisualEffectMaterial;
                let window = app.get_webview_window("main").unwrap();
                let _ = apply_vibrancy(&window, NSVisualEffectMaterial::HudWindow, None, None);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_, _event| {
            #[cfg(all(debug_assertions, target_os = "macos"))]
            if matches!(_event, tauri::RunEvent::Ready) {
                set_macos_development_app_icon();
            }
        });
}
