mod api_gateway;
mod commands;
mod config;
mod db;
mod error;
mod file_manager;
mod gallery;
mod image_engines;
mod llm;
mod model_registry;
mod models;
mod runtime_logs;

use chrono::{SecondsFormat, Utc};
use db::Database;
use error::AppError;
use model_registry::is_gemini_model;
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
                    file_path, e
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

        let img = image::load_from_memory(&data)
            .map_err(|e| AppError::FileSystem {
                message: format!("Decode image for repair failed: {}", e),
            })?;
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

fn set_generation_failed(
    conn: &Connection,
    generation_id: &str,
    error_message: &str,
    clear_recovery: bool,
) -> Result<(), AppError> {
    let tx = conn.unchecked_transaction().map_err(|e| AppError::Database {
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
    let app_data_dir = app.path().app_data_dir().map_err(|e| AppError::FileSystem {
        message: format!("Get app data dir failed: {}", e),
    })?;
    let fm = file_manager::FileManager::new(app_data_dir);
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let tx = conn.unchecked_transaction().map_err(|e| AppError::Database {
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

async fn recover_interrupted_generations(
    app: &tauri::AppHandle,
    db: &Database,
    engine: &api_gateway::GptImageEngine,
) -> Result<(), AppError> {
    {
        let conn = db.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        conn.execute(
            "DELETE FROM generation_recoveries WHERE generation_id IN (SELECT id FROM generations WHERE status != 'processing')",
            [],
        )
        .map_err(|e| AppError::Database {
            message: format!("Clean stale recoveries failed: {}", e),
        })?;
    }

    let pending = {
        let conn = db.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        let mut stmt = conn
            .prepare(
                "SELECT g.id, g.created_at, COALESCE(r.output_format, ?1), r.request_state, r.response_file
                 FROM generations g
                 LEFT JOIN generation_recoveries r ON r.generation_id = g.id
                 WHERE g.status = 'processing'",
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
        rows.filter_map(|row| row.ok()).collect::<Vec<_>>()
    };

    for recovery in pending {
        let Some(request_state) = recovery.request_state.as_deref() else {
            let conn = db.conn.lock().map_err(|e| AppError::Database {
                message: format!("Lock failed: {}", e),
            })?;
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
                let conn = db.conn.lock().map_err(|e| AppError::Database {
                    message: format!("Lock failed: {}", e),
                })?;
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
                    let conn = db.conn.lock().map_err(|e| AppError::Database {
                        message: format!("Lock failed: {}", e),
                    })?;
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
                    let conn = db.conn.lock().map_err(|e| AppError::Database {
                        message: format!("Lock failed: {}", e),
                    })?;
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

            match save_generation_images_for_recovery(
                app, db, &recovery.generation_id, &recovery.created_at,
                &recovery.output_format, &decoded_images,
            ) {
                Ok(saved_images) => {
                    log::info!(
                        "[{}] Recovered {} image(s) from saved API response",
                        recovery.generation_id,
                        saved_images.len()
                    );
                    let _ = db.insert_log(
                        "generation", "info",
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
                        recovery.generation_id, error
                    );
                }
            }

            continue;
        }

        let conn = db.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        set_generation_failed(
            &conn,
            &recovery.generation_id,
            INTERRUPTED_GENERATION_MESSAGE,
            true,
        )?;
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

        assert!(repaired_path.ends_with("legacy-gemini.png"));
        assert!(!image_path.exists());
        assert_eq!(
            file_manager::detected_image_extension(&repaired_data),
            Some("png")
        );

        std::fs::remove_dir_all(db_path.parent().expect("db parent")).ok();
    }
}
