use crate::{db::Database, file_manager, models::*};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use tauri::Manager;

fn active_generation_filter(alias: &str) -> String {
    format!("{alias}.deleted_at IS NULL")
}

fn deleted_generation_filter(alias: &str) -> String {
    format!("{alias}.deleted_at IS NOT NULL")
}

fn trash_cutoff_timestamp(retention_days: u32) -> String {
    (chrono::Utc::now() - chrono::Duration::days(retention_days as i64))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub(crate) fn row_to_generation(row: &rusqlite::Row) -> rusqlite::Result<Generation> {
    let source_image_paths: String = row.get("source_image_paths")?;
    let parsed_source_image_paths = serde_json::from_str(&source_image_paths).unwrap_or_default();

    Ok(Generation {
        id: row.get("id")?,
        prompt: row.get("prompt")?,
        engine: row.get("engine")?,
        request_kind: row.get("request_kind")?,
        size: row.get("size")?,
        quality: row.get("quality")?,
        background: row.get("background")?,
        output_format: row.get("output_format")?,
        output_compression: row.get("output_compression")?,
        moderation: row.get("moderation")?,
        input_fidelity: row.get("input_fidelity")?,
        image_count: row.get("image_count")?,
        source_image_count: row.get("source_image_count")?,
        source_image_paths: parsed_source_image_paths,
        request_metadata: row.get("request_metadata")?,
        status: row.get("status")?,
        error_message: row.get("error_message")?,
        created_at: row.get("created_at")?,
        deleted_at: row.get("deleted_at")?,
    })
}

fn generation_search_value_clause(
    clauses: &mut Vec<String>,
    params: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
    column: &str,
    value: Option<&str>,
) {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) {
        clauses.push(format!("{column} = ?{}", params.len() + 1));
        params.push(Box::new(value.to_string()));
    }
}

fn generation_search_range_clause(
    clauses: &mut Vec<String>,
    params: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
    column: &str,
    lower_bound: Option<&str>,
    upper_bound: Option<&str>,
) {
    if let Some(value) = lower_bound.map(str::trim).filter(|value| !value.is_empty()) {
        clauses.push(format!("date({column}) >= date(?{})", params.len() + 1));
        params.push(Box::new(value.to_string()));
    }
    if let Some(value) = upper_bound.map(str::trim).filter(|value| !value.is_empty()) {
        clauses.push(format!("date({column}) <= date(?{})", params.len() + 1));
        params.push(Box::new(value.to_string()));
    }
}

fn generation_source_image_count_clause(
    clauses: &mut Vec<String>,
    params: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
    value: Option<&str>,
) {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };

    match value {
        "any" => {}
        "0" => {
            clauses.push(format!("g.source_image_count = ?{}", params.len() + 1));
            params.push(Box::new(0_i64));
        }
        "1" => {
            clauses.push(format!("g.source_image_count = ?{}", params.len() + 1));
            params.push(Box::new(1_i64));
        }
        "2" => {
            clauses.push(format!("g.source_image_count = ?{}", params.len() + 1));
            params.push(Box::new(2_i64));
        }
        "3" => {
            clauses.push(format!("g.source_image_count = ?{}", params.len() + 1));
            params.push(Box::new(3_i64));
        }
        "4+" => {
            clauses.push(format!("g.source_image_count >= ?{}", params.len() + 1));
            params.push(Box::new(4_i64));
        }
        _ => {}
    }
}

fn generation_filters_to_sql(
    only_deleted: bool,
    query: Option<&str>,
    filters: Option<&GenerationSearchFilters>,
) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
    let mut clauses = vec![if only_deleted {
        deleted_generation_filter("g")
    } else {
        active_generation_filter("g")
    }];
    let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(query) = query.map(str::trim).filter(|value| !value.is_empty()) {
        clauses.push(format!("g.prompt LIKE ?{}", params.len() + 1));
        params.push(Box::new(format!("%{}%", query)));
    }

    if let Some(filters) = filters {
        generation_search_value_clause(
            &mut clauses,
            &mut params,
            "g.engine",
            filters.model.as_deref(),
        );
        generation_search_value_clause(
            &mut clauses,
            &mut params,
            "g.request_kind",
            filters.request_kind.as_deref(),
        );
        generation_search_value_clause(
            &mut clauses,
            &mut params,
            "g.status",
            filters.status.as_deref(),
        );
        generation_search_value_clause(
            &mut clauses,
            &mut params,
            "g.size",
            filters.size.as_deref(),
        );
        generation_search_value_clause(
            &mut clauses,
            &mut params,
            "g.quality",
            filters.quality.as_deref(),
        );
        generation_search_value_clause(
            &mut clauses,
            &mut params,
            "g.background",
            filters.background.as_deref(),
        );
        generation_search_value_clause(
            &mut clauses,
            &mut params,
            "g.output_format",
            filters.output_format.as_deref(),
        );
        generation_search_value_clause(
            &mut clauses,
            &mut params,
            "g.moderation",
            filters.moderation.as_deref(),
        );
        generation_search_value_clause(
            &mut clauses,
            &mut params,
            "g.input_fidelity",
            filters.input_fidelity.as_deref(),
        );
        generation_source_image_count_clause(
            &mut clauses,
            &mut params,
            filters.source_image_count.as_deref(),
        );
        generation_search_range_clause(
            &mut clauses,
            &mut params,
            "g.created_at",
            filters.created_from.as_deref(),
            filters.created_to.as_deref(),
        );
    }

    (format!("WHERE {}", clauses.join(" AND ")), params)
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

pub(crate) fn generation_results_with_images(
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

pub(crate) fn purge_trashed_generations(
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
pub(crate) fn search_generations(
    db: tauri::State<'_, Database>,
    query: Option<String>,
    page: Option<i32>,
    only_deleted: Option<bool>,
    filters: Option<GenerationSearchFilters>,
) -> Result<SearchResult, String> {
    let page = page.unwrap_or(1);
    let page_size = DEFAULT_PAGE_SIZE;
    let offset = (page - 1) * page_size;
    let only_deleted = only_deleted.unwrap_or(false);

    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let (where_sql, params_boxed) =
        generation_filters_to_sql(only_deleted, query.as_deref(), filters.as_ref());
    let count_sql = format!("SELECT COUNT(*) FROM generations g {}", where_sql);
    let count: i32 = {
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_boxed.iter().map(|param| param.as_ref()).collect();
        conn.query_row(&count_sql, params_refs.as_slice(), |row| row.get(0))
            .map_err(|e| e.to_string())?
    };

    let mut query_params = params_boxed;
    query_params.push(Box::new(page_size));
    query_params.push(Box::new(offset));
    let query_refs: Vec<&dyn rusqlite::types::ToSql> =
        query_params.iter().map(|param| param.as_ref()).collect();

    let query_sql = format!(
        "SELECT g.* FROM generations g {} ORDER BY g.created_at DESC LIMIT ?{} OFFSET ?{}",
        where_sql,
        query_params.len() - 1,
        query_params.len()
    );
    let mut stmt = conn.prepare(&query_sql).map_err(|e| e.to_string())?;
    let generations = stmt
        .query_map(query_refs.as_slice(), row_to_generation)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect::<Vec<_>>();

    let results = generation_results_with_images(&conn, generations)?;

    Ok(SearchResult {
        generations: results,
        total: count,
        page,
        page_size,
    })
}

#[tauri::command]
pub(crate) fn delete_generation(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    log::info!("Moving generation {} to trash", id);
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE generations SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![crate::current_timestamp(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn restore_generation(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    log::info!("Restoring generation {}", id);
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let conversation_id = conn
        .query_row(
            "SELECT conversation_id FROM generations WHERE id = ?1",
            params![id.as_str()],
            |row| row.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten();

    conn.execute(
        "UPDATE generations SET deleted_at = NULL WHERE id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;

    if let Some(conversation_id) = conversation_id {
        conn.execute(
            "UPDATE conversations SET deleted_at = NULL, archived_at = NULL, updated_at = ?1 WHERE id = ?2",
            params![crate::current_timestamp(), conversation_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn permanently_delete_generation(
    app: tauri::AppHandle,
    db: tauri::State<'_, Database>,
    id: String,
) -> Result<(), String> {
    log::info!("Permanently deleting generation {}", id);
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    permanently_delete_generation_files_and_records(&app, &conn, &id)
}

#[tauri::command]
pub(crate) fn create_folder(
    db: tauri::State<'_, Database>,
    name: String,
) -> Result<Folder, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = crate::current_timestamp();
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
pub(crate) fn rename_folder(
    db: tauri::State<'_, Database>,
    id: String,
    name: String,
) -> Result<(), String> {
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
pub(crate) fn delete_folder(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    if id == "default" {
        return Err("默认收藏文件夹不可删除".to_string());
    }
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM folders WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn get_folders(db: tauri::State<'_, Database>) -> Result<Vec<Folder>, String> {
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
pub(crate) fn add_image_to_folders(
    db: tauri::State<'_, Database>,
    image_id: String,
    folder_ids: Vec<String>,
) -> Result<(), String> {
    let added_at = crate::current_timestamp();
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
pub(crate) fn remove_image_from_folders(
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
pub(crate) fn get_image_folders(
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
pub(crate) fn get_favorite_images(
    db: tauri::State<'_, Database>,
    folder_id: Option<String>,
    query: Option<String>,
    page: Option<i32>,
) -> Result<SearchResult, String> {
    let page = page.unwrap_or(1);
    let page_size = DEFAULT_PAGE_SIZE;
    let offset = (page - 1) * page_size;
    let conn = db.conn.lock().map_err(|e| e.to_string())?;

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
