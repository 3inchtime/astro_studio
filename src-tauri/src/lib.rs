mod api_gateway;
mod config;
mod db;
mod file_manager;
mod models;

use api_gateway::ImageEngine;
use db::Database;
use models::*;
use rusqlite::params;
use tauri::{Emitter, Manager};

// --- Settings commands ---

#[tauri::command]
fn save_api_key(db: tauri::State<'_, Database>, key: String) -> Result<(), String> {
    log::info!("Saving API key");
    db.set_setting(SETTING_API_KEY, &key)
}

#[tauri::command]
fn get_api_key(db: tauri::State<'_, Database>) -> Result<Option<String>, String> {
    db.get_setting(SETTING_API_KEY)
}

#[tauri::command]
fn save_base_url(db: tauri::State<'_, Database>, url: String) -> Result<(), String> {
    log::info!("Saving base URL: {}", url);
    db.set_setting(SETTING_BASE_URL, &url)
}

#[tauri::command]
fn get_base_url(db: tauri::State<'_, Database>) -> Result<String, String> {
    Ok(db.get_setting(SETTING_BASE_URL)?.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()))
}

// --- Generation commands ---

#[tauri::command]
async fn generate_image(
    app: tauri::AppHandle,
    db: tauri::State<'_, Database>,
    engine_state: tauri::State<'_, api_gateway::GptImageEngine>,
    prompt: String,
    size: Option<String>,
    quality: Option<String>,
) -> Result<GenerateResult, String> {
    let size = size.unwrap_or_else(|| "1024x1024".to_string());
    let quality = quality.unwrap_or_else(|| "high".to_string());
    let generation_id = uuid::Uuid::new_v4().to_string();

    let conversation_id = {
        let conn = db.conn.lock().map_err(|e| e.to_string())?;
        let recent: Option<(String, String)> = conn.query_row(
            "SELECT conversation_id, created_at FROM generations \
             WHERE conversation_id IS NOT NULL \
             ORDER BY created_at DESC LIMIT 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        ).ok();

        match recent {
            Some((conv_id, created_at_str)) => {
                let recent_time = chrono::NaiveDateTime::parse_from_str(&created_at_str, "%Y-%m-%d %H:%M:%S")
                    .unwrap_or_else(|_| chrono::Local::now().naive_local());
                let now = chrono::Local::now().naive_local();
                let diff = now - recent_time;
                if diff.num_minutes() <= 30 {
                    conn.execute(
                        "UPDATE conversations SET updated_at = datetime('now') WHERE id = ?1",
                        params![conv_id],
                    ).ok();
                    conv_id
                } else {
                    create_new_conversation(&conn, &prompt)?
                }
            }
            None => create_new_conversation(&conn, &prompt)?,
        }
    };

    log::info!("[{}] Generating image — prompt: {:?}, size: {}, quality: {}", generation_id, prompt, size, quality);

    let api_key = db.get_setting(SETTING_API_KEY)?
        .ok_or_else(|| "API key not set. Please set it in Settings.".to_string())?;
    let base_url = db.get_setting(SETTING_BASE_URL)?
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let fm = file_manager::FileManager::new(app_data_dir);

    let _ = app.emit("generation:progress", serde_json::json!({
        "generation_id": generation_id,
        "status": "processing"
    }));

    let result = engine_state.generate(&api_key, &base_url, &prompt, &size, &quality).await;

    match result {
        Ok(images_data) => {
            log::info!("[{}] API returned {} image(s), saving to disk...", generation_id, images_data.len());
            let mut image_paths = Vec::new();

            let conn = db.conn.lock().map_err(|e| e.to_string())?;
            let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

            tx.execute(
                "INSERT INTO generations (id, prompt, engine, size, quality, status, conversation_id) VALUES (?1, ?2, ?3, ?4, ?5, 'processing', ?6)",
                params![generation_id, prompt, ENGINE_GPT_IMAGE_2, size, quality, conversation_id],
            ).map_err(|e| e.to_string())?;

            for (i, data) in images_data.iter().enumerate() {
                let img_id = format!("{}_{}", generation_id, i);
                let saved = fm.save_image(&img_id, data)?;
                image_paths.push(saved.file_path.clone());

                tx.execute(
                    "INSERT INTO images (id, generation_id, file_path, thumbnail_path, width, height, file_size) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![img_id, generation_id, saved.file_path, saved.thumbnail_path, saved.width, saved.height, saved.file_size],
                ).map_err(|e| e.to_string())?;
            }

            tx.execute(
                "UPDATE generations SET status = 'completed' WHERE id = ?1",
                params![generation_id],
            ).map_err(|e| e.to_string())?;

            tx.commit().map_err(|e| e.to_string())?;

            log::info!("[{}] Generation completed — {} image(s) saved", generation_id, image_paths.len());

            let _ = app.emit("generation:complete", serde_json::json!({
                "generation_id": generation_id,
                "status": "completed"
            }));

            Ok(GenerateResult { generation_id, conversation_id, image_paths })
        }
        Err(e) => {
            log::error!("[{}] Generation failed: {}", generation_id, e);

            let conn = db.conn.lock().map_err(|e| e.to_string())?;
            conn.execute(
                "UPDATE generations SET status = 'failed' WHERE id = ?1",
                params![generation_id],
            )
            .map_err(|e| e.to_string())?;

            let _ = app.emit("generation:failed", serde_json::json!({
                "generation_id": generation_id,
                "error": &e
            }));

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
        created_at: row.get("created_at")?,
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

#[tauri::command]
fn search_generations(
    db: tauri::State<'_, Database>,
    query: Option<String>,
    page: Option<i32>,
) -> Result<SearchResult, String> {
    let page = page.unwrap_or(1);
    let page_size = 20;
    let offset = (page - 1) * page_size;

    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let (count, generations) = if let Some(q) = &query {
        if !q.is_empty() {
            let pattern = format!("%{}%", q);
            let count: i32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM generations WHERE prompt LIKE ?1",
                    params![pattern],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;

            let mut stmt = conn
                .prepare("SELECT * FROM generations WHERE prompt LIKE ?1 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3")
                .map_err(|e| e.to_string())?;
            let gens = stmt
                .query_map(params![pattern, page_size, offset], row_to_generation)
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect::<Vec<_>>();

            (count, gens)
        } else {
            fetch_all_generations(&conn, page_size, offset)?
        }
    } else {
        fetch_all_generations(&conn, page_size, offset)?
    };

    let gen_ids: Vec<&str> = generations.iter().map(|g| g.id.as_str()).collect();
    let mut image_map: std::collections::HashMap<String, Vec<GeneratedImage>> =
        std::collections::HashMap::new();

    if !gen_ids.is_empty() {
        let placeholders: Vec<&str> = gen_ids.iter().map(|_| "?").collect();
        let sql = format!(
            "SELECT * FROM images WHERE generation_id IN ({})",
            placeholders.join(",")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = gen_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let images = stmt
            .query_map(params.as_slice(), row_to_image)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok());

        for img in images {
            image_map.entry(img.generation_id.clone())
                .or_default()
                .push(img);
        }
    }

    let results = generations
        .into_iter()
        .map(|gen| {
            let images = image_map.remove(&gen.id).unwrap_or_default();
            GenerationResult { generation: gen, images }
        })
        .collect();

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
) -> Result<(i32, Vec<Generation>), String> {
    let count: i32 = conn
        .query_row("SELECT COUNT(*) FROM generations", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT * FROM generations ORDER BY created_at DESC LIMIT ?1 OFFSET ?2")
        .map_err(|e| e.to_string())?;
    let gens = stmt
        .query_map(params![limit, offset], row_to_generation)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok((count, gens))
}

#[tauri::command]
fn delete_generation(
    app: tauri::AppHandle,
    db: tauri::State<'_, Database>,
    id: String,
) -> Result<(), String> {
    log::info!("Deleting generation {}", id);
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare("SELECT file_path, thumbnail_path FROM images WHERE generation_id = ?1")
        .map_err(|e| e.to_string())?;
    let paths: Vec<(String, String)> = stmt
        .query_map(params![id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;
    let fm = file_manager::FileManager::new(app_data_dir);
    for (file, thumb) in &paths {
        let _ = fm.delete_image(file, thumb);
    }

    conn.execute("DELETE FROM images WHERE generation_id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM generations WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn create_new_conversation(conn: &rusqlite::Connection, prompt: &str) -> Result<String, String> {
    let conv_id = uuid::Uuid::new_v4().to_string();
    let title = if prompt.len() > 40 { format!("{}...", &prompt[..40]) } else { prompt.to_string() };
    conn.execute(
        "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?1, ?2, datetime('now'), datetime('now'))",
        params![conv_id, title],
    ).map_err(|e| e.to_string())?;
    Ok(conv_id)
}

fn conversations_base_sql() -> String {
    "SELECT c.id, c.title, c.created_at, c.updated_at, \
     (SELECT COUNT(*) FROM generations WHERE conversation_id = c.id) as generation_count, \
     (SELECT i.thumbnail_path FROM generations g JOIN images i ON i.generation_id = g.id \
      WHERE g.conversation_id = c.id ORDER BY g.created_at DESC LIMIT 1) as latest_thumbnail \
     FROM conversations c \
     ORDER BY c.updated_at DESC".to_string()
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
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        }).map_err(|e| e.to_string())?;
        for row in rows {
            if let Ok(r) = row { orphans.push(r); }
        }
    }

    for (gen_id, prompt, created_at) in &orphans {
        let conv_id = uuid::Uuid::new_v4().to_string();
        let title = if prompt.len() > 40 { format!("{}...", &prompt[..40]) } else { prompt.clone() };
        conn.execute(
            "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![conv_id, title, created_at, created_at],
        ).map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE generations SET conversation_id = ?1 WHERE id = ?2",
            params![conv_id, gen_id],
        ).map_err(|e| e.to_string())?;
    }

    // Now query conversations
    let (sql, query_params): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(q) = &query {
        if !q.is_empty() {
            let pattern = format!("%{}%", q);
            (
                "SELECT c.id, c.title, c.created_at, c.updated_at, \
                 (SELECT COUNT(*) FROM generations WHERE conversation_id = c.id) as generation_count, \
                 (SELECT i.thumbnail_path FROM generations g JOIN images i ON i.generation_id = g.id \
                  WHERE g.conversation_id = c.id ORDER BY g.created_at DESC LIMIT 1) as latest_thumbnail \
                 FROM conversations c \
                 WHERE c.title LIKE ?1 \
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
    let params_refs: Vec<&dyn rusqlite::types::ToSql> = query_params.iter().map(|p| p.as_ref()).collect();
    let conversations = stmt.query_map(params_refs.as_slice(), |row| {
        Ok(Conversation {
            id: row.get("id")?,
            title: row.get("title")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
            generation_count: row.get("generation_count")?,
            latest_thumbnail: row.get("latest_thumbnail")?,
        })
    }).map_err(|e| e.to_string())?
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
        .prepare("SELECT * FROM generations WHERE conversation_id = ?1 ORDER BY created_at ASC")
        .map_err(|e| e.to_string())?;
    let generations: Vec<Generation> = stmt
        .query_map(params![conversation_id], row_to_generation)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let gen_ids: Vec<&str> = generations.iter().map(|g| g.id.as_str()).collect();
    let mut image_map: std::collections::HashMap<String, Vec<GeneratedImage>> =
        std::collections::HashMap::new();

    if !gen_ids.is_empty() {
        let placeholders: Vec<&str> = gen_ids.iter().map(|_| "?").collect();
        let sql = format!(
            "SELECT * FROM images WHERE generation_id IN ({})",
            placeholders.join(",")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = gen_ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
        let mut img_stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let images = img_stmt
            .query_map(params.as_slice(), row_to_image)
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok());
        for img in images {
            image_map.entry(img.generation_id.clone()).or_default().push(img);
        }
    }

    let results = generations.into_iter().map(|gen| {
        let images = image_map.remove(&gen.id).unwrap_or_default();
        GenerationResult { generation: gen, images }
    }).collect();

    Ok(results)
}

// --- Lightbox commands ---

#[tauri::command]
fn copy_image_to_clipboard(image_path: String) -> Result<(), String> {
    let data = std::fs::read(&image_path)
        .map_err(|e| format!("Read image failed: {}", e))?;
    let img = image::load_from_memory(&data)
        .map_err(|e| format!("Decode image failed: {}", e))?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();

    let mut clipboard = arboard::Clipboard::new()
        .map_err(|e| format!("Clipboard access failed: {}", e))?;
    clipboard.set_image(arboard::ImageData {
        width: w as usize,
        height: h as usize,
        bytes: std::borrow::Cow::Owned(rgba.into_raw()),
    }).map_err(|e| format!("Copy to clipboard failed: {}", e))?;

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
        .add_filter("PNG Image", &["png"])
        .save_file()
        .await
        .ok_or_else(|| "Save cancelled".to_string())?;

    let save_path = save_path.path().to_path_buf();
    tokio::task::spawn_blocking(move || {
        std::fs::copy(&image_path, &save_path)
            .map(|_| ())
            .map_err(|e| format!("Save failed: {}", e))
    }).await
        .map_err(|e| e.to_string())?
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
    log::info!("Config path: {}", config::AppConfig::config_path().display());

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
            save_base_url,
            get_base_url,
            generate_image,
            search_generations,
            delete_generation,
            get_conversations,
            get_conversation_generations,
            copy_image_to_clipboard,
            save_image_to_file,
        ])
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().expect("Cannot determine app data dir");
            let fm = file_manager::FileManager::new(app_data_dir);
            fm.ensure_dirs()?;

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
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
