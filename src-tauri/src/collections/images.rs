use crate::models::*;
use rusqlite::{params, Connection};
use std::collections::HashMap;

fn active_generation_filter(alias: &str) -> String {
    format!("{alias}.deleted_at IS NULL")
}

fn deleted_generation_filter(alias: &str) -> String {
    format!("{alias}.deleted_at IS NOT NULL")
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

fn generation_filters_to_sql(
    only_deleted: bool,
    query: Option<&str>,
    filters: Option<&GenerationSearchFilters>,
    project_id: Option<&str>,
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

    if let Some(project_id) = project_id.map(str::trim).filter(|value| !value.is_empty()) {
        clauses.push(format!(
            "EXISTS (SELECT 1 FROM conversations c WHERE c.id = g.conversation_id AND COALESCE(c.project_id, 'default') = ?{})",
            params.len() + 1
        ));
        params.push(Box::new(project_id.to_string()));
    }

    if let Some(filters) = filters {
        generation_search_value_clause(
            &mut clauses,
            &mut params,
            "g.engine",
            filters.model.as_deref(),
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

pub(crate) fn row_to_image(row: &rusqlite::Row) -> rusqlite::Result<GeneratedImage> {
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
            GenerationResult {
                generation,
                images,
                active_job_id: None,
            }
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

pub(crate) fn search_generations(
    conn: &rusqlite::Connection,
    query: Option<String>,
    page: Option<i32>,
    only_deleted: Option<bool>,
    filters: Option<GenerationSearchFilters>,
    project_id: Option<String>,
) -> Result<SearchResult, String> {
    let page = page.unwrap_or(1);
    let page_size = DEFAULT_PAGE_SIZE;
    let offset = (i64::from(page) - 1) * i64::from(page_size);
    let only_deleted = only_deleted.unwrap_or(false);

    let (where_sql, params_boxed) = generation_filters_to_sql(
        only_deleted,
        query.as_deref(),
        filters.as_ref(),
        project_id.as_deref(),
    );
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

    let results = generation_results_with_images(conn, generations)?;

    Ok(SearchResult {
        generations: results,
        total: count,
        page,
        page_size,
    })
}

pub(crate) fn create_folder(conn: &rusqlite::Connection, name: String) -> Result<Folder, String> {
    super::IMAGE_FOLDERS
        .create(conn, name)
        .map_err(|e| e.to_string())
}

pub(crate) fn rename_folder(
    conn: &rusqlite::Connection,
    id: String,
    name: String,
) -> Result<(), String> {
    if id == "default" {
        return Err("默认收藏文件夹不可重命名".to_string());
    }

    super::IMAGE_FOLDERS
        .rename(conn, &id, &name)
        .map_err(|e| e.to_string())
}

pub(crate) fn delete_folder(conn: &rusqlite::Connection, id: String) -> Result<(), String> {
    if id == "default" {
        return Err("默认收藏文件夹不可删除".to_string());
    }

    super::IMAGE_FOLDERS
        .delete(conn, &id)
        .map_err(|e| e.to_string())
}

pub(crate) fn get_folders(conn: &rusqlite::Connection) -> Result<Vec<Folder>, String> {
    super::IMAGE_FOLDERS.list(conn).map_err(|e| e.to_string())
}

pub(crate) fn add_image_to_folders(
    conn: &rusqlite::Connection,
    image_id: String,
    folder_ids: Vec<String>,
) -> Result<(), String> {
    super::IMAGE_FOLDERS
        .add(conn, &image_id, &folder_ids)
        .map_err(|e| e.to_string())
}

pub(crate) fn remove_image_from_folders(
    conn: &rusqlite::Connection,
    image_id: String,
    folder_ids: Vec<String>,
) -> Result<(), String> {
    super::IMAGE_FOLDERS
        .remove(conn, &image_id, &folder_ids)
        .map_err(|e| e.to_string())
}

pub(crate) fn get_image_folders(
    conn: &rusqlite::Connection,
    image_id: String,
) -> Result<Vec<String>, String> {
    super::IMAGE_FOLDERS
        .containing(conn, &image_id)
        .map_err(|e| e.to_string())
}

pub(crate) fn get_favorite_images(
    conn: &rusqlite::Connection,
    folder_id: Option<String>,
    query: Option<String>,
    page: Option<i32>,
) -> Result<SearchResult, String> {
    let page = page.unwrap_or(1);
    let page_size = DEFAULT_PAGE_SIZE;
    let offset = (i64::from(page) - 1) * i64::from(page_size);
    let pattern = query.filter(|q| !q.is_empty()).map(|q| format!("%{q}%"));
    let from = "FROM generations g JOIN images i ON i.generation_id = g.id
                JOIN folder_images fi ON fi.image_id = i.id WHERE g.deleted_at IS NULL
                AND (?1 IS NULL OR fi.folder_id = ?1) AND (?2 IS NULL OR g.prompt LIKE ?2)";
    let count: i32 = conn
        .query_row(
            &format!("SELECT COUNT(DISTINCT g.id) {from}"),
            params![folder_id, pattern],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    let order = if folder_id.is_some() {
        "fi.added_at"
    } else {
        "g.created_at"
    };
    let mut stmt = conn
        .prepare(&format!(
            "SELECT DISTINCT g.* {from} ORDER BY {order} DESC LIMIT ?3 OFFSET ?4"
        ))
        .map_err(|e| e.to_string())?;
    let generations = stmt
        .query_map(
            params![folder_id, pattern, page_size, offset],
            row_to_generation,
        )
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<_>>()
        .map_err(|e| e.to_string())?;
    Ok(SearchResult {
        generations: generation_results_with_images(conn, generations)?,
        total: count,
        page,
        page_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_filters_to_sql_adds_project_scope_clause() {
        let (sql, params) = generation_filters_to_sql(
            false,
            Some("sunrise"),
            Some(&GenerationSearchFilters {
                model: Some("gpt-image-2".into()),
                created_from: Some("2026-05-01".into()),
                created_to: None,
            }),
            Some("project-1"),
        );

        assert!(sql.contains("EXISTS (SELECT 1 FROM conversations c"));
        assert!(sql.contains("COALESCE(c.project_id, 'default')"));
        assert_eq!(params.len(), 4);
    }
}
