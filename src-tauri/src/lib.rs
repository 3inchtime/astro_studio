mod api_gateway;
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
    db.set_setting(SETTING_API_KEY, &key)
}

#[tauri::command]
fn get_api_key(db: tauri::State<'_, Database>) -> Result<Option<String>, String> {
    db.get_setting(SETTING_API_KEY)
}

#[tauri::command]
fn save_base_url(db: tauri::State<'_, Database>, url: String) -> Result<(), String> {
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
            let mut image_paths = Vec::new();

            let conn = db.conn.lock().map_err(|e| e.to_string())?;
            let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

            tx.execute(
                "INSERT INTO generations (id, prompt, engine, size, quality, status) VALUES (?1, ?2, ?3, ?4, ?5, 'processing')",
                params![generation_id, prompt, ENGINE_GPT_IMAGE_2, size, quality],
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

            let _ = app.emit("generation:complete", serde_json::json!({
                "generation_id": generation_id,
                "status": "completed"
            }));

            Ok(GenerateResult { generation_id, image_paths })
        }
        Err(e) => {
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

// --- App entry point ---

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db_path = dirs::data_dir()
        .expect("Cannot determine app data directory")
        .join("astro-studio")
        .join("astro_studio.db");

    let database = Database::open(&db_path).expect("Failed to open database");
    database.run_migrations().expect("Failed to run migrations");

    let engine = api_gateway::GptImageEngine::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
                use window_vibrancy::NSVisualEffectViewMaterial;
                let window = app.get_webview_window("main").unwrap();
                let _ = apply_vibrancy(&window, NSVisualEffectViewMaterial::HudWindow, None, None);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
