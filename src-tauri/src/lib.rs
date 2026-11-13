mod api_gateway;
mod commands;
mod config;
mod credentials;
mod db;
mod error;
mod file_manager;
mod gallery;
mod generation_execution;
// Durable worker persistence is retained for backwards-compatible job records,
// but its executor is intentionally not part of the compatibility runtime yet.
#[allow(dead_code)]
mod generation_jobs;
mod generation_lifecycle;
// Lease primitives belong to the deferred worker runtime and have no caller
// while compatibility execution remains the supported production path.
mod atomic_files;
mod batch_export;
mod canvas_assets;
mod canvas_close;
mod generation_recipe;
mod generation_recovery;
#[allow(dead_code)]
mod generation_worker_lease;
mod image_assets;
mod image_engines;
mod library_backup;
mod llm;
mod model_registry;
mod models;
mod operations;
mod prompt_agent;
mod provider_diagnostics;
mod runtime_logs;
mod storage;
mod updater;

use std::sync::Mutex;

use chrono::{SecondsFormat, Utc};
use db::Database;
use error::AppError;
use models::*;
use rusqlite::{params, Connection};
use tauri::Manager;

pub(crate) fn current_timestamp() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub(crate) fn format_log_clear_cutoff(now: chrono::DateTime<Utc>, days: u32) -> String {
    (now - chrono::Duration::days(days as i64)).to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub(crate) fn normalize_font_size(font_size: &str) -> &'static str {
    match font_size {
        "small" => "small",
        "large" => "large",
        _ => DEFAULT_FONT_SIZE,
    }
}

// ── Image extension repair ───────────────────────────────────────────────────

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

fn repair_mismatched_image_extensions(db: &Database) -> Result<usize, AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let rows: Vec<(String, String, String)> = {
        let mut stmt = conn
            .prepare(
                "SELECT i.id, i.file_path, g.engine
                 FROM images i
                 JOIN generations g ON g.id = i.generation_id",
            )
            .map_err(|e| AppError::Database {
                message: format!("Prepare repair query failed: {}", e),
            })?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .map_err(|e| AppError::Database {
                message: format!("Query repair rows failed: {}", e),
            })?
            .filter_map(|row| row.ok())
            .collect();
        rows
    };

    let mut repaired = 0;
    for (image_id, file_path, _engine) in rows {
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
        if normalized_path_extension(&path).as_deref() == Some(detected_extension) {
            continue;
        }

        let next_path = repaired_image_path(&path, detected_extension);
        std::fs::rename(&path, &next_path).map_err(|e| AppError::FileSystem {
            message: format!("Rename image for extension repair failed: {}", e),
        })?;
        let file_size = data.len() as i64;
        let next_path_str = next_path.to_string_lossy().to_string();
        if let Err(e) = conn.execute(
            "UPDATE images SET file_path = ?1, file_size = ?2 WHERE id = ?3",
            params![next_path_str, file_size, image_id],
        ) {
            let _ = std::fs::rename(&next_path, &path);
            return Err(AppError::Database {
                message: format!("Update repaired image path failed: {}", e),
            });
        }
        repaired += 1;
    }

    if repaired > 0 {
        log::info!("Repaired {} image file extension mismatch(es)", repaired);
    }

    Ok(repaired)
}

// ── Generation recovery ──────────────────────────────────────────────────────

const RECOVERY_STATE_RESPONSE_READY: &str = "response_ready";
const INTERRUPTED_GENERATION_MESSAGE: &str =
    "This task was interrupted because Astro Studio closed before the response was saved.";

struct PendingGenerationRecovery {
    generation_id: String,
    created_at: String,
    output_format: String,
    request_state: Option<String>,
    response_file: Option<String>,
}

fn cleanup_stale_legacy_generation_recoveries(conn: &Connection) -> Result<usize, AppError> {
    conn.execute(
        "DELETE FROM generation_recoveries
         WHERE generation_id IN (
             SELECT g.id
             FROM generations g
             WHERE g.status != 'processing'
               AND NOT (g.status = 'failed' AND generation_recoveries.request_state IN ('response_ready', 'requesting'))
               AND NOT EXISTS (
                   SELECT 1
                   FROM generation_jobs j
                   WHERE j.generation_id = g.id
               )
         )",
        [],
    )
    .map_err(|e| AppError::Database {
        message: format!("Clean stale recoveries failed: {}", e),
    })
}

fn load_pending_legacy_generation_recoveries(
    conn: &Connection,
) -> Result<Vec<PendingGenerationRecovery>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT g.id, g.created_at, COALESCE(r.output_format, ?1), r.request_state, r.response_file
             FROM generations g
             LEFT JOIN generation_recoveries r ON r.generation_id = g.id
             WHERE (g.status = 'processing' OR (g.status = 'failed' AND r.request_state = 'response_ready'))
               AND g.deleted_at IS NULL
               AND NOT EXISTS (
                   SELECT 1
                   FROM generation_jobs j
                   WHERE j.generation_id = g.id
               )",
        )
        .map_err(|e| AppError::Database {
            message: format!("Prepare recovery query failed: {}", e),
        })?;
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
        .map_err(|e| AppError::Database {
            message: format!("Query pending recoveries failed: {}", e),
        })?;

    let mut pending = Vec::new();
    for row in rows {
        match row {
            Ok(recovery) => pending.push(recovery),
            Err(_) => log::warn!("A malformed legacy recovery row was skipped; remaining rows will still be recovered."),
        }
    }
    Ok(pending)
}

fn set_generation_failed(
    conn: &Connection,
    generation_id: &str,
    error_message: &str,
    clear_recovery: bool,
) -> Result<(), AppError> {
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::Database {
            message: format!("Begin transaction failed: {}", e),
        })?;
    tx.execute(
        "UPDATE generations SET status = 'failed', error_message = ?1 WHERE id = ?2",
        params![error_message, generation_id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Update generation status failed: {}", e),
    })?;
    if clear_recovery {
        tx.execute(
            "DELETE FROM generation_recoveries WHERE generation_id = ?1",
            params![generation_id],
        )
        .map_err(|e| AppError::Database {
            message: format!("Clear recovery failed: {}", e),
        })?;
    }
    tx.commit().map_err(|e| AppError::Database {
        message: format!("Commit transaction failed: {}", e),
    })
}

fn save_generation_images_for_recovery(
    app: &tauri::AppHandle,
    db: &Database,
    generation_id: &str,
    created_at: &str,
    output_format: &str,
    images_data: &[Vec<u8>],
) -> Result<Vec<GeneratedImage>, AppError> {
    let app_data_dir = storage::asset_root(app).map_err(|e| AppError::FileSystem {
        message: format!("Get app data dir failed: {}", e),
    })?;
    save_generation_images_for_recovery_at(
        app_data_dir,
        db,
        generation_id,
        created_at,
        output_format,
        images_data,
    )
}

fn save_generation_images_for_recovery_at(
    app_data_dir: std::path::PathBuf,
    db: &Database,
    generation_id: &str,
    created_at: &str,
    output_format: &str,
    images_data: &[Vec<u8>],
) -> Result<Vec<GeneratedImage>, AppError> {
    let fm = file_manager::FileManager::new(app_data_dir);
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::Database {
            message: format!("Begin transaction failed: {}", e),
        })?;

    tx.execute(
        "DELETE FROM images WHERE generation_id = ?1",
        params![generation_id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Clear existing images failed: {}", e),
    })?;

    let mut saved_images = Vec::with_capacity(images_data.len());
    for (i, data) in images_data.iter().enumerate() {
        let img_id = format!("{}_{}", generation_id, i);
        let saved = fm.save_image_at(&img_id, data, output_format, Some(created_at))?;

        tx.execute(
            "INSERT INTO images (id, generation_id, file_path, thumbnail_path, width, height, file_size, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![img_id, generation_id, saved.file_path, saved.thumbnail_path, saved.width, saved.height, saved.file_size, created_at],
        )
        .map_err(|e| AppError::Database {
            message: format!("Insert image for recovery failed: {}", e),
        })?;

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
    .map_err(|e| AppError::Database {
        message: format!("Update generation status failed: {}", e),
    })?;
    tx.execute(
        "DELETE FROM generation_recoveries WHERE generation_id = ?1",
        params![generation_id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Clear recovery failed: {}", e),
    })?;
    tx.commit().map_err(|e| AppError::Database {
        message: format!("Commit transaction failed: {}", e),
    })?;

    Ok(saved_images)
}

async fn recover_legacy_payload(
    db: &Database,
    engine: &api_gateway::GptImageEngine,
    recovery: &PendingGenerationRecovery,
) -> Result<Vec<Vec<u8>>, AppError> {
    let result = async {
        let file = recovery
            .response_file
            .as_deref()
            .ok_or_else(|| AppError::Validation {
                message: "Saved response path is missing.".into(),
            })?;
        let body = std::fs::read_to_string(file).map_err(|_| AppError::Validation {
            message: "Saved response could not be read. Its recovery record was preserved.".into(),
        })?;
        engine
            .decode_images_from_response(&body, &|| false)
            .await
            .map_err(|_| AppError::Validation {
                message: "Saved response could not be decoded. Its recovery record was preserved."
                    .into(),
            })
    }
    .await;
    if let Err(error) = &result {
        let conn = db.conn.lock().map_err(|_| AppError::Validation {
            message: "Library unavailable.".into(),
        })?;
        set_generation_failed(&conn, &recovery.generation_id, &error.to_string(), false)?;
    }
    result
}

fn legacy_recovery_registry() -> &'static std::sync::Mutex<std::collections::HashSet<String>> {
    static REGISTRY: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
        std::sync::OnceLock::new();
    REGISTRY.get_or_init(Default::default)
}
struct LegacyRecoveryGuard(String);
impl Drop for LegacyRecoveryGuard {
    fn drop(&mut self) {
        if let Ok(mut active) = legacy_recovery_registry().lock() {
            active.remove(&self.0);
        }
    }
}

pub(crate) async fn recover_legacy_generation(
    app: &tauri::AppHandle,
    db: &Database,
    engine: &api_gateway::GptImageEngine,
    generation_id: &str,
) -> Result<(), AppError> {
    use tauri::Emitter;
    {
        let mut active = legacy_recovery_registry()
            .lock()
            .map_err(|_| AppError::Validation {
                message: "Recovery state unavailable.".into(),
            })?;
        if !active.insert(generation_id.to_owned()) {
            return Err(AppError::Validation {
                message: "This saved response is already being recovered.".into(),
            });
        }
    }
    let _guard = LegacyRecoveryGuard(generation_id.to_owned());
    let recovery = {
        let conn = db.conn.lock().map_err(|_| AppError::Validation {
            message: "Library unavailable.".into(),
        })?;
        load_pending_legacy_generation_recoveries(&conn)?
            .into_iter()
            .find(|r| r.generation_id == generation_id)
            .ok_or(AppError::GenerationJobNotRetryable)?
    };
    if recovery.request_state.as_deref() != Some(RECOVERY_STATE_RESPONSE_READY) {
        let conn = db.conn.lock().map_err(|_| AppError::Validation {
            message: "Library unavailable.".into(),
        })?;
        set_generation_failed(&conn, generation_id, INTERRUPTED_GENERATION_MESSAGE, false)?;
        return Ok(());
    }
    let images = recover_legacy_payload(db, engine, &recovery).await?;
    if images.is_empty() {
        return Err(AppError::Validation {
            message: "Saved response contained no image.".into(),
        });
    }
    match save_generation_images_for_recovery(
        app,
        db,
        generation_id,
        &recovery.created_at,
        &recovery.output_format,
        &images,
    ) {
        Ok(_) => {
            let _ = app.emit(
                "generation:complete",
                serde_json::json!({"generation_id":generation_id,"status":"completed"}),
            );
            Ok(())
        }
        Err(error) => {
            let conn = db.conn.lock().map_err(|_| AppError::Validation {
                message: "Library unavailable.".into(),
            })?;
            set_generation_failed(
                &conn,
                generation_id,
                "Recovered image could not be saved. Its saved response was preserved.",
                false,
            )?;
            Err(error)
        }
    }
}

async fn recover_interrupted_generations(
    app: &tauri::AppHandle,
    db: &Database,
    engine: &api_gateway::GptImageEngine,
) -> Result<(), AppError> {
    let pending = {
        let conn = db.conn.lock().map_err(|_| AppError::Validation {
            message: "Library unavailable.".into(),
        })?;
        cleanup_stale_legacy_generation_recoveries(&conn)?;
        load_pending_legacy_generation_recoveries(&conn)?
    };
    for recovery in pending {
        if recover_legacy_generation(app, db, engine, &recovery.generation_id)
            .await
            .is_err()
        {
            log::warn!(
                "A legacy saved response could not be recovered; retained for manual recovery."
            );
        }
    }
    Ok(())
}

// ── macOS dev icon ───────────────────────────────────────────────────────────

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

// ── App entry point ──────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_config = config::AppConfig::load();
    config::init_logger(&app_config.log);

    let engine =
        api_gateway::GptImageEngine::new(&app_config.api).expect("Failed to build image engine");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(canvas_close::CanvasCloseState::default())
        .manage(app_config)
        .manage(engine)
        .manage(operations::Operations::default())
        .manage(batch_export::ExportSessions::default())
        .manage(library_backup::BackupInspections::default())
        .manage(commands::generation::SelectedImageRegistry::default())
        .manage(updater::PendingUpdate(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![
            generation_recovery::get_generation_recovery_summary,
            generation_recovery::recover_generation,
            library_backup::create_library_backup,
            library_backup::inspect_library_backup,
            library_backup::restore_library_backup,
            library_backup::rollback_library_restore,
            library_backup::can_rollback_library_restore,
            operations::cancel_operation,
            batch_export::export_generations,
            generation_recipe::get_generation_recipe,
            provider_diagnostics::diagnose_image_provider,
            provider_diagnostics::get_provider_smoke_plan,
            provider_diagnostics::run_provider_smoke_test,
            credentials::get_credential_status,
            commands::settings::save_api_key,
            commands::settings::get_api_key,
            commands::settings::get_model_api_key,
            commands::settings::save_model_api_key,
            commands::settings::save_base_url,
            commands::settings::get_base_url,
            commands::settings::get_endpoint_settings,
            commands::settings::save_endpoint_settings,
            commands::settings::get_model_endpoint_settings,
            commands::settings::save_model_endpoint_settings,
            commands::settings::get_model_provider_profiles,
            commands::settings::save_model_provider_profiles,
            commands::settings::create_model_provider_profile,
            commands::settings::delete_model_provider_profile,
            commands::settings::set_active_model_provider,
            commands::settings::get_font_size,
            commands::settings::save_font_size,
            commands::settings::get_image_model,
            commands::settings::save_image_model,
            commands::generation::generate_image,
            commands::generation::edit_image,
            commands::prompt_agent::get_prompt_agent_health,
            commands::prompt_agent::start_prompt_agent_session,
            commands::prompt_agent::send_prompt_agent_message,
            commands::prompt_agent::accept_prompt_agent_draft,
            commands::prompt_agent::cancel_prompt_agent_session,
            commands::prompt_agent::get_prompt_agent_session,
            gallery::search_generations,
            gallery::delete_generation,
            gallery::restore_generation,
            gallery::permanently_delete_generation,
            commands::projects::create_project,
            commands::projects::get_projects,
            commands::projects::rename_project,
            commands::projects::archive_project,
            commands::projects::unarchive_project,
            commands::projects::pin_project,
            commands::projects::unpin_project,
            commands::projects::delete_project,
            canvas_close::enable_canvas_close_guard,
            canvas_close::finish_canvas_close,
            canvas_close::cancel_canvas_close,
            commands::canvas::create_canvas_document,
            commands::canvas::list_canvas_documents,
            commands::canvas::get_canvas_document,
            commands::canvas::save_canvas_document,
            commands::canvas::import_canvas_asset,
            commands::canvas::rename_canvas_document,
            commands::canvas::delete_canvas_document,
            commands::canvas::save_canvas_export,
            commands::canvas::get_canvas_generation_status,
            commands::conversations::create_conversation,
            commands::conversations::get_conversations,
            commands::conversations::rename_conversation,
            commands::conversations::move_conversation_to_project,
            commands::conversations::archive_conversation,
            commands::conversations::unarchive_conversation,
            commands::conversations::pin_conversation,
            commands::conversations::unpin_conversation,
            commands::conversations::delete_conversation,
            commands::conversations::get_conversation_generations,
            commands::generation::copy_image_to_clipboard,
            commands::generation::save_image_to_file,
            commands::generation::pick_source_images,
            commands::prompts::create_prompt_favorite,
            commands::prompts::get_prompt_favorites,
            commands::prompts::delete_prompt_favorite,
            commands::prompts::create_prompt_folder,
            commands::prompts::rename_prompt_folder,
            commands::prompts::delete_prompt_folder,
            commands::prompts::get_prompt_folders,
            commands::prompts::add_prompt_favorite_to_folders,
            commands::prompts::remove_prompt_favorite_from_folders,
            commands::prompts::get_prompt_favorite_folders,
            gallery::create_folder,
            gallery::rename_folder,
            gallery::delete_folder,
            gallery::get_folders,
            gallery::add_image_to_folders,
            gallery::remove_image_from_folders,
            gallery::get_image_folders,
            gallery::get_favorite_images,
            commands::logs::get_logs,
            commands::logs::get_runtime_logs,
            commands::logs::get_log_detail,
            commands::logs::read_log_response_file,
            commands::logs::clear_logs,
            commands::logs::get_log_settings,
            commands::logs::save_log_settings,
            commands::logs::get_trash_settings,
            commands::logs::save_trash_settings,
            commands::llm::get_llm_configs,
            commands::llm::save_llm_configs,
            commands::llm::optimize_prompt,
            commands::llm::get_prompt_extractions,
            commands::llm::extract_prompt_from_image,
            updater::is_update_supported,
            updater::check_update,
            updater::install_update,
            updater::relaunch_app,
        ])
        .setup(|app| {
            let test_root = storage::development_library_root();
            let bootstrap = match &test_root {
                Some(root) => root.join("app-data"),
                None => app.path().app_data_dir()?,
            };
            let library_lock =
                storage::LibraryLock::acquire(&bootstrap).map_err(std::io::Error::other)?;
            let legacy_db = match &test_root {
                Some(root) => root.join("library.db"),
                None => dirs::data_dir()
                    .ok_or("Cannot determine data directory")?
                    .join("astro-studio")
                    .join("astro_studio.db"),
            };
            let mut paths =
                storage::StoragePaths::new(bootstrap.clone(), legacy_db.clone(), bootstrap.clone())
                    .map_err(std::io::Error::other)?;
            let open_library = |paths: &storage::StoragePaths| -> Result<Database, AppError> {
                if paths.previous.is_some() && !paths.database.is_file() {
                    return Err(AppError::Validation {
                        message: "Restored database is missing".into(),
                    });
                }
                let database = Database::open(&paths.database)?;
                database.run_migrations()?;
                let check: String = database
                    .conn
                    .lock()
                    .map_err(|_| AppError::Validation {
                        message: "Library lock unavailable".into(),
                    })?
                    .query_row("PRAGMA quick_check", [], |row| row.get(0))?;
                if check != "ok" {
                    return Err(AppError::Validation {
                        message: "Library integrity check failed".into(),
                    });
                }
                Ok(database)
            };
            let database = match open_library(&paths) {
                Ok(db) => db,
                Err(error) if paths.previous.is_some() => {
                    log::warn!(
                        "Restored library could not open; returning to original library: {error}"
                    );
                    paths.rollback().map_err(std::io::Error::other)?;
                    paths = storage::StoragePaths::new(bootstrap.clone(), legacy_db, bootstrap)
                        .map_err(std::io::Error::other)?;
                    open_library(&paths)?
                }
                Err(error) => return Err(Box::new(error)),
            };
            app.manage(library_lock);
            app.manage(paths.clone());
            app.manage(storage::global_gate().clone());
            if database.migrate_credentials().is_err() {
                log::warn!("Credential migration is pending; credential-backed operations are unavailable until the system vault is accessible.");
            }
            app.manage(database);
            runtime_logs::attach_app_handle(app.handle().clone());
            app.state::<api_gateway::GptImageEngine>()
                .attach_app(app.handle().clone());

            let app_data_dir = paths.assets.clone();
            let fm = file_manager::FileManager::new(app_data_dir);
            fm.ensure_dirs()?;

            {
                let db = app.state::<Database>();
                let _ = repair_mismatched_image_extensions(db.inner());
            }

            {
                let db = app.state::<Database>();
                let engine = app.state::<api_gateway::GptImageEngine>();
                let _maintenance = storage::global_gate()
                    .maintain()
                    .map_err(std::io::Error::other)?;
                tauri::async_runtime::block_on(generation_recovery::startup_recover(
                    app.handle(),
                    db.inner(),
                    engine.inner(),
                    &paths.assets,
                ))?;
                tauri::async_runtime::block_on(recover_interrupted_generations(
                    app.handle(),
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
                    gallery::purge_trashed_generations(app.handle(), db.inner(), retention_days);
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
        .run(|app, _event| {
            match &_event {
                tauri::RunEvent::WindowEvent {label,event:tauri::WindowEvent::CloseRequested{api,..},..} if canvas_close::guarded(app) => {
                    api.prevent_close();
                    if let Err(error)=canvas_close::request(app,"close",Some(label.clone())) {log::error!("Canvas close guard: {error}");}
                }
                tauri::RunEvent::ExitRequested {api,code,..} if canvas_close::guarded(app) && *code!=Some(tauri::RESTART_EXIT_CODE) => {
                    api.prevent_exit();
                    if let Err(error)=canvas_close::request(app,"exit",None) {log::error!("Canvas exit guard: {error}");}
                }
                _=>{}
            }

            #[cfg(all(debug_assertions, target_os = "macos"))]
            if matches!(_event, tauri::RunEvent::Ready) {
                set_macos_development_app_icon();
            }
        });
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn log_clear_cutoff_uses_database_timestamp_format() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-04-28T12:30:45Z")
            .unwrap()
            .with_timezone(&Utc);

        assert_eq!(format_log_clear_cutoff(now, 0), "2026-04-28T12:30:45Z");
        assert_eq!(format_log_clear_cutoff(now, 7), "2026-04-21T12:30:45Z");
    }
}

#[cfg(test)]
mod legacy_recovery_guard_tests {
    use super::*;

    fn temp_test_db(prefix: &str) -> (Database, std::path::PathBuf) {
        let db_path =
            std::env::temp_dir().join(format!("{prefix}-{}.sqlite", uuid::Uuid::new_v4()));
        let db = Database::open(&db_path).expect("open test database");
        db.run_migrations().expect("run real database migrations");
        (db, db_path)
    }

    fn remove_temp_test_db(db_path: std::path::PathBuf) {
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-wal"));
        let _ = std::fs::remove_file(db_path.with_extension("sqlite-shm"));
    }

    fn insert_generation(conn: &Connection, generation_id: &str, status: &str) {
        conn.execute(
            "INSERT INTO generations (id, prompt, status, created_at)
             VALUES (?1, 'legacy recovery fixture', ?2, '2026-07-10T00:00:00Z')",
            params![generation_id, status],
        )
        .expect("insert generation fixture");
    }

    fn insert_recovery(conn: &Connection, generation_id: &str) {
        conn.execute(
            "INSERT INTO generation_recoveries (
                generation_id, request_kind, request_state, output_format, response_file,
                created_at, updated_at
             ) VALUES (
                ?1, 'generate', 'requesting', 'png', NULL,
                '2026-07-10T00:00:00Z', '2026-07-10T00:00:00Z'
             )",
            params![generation_id],
        )
        .expect("insert recovery fixture");
    }

    fn insert_job(conn: &Connection, job_id: &str, generation_id: &str) {
        conn.execute(
            "INSERT INTO generation_jobs (
                id, client_request_id, generation_id, source_kind, status, request_json,
                provider_kind, provider_profile_id, endpoint_snapshot, queued_at
             ) VALUES (
                ?1, ?2, ?3, 'generate', 'queued', '{}', 'openai', 'provider-a',
                'https://api.example.test/v1/images/generations', '2026-07-10T00:00:00Z'
             )",
            params![job_id, format!("request-{job_id}"), generation_id],
        )
        .expect("insert generation job fixture");
    }

    fn insert_running_job_with_response_ready_recovery(
        conn: &Connection,
        job_id: &str,
        generation_id: &str,
    ) {
        conn.execute(
            "INSERT INTO generation_recoveries (
                generation_id, request_kind, request_state, output_format, response_file,
                created_at, updated_at
             ) VALUES (
                ?1, 'generate', 'response_ready', 'png', '/tmp/verified-response.json',
                '2026-07-10T00:00:00Z', '2026-07-10T00:00:01Z'
             )",
            params![generation_id],
        )
        .expect("insert response-ready recovery fixture");
        conn.execute(
            "INSERT INTO generation_jobs (
                id, client_request_id, generation_id, source_kind, status, request_json,
                provider_kind, provider_profile_id, endpoint_snapshot, queued_at, started_at,
                last_heartbeat_at
             ) VALUES (
                ?1, ?2, ?3, 'generate', 'running', '{}', 'openai', 'provider-a',
                'https://api.example.test/v1/images/generations',
                '2026-07-10T00:00:00Z', '2026-07-10T00:00:01Z',
                '2026-07-10T00:00:01Z'
             )",
            params![job_id, format!("request-{job_id}"), generation_id],
        )
        .expect("insert running generation job fixture");
    }

    fn recovery_exists(conn: &Connection, generation_id: &str) -> bool {
        conn.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM generation_recoveries WHERE generation_id = ?1
             )",
            params![generation_id],
            |row| row.get::<_, bool>(0),
        )
        .expect("query recovery fixture")
    }

    #[test]
    fn legacy_recovery_helpers_ignore_processing_and_nonprocessing_job_generations() {
        let (db, db_path) = temp_test_db("astro-studio-legacy-recovery-job-guard-test");
        {
            let conn = db.conn.lock().expect("lock test database");
            insert_generation(&conn, "job-processing", "processing");
            insert_recovery(&conn, "job-processing");
            insert_job(&conn, "job-processing-id", "job-processing");

            insert_generation(&conn, "job-completed", "completed");
            insert_recovery(&conn, "job-completed");
            insert_job(&conn, "job-completed-id", "job-completed");

            assert_eq!(
                cleanup_stale_legacy_generation_recoveries(&conn).unwrap(),
                0
            );
            assert!(load_pending_legacy_generation_recoveries(&conn)
                .unwrap()
                .is_empty());
            assert!(recovery_exists(&conn, "job-processing"));
            assert!(recovery_exists(&conn, "job-completed"));
        }

        drop(db);
        remove_temp_test_db(db_path);
    }

    #[test]
    fn legacy_recovery_helpers_ignore_running_job_response_ready_recovery() {
        let (db, db_path) = temp_test_db("astro-studio-legacy-response-ready-job-guard-test");
        {
            let conn = db.conn.lock().expect("lock test database");
            insert_generation(&conn, "job-response-ready", "processing");
            insert_running_job_with_response_ready_recovery(
                &conn,
                "job-response-ready-id",
                "job-response-ready",
            );

            assert!(load_pending_legacy_generation_recoveries(&conn)
                .unwrap()
                .is_empty());
            conn.execute(
                "UPDATE generations SET status = 'running' WHERE id = ?1",
                params!["job-response-ready"],
            )
            .expect("move linked generation outside legacy processing status");
            assert_eq!(
                cleanup_stale_legacy_generation_recoveries(&conn).unwrap(),
                0
            );
            assert!(recovery_exists(&conn, "job-response-ready"));
        }

        drop(db);
        remove_temp_test_db(db_path);
    }

    #[test]
    fn pending_legacy_recovery_scan_includes_processing_generation_without_job() {
        let (db, db_path) = temp_test_db("astro-studio-legacy-recovery-pending-test");
        {
            let conn = db.conn.lock().expect("lock test database");
            insert_generation(&conn, "legacy-processing", "processing");

            let pending = load_pending_legacy_generation_recoveries(&conn).unwrap();

            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].generation_id, "legacy-processing");
            assert!(pending[0].request_state.is_none());
        }

        drop(db);
        remove_temp_test_db(db_path);
    }

    #[tokio::test]
    async fn legacy_local_failures_preserve_payload_and_success_is_not_queued_twice() {
        let (db, path) = temp_test_db("legacy-retry");
        let root = std::env::temp_dir().join(format!("legacy-assets-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let response = root.join("response.json");
        {
            let conn = db.conn.lock().unwrap();
            insert_generation(&conn, "legacy-retry", "processing");
            insert_recovery(&conn, "legacy-retry");
            conn.execute("UPDATE generation_recoveries SET request_state='response_ready',response_file=?1 WHERE generation_id='legacy-retry'",[response.to_string_lossy().as_ref()]).unwrap();
        }
        let engine = api_gateway::GptImageEngine::new(&config::ApiConfig {
            timeout_secs: 1,
            max_retries: 0,
        })
        .unwrap();
        let pending = {
            let conn = db.conn.lock().unwrap();
            load_pending_legacy_generation_recoveries(&conn)
                .unwrap()
                .remove(0)
        };
        assert!(recover_legacy_payload(&db, &engine, &pending)
            .await
            .is_err());
        std::fs::write(&response, "not valid JSON").unwrap();
        assert!(recover_legacy_payload(&db, &engine, &pending)
            .await
            .is_err());
        {
            let conn = db.conn.lock().unwrap();
            assert!(recovery_exists(&conn, "legacy-retry"));
            assert_eq!(
                cleanup_stale_legacy_generation_recoveries(&conn).unwrap(),
                0
            );
        }
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(1, 1)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            bytes.into_inner(),
        );
        std::fs::write(
            &response,
            serde_json::json!({"data":[{"b64_json":encoded}]}).to_string(),
        )
        .unwrap();
        let images = recover_legacy_payload(&db, &engine, &pending)
            .await
            .unwrap();
        assert_eq!(images.len(), 1);
        save_generation_images_for_recovery_at(
            root.clone(),
            &db,
            &pending.generation_id,
            &pending.created_at,
            &pending.output_format,
            &images,
        )
        .unwrap();
        {
            let conn = db.conn.lock().unwrap();
            assert!(load_pending_legacy_generation_recoveries(&conn)
                .unwrap()
                .is_empty());
            assert_eq!(
                conn.query_row(
                    "SELECT COUNT(*) FROM images WHERE generation_id='legacy-retry'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
                1
            );
        }
        drop(db);
        remove_temp_test_db(path);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_legacy_row_does_not_block_other_saved_responses() {
        let (db, path) = temp_test_db("legacy-corrupt-row");
        {
            let conn = db.conn.lock().unwrap();
            insert_generation(&conn, "bad", "processing");
            insert_generation(&conn, "good", "processing");
            conn.execute("UPDATE generations SET created_at=x'ff' WHERE id='bad'", [])
                .unwrap();
            let pending = load_pending_legacy_generation_recoveries(&conn).unwrap();
            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].generation_id, "good");
        }
        drop(db);
        remove_temp_test_db(path);
    }

    #[test]
    fn failed_legacy_saved_response_survives_cleanup_and_remains_recoverable() {
        let (db, path) = temp_test_db("legacy-preserve");
        {
            let conn = db.conn.lock().unwrap();
            insert_generation(&conn, "legacy-failed", "failed");
            insert_recovery(&conn, "legacy-failed");
            conn.execute("UPDATE generation_recoveries SET request_state='response_ready',response_file='/missing/response.json' WHERE generation_id='legacy-failed'",[]).unwrap();
            assert_eq!(
                cleanup_stale_legacy_generation_recoveries(&conn).unwrap(),
                0
            );
            let pending = load_pending_legacy_generation_recoveries(&conn).unwrap();
            assert_eq!(pending.len(), 1);
            assert_eq!(pending[0].generation_id, "legacy-failed");
        }
        drop(db);
        remove_temp_test_db(path);
    }

    #[test]
    fn legacy_unknown_request_survives_cleanup_without_becoming_recoverable() {
        let (db, path) = temp_test_db("legacy-unknown");
        {
            let conn = db.conn.lock().unwrap();
            insert_generation(&conn, "legacy-unknown", "processing");
            insert_recovery(&conn, "legacy-unknown");
            set_generation_failed(
                &conn,
                "legacy-unknown",
                INTERRUPTED_GENERATION_MESSAGE,
                false,
            )
            .unwrap();
            assert_eq!(
                cleanup_stale_legacy_generation_recoveries(&conn).unwrap(),
                0
            );
            assert!(recovery_exists(&conn, "legacy-unknown"));
            assert!(load_pending_legacy_generation_recoveries(&conn)
                .unwrap()
                .is_empty());
        }
        drop(db);
        remove_temp_test_db(path);
    }

    #[test]
    fn stale_legacy_recovery_cleanup_deletes_nonprocessing_generation_without_job() {
        let (db, db_path) = temp_test_db("astro-studio-legacy-recovery-cleanup-test");
        {
            let conn = db.conn.lock().expect("lock test database");
            insert_generation(&conn, "legacy-completed", "completed");
            insert_recovery(&conn, "legacy-completed");

            assert_eq!(
                cleanup_stale_legacy_generation_recoveries(&conn).unwrap(),
                1
            );
            assert!(!recovery_exists(&conn, "legacy-completed"));
        }

        drop(db);
        remove_temp_test_db(db_path);
    }
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
        let source = jpeg_bytes();
        std::fs::write(&image_path, &source).expect("write mismatched image");

        {
            let conn = database.conn.lock().expect("lock db");
            conn.execute(
                "INSERT INTO generations (id, prompt, engine, size, quality, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    "generation-1", "prompt", ENGINE_NANO_BANANA_PRO,
                    "1024x1024", "auto", "completed", "2026-04-29T05:57:11Z"
                ],
            )
            .expect("insert generation");
            conn.execute(
                "INSERT INTO images (id, generation_id, file_path, thumbnail_path, width, height, file_size, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    "image-1", "generation-1",
                    image_path.to_string_lossy().to_string(), "", 4, 4, 10, "2026-04-29T05:57:11Z"
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

        assert!(repaired_path.ends_with("bad-extension.jpeg"));
        assert!(!image_path.exists());
        assert_eq!(repaired_data, source);

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
        let source = jpeg_bytes();
        std::fs::write(&image_path, &source).expect("write legacy image");

        {
            let conn = database.conn.lock().expect("lock db");
            conn.execute(
                "INSERT INTO generations (id, prompt, engine, size, quality, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    "generation-legacy", "prompt", ENGINE_NANO_BANANA_PRO,
                    "1024x1024", "auto", "completed", "2026-04-29T06:18:01Z"
                ],
            )
            .expect("insert generation");
            conn.execute(
                "INSERT INTO images (id, generation_id, file_path, thumbnail_path, width, height, file_size, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    "image-legacy", "generation-legacy",
                    image_path.to_string_lossy().to_string(), "", 4, 4, 10, "2026-04-29T06:18:01Z"
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

        assert!(repaired_path.ends_with("legacy-gemini.jpeg"));
        assert!(image_path.exists());
        assert_eq!(repaired_data, source);

        std::fs::remove_dir_all(db_path.parent().expect("db parent")).ok();
    }
}
