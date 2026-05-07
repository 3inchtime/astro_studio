mod api_gateway;
mod config;
mod db;
mod file_manager;
mod gallery;
mod image_engines;
mod model_registry;
mod models;
mod runtime_logs;

use api_gateway::ImageEngine;
use chrono::{SecondsFormat, Utc};
use db::Database;
use model_registry::{
    default_endpoint_settings_for_model, endpoint_value_or_default,
    image_endpoint_url_for_model_settings, is_gemini_model, legacy_model_setting_ids,
    model_setting_key, normalize_endpoint_mode, normalize_image_model,
    sanitize_request_options_for_model, ImageEndpointKind,
};
use models::*;
use rusqlite::{params, Connection, OptionalExtension};
use tauri::{Emitter, Manager};

const DEFAULT_PROJECT_ID: &str = "default";
const DEFAULT_PROJECT_NAME: &str = "Default Project";
const DEFAULT_NEW_PROJECT_NAME: &str = "New Project";
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

fn normalize_font_size(font_size: &str) -> &'static str {
    match font_size {
        "small" => "small",
        "large" => "large",
        _ => DEFAULT_FONT_SIZE,
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
    Ok(
        get_model_setting(db, model, SETTING_API_KEY, Some(SETTING_API_KEY))?
            .map(|key| normalize_api_key_for_storage(&key)),
    )
}

fn normalize_api_key_for_storage(key: &str) -> String {
    let trimmed = key.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let prefix = parts.next().unwrap_or_default();

    if prefix.eq_ignore_ascii_case("bearer") {
        return parts.next().unwrap_or_default().trim().to_string();
    }

    trimmed.to_string()
}

fn save_model_api_key_value(db: &Database, model: &str, key: &str) -> Result<(), String> {
    let key = normalize_api_key_for_storage(key);
    set_model_setting(db, model, SETTING_API_KEY, &key)?;
    if normalize_image_model(model) == ENGINE_GPT_IMAGE_2 {
        db.set_setting(SETTING_API_KEY, &key)?;
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
            read_model_api_key(&db, ENGINE_GPT_IMAGE_2).unwrap(),
            Some("sk-proj-valid-token".to_string())
        );

        drop(db);
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
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

fn source_image_paths_json(source_image_paths: &[String]) -> Result<String, String> {
    serde_json::to_string(source_image_paths)
        .map_err(|e| format!("Serialize source image paths failed: {}", e))
}

fn generation_request_metadata_json(
    request_kind: &str,
    conversation_id: &str,
    model: &str,
    options: &GptImageRequestOptions,
    source_image_paths: &[String],
) -> Result<String, String> {
    serde_json::to_string(&serde_json::json!({
        "request_kind": request_kind,
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
        "source_image_paths": source_image_paths,
    }))
    .map_err(|e| format!("Serialize generation metadata failed: {}", e))
}

fn create_processing_generation(
    conn: &Connection,
    generation_id: &str,
    prompt: &str,
    model: &str,
    options: &GptImageRequestOptions,
    conversation_id: &str,
    created_at: &str,
    request_kind: &str,
    source_image_paths: &[String],
) -> Result<(), String> {
    let source_image_paths_json = source_image_paths_json(source_image_paths)?;
    let request_metadata = generation_request_metadata_json(
        request_kind,
        conversation_id,
        model,
        options,
        source_image_paths,
    )?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
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
            request_kind,
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
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO generation_recoveries (generation_id, request_kind, request_state, output_format, response_file, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?5)",
        params![
            generation_id,
            request_kind,
            RECOVERY_STATE_REQUESTING,
            &options.output_format,
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

fn normalized_path_extension(path: &std::path::Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            if extension.eq_ignore_ascii_case("jpg") {
                "jpeg".to_string()
            } else {
                extension.to_ascii_lowercase()
            }
        })
}

fn repaired_image_path(path: &std::path::Path, extension: &str) -> std::path::PathBuf {
    let candidate = path.with_extension(extension);
    if !candidate.exists() {
        return candidate;
    }

    let parent = path.parent().unwrap_or_else(|| std::path::Path::new(""));
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("image");

    for index in 1..1000 {
        let candidate = parent.join(format!("{stem}_format_{index}.{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    parent.join(format!(
        "{}_format_{}.{}",
        stem,
        uuid::Uuid::new_v4(),
        extension
    ))
}

fn target_repair_output_format(
    engine: &str,
    path: &std::path::Path,
    detected_extension: &str,
) -> &'static str {
    if is_gemini_model(engine) && normalized_path_extension(path).as_deref() != Some("png") {
        return "png";
    }

    normalized_path_extension(path)
        .as_deref()
        .and_then(file_manager::output_format_for_extension)
        .or_else(|| file_manager::output_format_for_extension(detected_extension))
        .unwrap_or("png")
}

fn repair_mismatched_image_extensions(db: &Database) -> Result<usize, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let rows: Vec<(String, String, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT i.id, i.file_path, g.engine
                 FROM images i
                 JOIN generations g ON g.id = i.generation_id",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(|e| e.to_string())?
            .filter_map(|row| row.ok())
            .collect();
        rows
    };

    let mut repaired = 0;
    for (image_id, file_path, engine) in rows {
        let path = std::path::PathBuf::from(&file_path);
        if !path.is_file() {
            continue;
        }

        let data = match std::fs::read(&path) {
            Ok(data) => data,
            Err(e) => {
                log::warn!(
                    "Failed to read image for extension repair: {} ({})",
                    file_path,
                    e
                );
                continue;
            }
        };
        let Some(detected_extension) = file_manager::detected_image_extension(&data) else {
            continue;
        };
        let output_format = target_repair_output_format(&engine, &path, detected_extension);
        let target_extension = file_manager::extension_for_output_format(output_format);

        if normalized_path_extension(&path).as_deref() == Some(target_extension)
            && detected_extension == target_extension
        {
            continue;
        }

        let img =
            image::load_from_memory(&data).map_err(|e| format!("Decode image failed: {}", e))?;
        let next_path = if normalized_path_extension(&path).as_deref() == Some(target_extension) {
            path.clone()
        } else {
            repaired_image_path(&path, target_extension)
        };
        let file_size =
            file_manager::write_image_in_output_format(&img, &next_path, output_format)?;
        if next_path != path {
            let _ = std::fs::remove_file(&path);
        }
        let next_path_str = next_path.to_string_lossy().to_string();
        if let Err(e) = conn.execute(
            "UPDATE images SET file_path = ?1, file_size = ?2 WHERE id = ?3",
            params![next_path_str, file_size, image_id],
        ) {
            if next_path != path {
                let _ = std::fs::rename(&next_path, &path);
            }
            return Err(e.to_string());
        }
        repaired += 1;
    }

    if repaired > 0 {
        log::info!("Repaired {} image file extension mismatch(es)", repaired);
    }

    Ok(repaired)
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
    project_id: Option<String>,
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
        resolve_conversation_id(
            &conn,
            conversation_id.as_deref(),
            project_id.as_deref(),
            &prompt,
        )?
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
            &options,
            &conversation_id,
            &created_at,
            RECOVERY_KIND_GENERATE,
            &[],
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
    project_id: Option<String>,
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
        resolve_conversation_id(
            &conn,
            conversation_id.as_deref(),
            project_id.as_deref(),
            &prompt,
        )?
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
            &options,
            &conversation_id,
            &created_at,
            RECOVERY_KIND_EDIT,
            &source_image_paths,
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

fn row_to_prompt_favorite(row: &rusqlite::Row) -> rusqlite::Result<PromptFavorite> {
    Ok(PromptFavorite {
        id: row.get("id")?,
        prompt: row.get("prompt")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

#[tauri::command]
fn create_project(db: tauri::State<'_, Database>, name: Option<String>) -> Result<Project, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let project_id = uuid::Uuid::new_v4().to_string();
    let timestamp = current_timestamp();
    let name = name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_NEW_PROJECT_NAME);

    conn.execute(
        "INSERT INTO projects (id, name, created_at, updated_at, archived_at, pinned_at, deleted_at)
         VALUES (?1, ?2, ?3, ?3, NULL, NULL, NULL)",
        params![project_id, name, &timestamp],
    )
    .map_err(|e| e.to_string())?;

    fetch_project(&conn, &project_id)
}

#[tauri::command]
fn get_projects(
    db: tauri::State<'_, Database>,
    include_archived: Option<bool>,
) -> Result<Vec<Project>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    ensure_default_project(&conn)?;

    let include_archived = include_archived.unwrap_or(false);
    let sql = if include_archived {
        projects_base_sql("p.deleted_at IS NULL")
    } else {
        projects_base_sql("p.deleted_at IS NULL AND p.archived_at IS NULL")
    };

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let projects = stmt
        .query_map([], row_to_project)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(projects)
}

#[tauri::command]
fn rename_project(db: tauri::State<'_, Database>, id: String, name: String) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Project name cannot be empty.".to_string());
    }

    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE projects SET name = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
        params![name, current_timestamp(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn archive_project(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let timestamp = current_timestamp();
    conn.execute(
        "UPDATE projects SET archived_at = COALESCE(archived_at, ?1), updated_at = ?1
         WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE conversations SET archived_at = COALESCE(archived_at, ?1), updated_at = ?1
         WHERE project_id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn unarchive_project(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let timestamp = current_timestamp();
    conn.execute(
        "UPDATE projects SET archived_at = NULL, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE conversations SET archived_at = NULL, updated_at = ?1
         WHERE project_id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn pin_project(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let timestamp = current_timestamp();
    conn.execute(
        "UPDATE projects SET pinned_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn unpin_project(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE projects SET pinned_at = NULL, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![current_timestamp(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn delete_project(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    if id == DEFAULT_PROJECT_ID {
        return Err("The default project cannot be deleted.".to_string());
    }

    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let timestamp = current_timestamp();
    conn.execute(
        "UPDATE projects SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE conversations SET deleted_at = ?1, updated_at = ?1 WHERE project_id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE generations SET deleted_at = COALESCE(deleted_at, ?1)
         WHERE conversation_id IN (SELECT id FROM conversations WHERE project_id = ?2)",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn ensure_default_project(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO projects (id, name, created_at, updated_at)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
        params![DEFAULT_PROJECT_ID, DEFAULT_PROJECT_NAME],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn resolve_project_id(
    conn: &rusqlite::Connection,
    project_id: Option<&str>,
) -> Result<String, String> {
    ensure_default_project(conn)?;

    if let Some(project_id) = project_id.map(str::trim).filter(|id| !id.is_empty()) {
        let exists = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND deleted_at IS NULL)",
                params![project_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?
            != 0;

        if exists {
            return Ok(project_id.to_string());
        }
    }

    Ok(DEFAULT_PROJECT_ID.to_string())
}

fn projects_base_sql(where_sql: &str) -> String {
    format!(
        "SELECT p.id, p.name, p.created_at, p.updated_at, p.archived_at, p.pinned_at, p.deleted_at, \
         (SELECT COUNT(*) FROM conversations c \
          WHERE c.project_id = p.id AND c.deleted_at IS NULL AND c.archived_at IS NULL) as conversation_count, \
         (SELECT COUNT(i.id) FROM conversations c \
          JOIN generations g ON g.conversation_id = c.id \
          JOIN images i ON i.generation_id = g.id \
          WHERE c.project_id = p.id AND c.deleted_at IS NULL AND c.archived_at IS NULL AND g.deleted_at IS NULL) as image_count \
         FROM projects p \
         WHERE {} \
         ORDER BY CASE WHEN p.pinned_at IS NULL THEN 1 ELSE 0 END, p.pinned_at DESC, p.updated_at DESC",
        where_sql
    )
}

fn row_to_project(row: &rusqlite::Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get("id")?,
        name: row.get("name")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        archived_at: row.get("archived_at")?,
        pinned_at: row.get("pinned_at")?,
        deleted_at: row.get("deleted_at")?,
        conversation_count: row.get("conversation_count")?,
        image_count: row.get("image_count")?,
    })
}

fn fetch_project(conn: &rusqlite::Connection, project_id: &str) -> Result<Project, String> {
    conn.query_row(
        &projects_base_sql("p.id = ?1"),
        params![project_id],
        row_to_project,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn create_conversation(
    db: tauri::State<'_, Database>,
    title: Option<String>,
    project_id: Option<String>,
) -> Result<Conversation, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let project_id = resolve_project_id(&conn, project_id.as_deref())?;
    let conv_id = uuid::Uuid::new_v4().to_string();
    let timestamp = current_timestamp();
    let title = title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_CONVERSATION_TITLE);

    conn.execute(
        "INSERT INTO conversations (id, project_id, title, created_at, updated_at, archived_at, pinned_at, deleted_at)
         VALUES (?1, ?2, ?3, ?4, ?4, NULL, NULL, NULL)",
        params![conv_id, project_id, title, &timestamp],
    )
    .map_err(|e| e.to_string())?;

    fetch_conversation(&conn, &conv_id)
}

fn resolve_conversation_id(
    conn: &rusqlite::Connection,
    conversation_id: Option<&str>,
    project_id: Option<&str>,
    prompt: &str,
) -> Result<String, String> {
    let project_id = resolve_project_id(conn, project_id)?;

    if let Some(conv_id) = conversation_id.map(str::trim).filter(|id| !id.is_empty()) {
        let exists = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1 AND deleted_at IS NULL)",
                params![conv_id],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|e| e.to_string())?
            != 0;

        if !exists {
            return create_new_conversation(conn, prompt, &project_id);
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
                    "UPDATE conversations SET title = ?1, project_id = ?2, archived_at = NULL, updated_at = ?3 WHERE id = ?4",
                    params![conversation_title_from_prompt(prompt), project_id, timestamp, conv_id],
                )
                .map_err(|e| e.to_string())?;
                return Ok(conv_id.to_string());
            }

            let timestamp = current_timestamp();
            conn.execute(
                "UPDATE conversations SET archived_at = NULL, updated_at = ?1 WHERE id = ?2",
                params![timestamp, conv_id],
            )
            .map_err(|e| e.to_string())?;
            return Ok(conv_id.to_string());
        }
    }

    create_new_conversation(conn, prompt, &project_id)
}

fn create_new_conversation(
    conn: &rusqlite::Connection,
    prompt: &str,
    project_id: &str,
) -> Result<String, String> {
    let conv_id = uuid::Uuid::new_v4().to_string();
    let title = conversation_title_from_prompt(prompt);
    let timestamp = current_timestamp();
    conn.execute(
        "INSERT INTO conversations (id, project_id, title, created_at, updated_at, archived_at, pinned_at, deleted_at)
         VALUES (?1, ?2, ?3, ?4, ?4, NULL, NULL, NULL)",
        params![conv_id, project_id, title, &timestamp],
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

fn conversations_list_sql(where_sql: &str) -> String {
    format!(
        "SELECT c.id, COALESCE(c.project_id, 'default') as project_id, p.name as project_name, c.title, c.created_at, c.updated_at, \
         c.archived_at, c.pinned_at, c.deleted_at, \
         (SELECT COUNT(*) FROM generations WHERE conversation_id = c.id AND deleted_at IS NULL) as generation_count, \
         (SELECT g.created_at FROM generations g \
          WHERE g.conversation_id = c.id AND g.deleted_at IS NULL ORDER BY g.created_at DESC LIMIT 1) as latest_generation_at, \
         (SELECT i.thumbnail_path FROM generations g JOIN images i ON i.generation_id = g.id \
          WHERE g.conversation_id = c.id AND g.deleted_at IS NULL ORDER BY g.created_at DESC LIMIT 1) as latest_thumbnail \
         FROM conversations c \
         LEFT JOIN projects p ON p.id = c.project_id \
         WHERE {} AND ( \
           EXISTS (SELECT 1 FROM generations g WHERE g.conversation_id = c.id AND g.deleted_at IS NULL) \
           OR NOT EXISTS (SELECT 1 FROM generations g WHERE g.conversation_id = c.id) \
         ) \
         ORDER BY CASE WHEN c.pinned_at IS NULL THEN 1 ELSE 0 END, c.pinned_at DESC, c.updated_at DESC",
        where_sql
    )
}

fn row_to_conversation(row: &rusqlite::Row) -> rusqlite::Result<Conversation> {
    Ok(Conversation {
        id: row.get("id")?,
        project_id: row.get("project_id")?,
        project_name: row.get("project_name")?,
        title: row.get("title")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        archived_at: row.get("archived_at")?,
        pinned_at: row.get("pinned_at")?,
        deleted_at: row.get("deleted_at")?,
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
        "SELECT c.id, COALESCE(c.project_id, 'default') as project_id, p.name as project_name, c.title, c.created_at, c.updated_at, \
         c.archived_at, c.pinned_at, c.deleted_at, \
         (SELECT COUNT(*) FROM generations WHERE conversation_id = c.id AND deleted_at IS NULL) as generation_count, \
         (SELECT g.created_at FROM generations g \
          WHERE g.conversation_id = c.id AND g.deleted_at IS NULL ORDER BY g.created_at DESC LIMIT 1) as latest_generation_at, \
         (SELECT i.thumbnail_path FROM generations g JOIN images i ON i.generation_id = g.id \
          WHERE g.conversation_id = c.id AND g.deleted_at IS NULL ORDER BY g.created_at DESC LIMIT 1) as latest_thumbnail \
         FROM conversations c LEFT JOIN projects p ON p.id = c.project_id WHERE c.id = ?1",
        params![conversation_id],
        row_to_conversation,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_conversations(
    db: tauri::State<'_, Database>,
    query: Option<String>,
    project_id: Option<String>,
    include_archived: Option<bool>,
) -> Result<Vec<Conversation>, String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    ensure_default_project(&conn)?;

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
            "INSERT INTO conversations (id, project_id, title, created_at, updated_at, archived_at, pinned_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?4, NULL, NULL, NULL)",
            params![conv_id, DEFAULT_PROJECT_ID, title, created_at],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE generations SET conversation_id = ?1 WHERE id = ?2",
            params![conv_id, gen_id],
        )
        .map_err(|e| e.to_string())?;
    }

    conn.execute(
        "UPDATE conversations SET project_id = ?1 WHERE project_id IS NULL",
        params![DEFAULT_PROJECT_ID],
    )
    .map_err(|e| e.to_string())?;

    // Now query conversations
    let mut where_clauses = vec!["c.deleted_at IS NULL".to_string()];
    let mut query_params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let include_archived = include_archived.unwrap_or(false);

    if !include_archived {
        where_clauses.push("c.archived_at IS NULL".to_string());
        where_clauses.push("(p.archived_at IS NULL OR p.id IS NULL)".to_string());
    }
    where_clauses.push("(p.deleted_at IS NULL OR p.id IS NULL)".to_string());

    if let Some(project_id) = project_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_params.push(Box::new(project_id.to_string()));
        where_clauses.push(format!(
            "COALESCE(c.project_id, 'default') = ?{}",
            query_params.len()
        ));
    }

    if let Some(q) = query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        query_params.push(Box::new(format!("%{}%", q)));
        where_clauses.push(format!("c.title LIKE ?{}", query_params.len()));
    }

    let sql = conversations_list_sql(&where_clauses.join(" AND "));

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
fn rename_conversation(
    db: tauri::State<'_, Database>,
    id: String,
    title: String,
) -> Result<(), String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Conversation title cannot be empty.".to_string());
    }

    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
        params![title, current_timestamp(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn move_conversation_to_project(
    db: tauri::State<'_, Database>,
    id: String,
    project_id: String,
) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let project_id = resolve_project_id(&conn, Some(project_id.as_str()))?;
    conn.execute(
        "UPDATE conversations SET project_id = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
        params![project_id, current_timestamp(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn archive_conversation(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let timestamp = current_timestamp();
    conn.execute(
        "UPDATE conversations SET archived_at = COALESCE(archived_at, ?1), updated_at = ?1
         WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn unarchive_conversation(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE conversations SET archived_at = NULL, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![current_timestamp(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn pin_conversation(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let timestamp = current_timestamp();
    conn.execute(
        "UPDATE conversations SET pinned_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn unpin_conversation(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE conversations SET pinned_at = NULL, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![current_timestamp(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn delete_conversation(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let timestamp = current_timestamp();
    conn.execute(
        "UPDATE conversations SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE generations SET deleted_at = COALESCE(deleted_at, ?1) WHERE conversation_id = ?2",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
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
        .query_map(params![conversation_id], gallery::row_to_generation)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let results = gallery::generation_results_with_images(&conn, generations)?;

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
    let _ = gallery::purge_trashed_generations(&app, db.inner(), retention_days);
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
            gallery::search_generations,
            gallery::delete_generation,
            gallery::restore_generation,
            gallery::permanently_delete_generation,
            create_project,
            get_projects,
            rename_project,
            archive_project,
            unarchive_project,
            pin_project,
            unpin_project,
            delete_project,
            create_conversation,
            get_conversations,
            rename_conversation,
            move_conversation_to_project,
            archive_conversation,
            unarchive_conversation,
            pin_conversation,
            unpin_conversation,
            delete_conversation,
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
            gallery::create_folder,
            gallery::rename_folder,
            gallery::delete_folder,
            gallery::get_folders,
            gallery::add_image_to_folders,
            gallery::remove_image_from_folders,
            gallery::get_image_folders,
            gallery::get_favorite_images,
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
                let _ = repair_mismatched_image_extensions(db.inner());
            }

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
                let _ =
                    gallery::purge_trashed_generations(&app.handle(), db.inner(), retention_days);
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

#[cfg(test)]
mod image_repair_tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb};
    use std::io::Cursor;

    fn test_db_path() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "astro-studio-image-repair-test-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir.join("astro_studio.db")
    }

    fn jpeg_bytes() -> Vec<u8> {
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(4, 4, Rgb([180, 72, 24])));
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Jpeg)
            .expect("encode jpeg");
        bytes
    }

    #[test]
    fn repairs_mismatched_saved_image_extension_and_database_path() {
        let db_path = test_db_path();
        let database = Database::open(&db_path).expect("open test db");
        database.run_migrations().expect("migrate test db");
        let image_path = db_path
            .parent()
            .expect("db parent")
            .join("bad-extension.png");
        std::fs::write(&image_path, jpeg_bytes()).expect("write mismatched image");

        {
            let conn = database.conn.lock().expect("lock db");
            conn.execute(
                "INSERT INTO generations (id, prompt, engine, size, quality, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    "generation-1",
                    "prompt",
                    ENGINE_NANO_BANANA_PRO,
                    "1024x1024",
                    "auto",
                    "completed",
                    "2026-04-29T05:57:11Z"
                ],
            )
            .expect("insert generation");
            conn.execute(
                "INSERT INTO images (id, generation_id, file_path, thumbnail_path, width, height, file_size, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    "image-1",
                    "generation-1",
                    image_path.to_string_lossy().to_string(),
                    "",
                    4,
                    4,
                    10,
                    "2026-04-29T05:57:11Z"
                ],
            )
            .expect("insert image");
        }

        repair_mismatched_image_extensions(&database).expect("repair images");

        let repaired_path: String = {
            let conn = database.conn.lock().expect("lock db");
            conn.query_row(
                "SELECT file_path FROM images WHERE id = ?1",
                params!["image-1"],
                |row| row.get(0),
            )
            .expect("select repaired path")
        };

        let repaired_data = std::fs::read(&repaired_path).expect("read repaired image");

        assert!(repaired_path.ends_with("bad-extension.png"));
        assert!(image_path.exists());
        assert_eq!(
            file_manager::detected_image_extension(&repaired_data),
            Some("png")
        );

        std::fs::remove_dir_all(db_path.parent().expect("db parent")).ok();
    }

    #[test]
    fn repairs_legacy_gemini_jpeg_originals_to_png() {
        let db_path = test_db_path();
        let database = Database::open(&db_path).expect("open test db");
        database.run_migrations().expect("migrate test db");
        let image_path = db_path
            .parent()
            .expect("db parent")
            .join("legacy-gemini.jpeg");
        std::fs::write(&image_path, jpeg_bytes()).expect("write legacy image");

        {
            let conn = database.conn.lock().expect("lock db");
            conn.execute(
                "INSERT INTO generations (id, prompt, engine, size, quality, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    "generation-legacy",
                    "prompt",
                    ENGINE_NANO_BANANA_PRO,
                    "1024x1024",
                    "auto",
                    "completed",
                    "2026-04-29T06:18:01Z"
                ],
            )
            .expect("insert generation");
            conn.execute(
                "INSERT INTO images (id, generation_id, file_path, thumbnail_path, width, height, file_size, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    "image-legacy",
                    "generation-legacy",
                    image_path.to_string_lossy().to_string(),
                    "",
                    4,
                    4,
                    10,
                    "2026-04-29T06:18:01Z"
                ],
            )
            .expect("insert image");
        }

        repair_mismatched_image_extensions(&database).expect("repair images");

        let repaired_path: String = {
            let conn = database.conn.lock().expect("lock db");
            conn.query_row(
                "SELECT file_path FROM images WHERE id = ?1",
                params!["image-legacy"],
                |row| row.get(0),
            )
            .expect("select repaired path")
        };
        let repaired_data = std::fs::read(&repaired_path).expect("read repaired image");

        assert!(repaired_path.ends_with("legacy-gemini.png"));
        assert!(!image_path.exists());
        assert_eq!(
            file_manager::detected_image_extension(&repaired_data),
            Some("png")
        );

        std::fs::remove_dir_all(db_path.parent().expect("db parent")).ok();
    }
}
