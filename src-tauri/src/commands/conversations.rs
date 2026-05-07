use crate::current_timestamp;
use crate::db::Database;
use crate::error::AppError;
use crate::gallery;
use crate::models::*;
use rusqlite::params;
use tauri::State;

const DEFAULT_PROJECT_ID: &str = "default";
const DEFAULT_CONVERSATION_TITLE: &str = "New Conversation";

fn ensure_default_project(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO projects (id, name, created_at, updated_at)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
        params![DEFAULT_PROJECT_ID, "Default Project"],
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

pub(crate) fn resolve_conversation_id_for_generation(
    conn: &rusqlite::Connection,
    conversation_id: Option<&str>,
    project_id: Option<&str>,
    prompt: &str,
) -> Result<String, AppError> {
    resolve_conversation_id(conn, conversation_id, project_id, prompt)
        .map_err(|e| AppError::Database { message: e })
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
pub(crate) fn create_conversation(
    db: State<'_, Database>,
    title: Option<String>,
    project_id: Option<String>,
) -> Result<Conversation, AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
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

    Ok(fetch_conversation(&conn, &conv_id)?)
}

#[tauri::command]
pub(crate) fn get_conversations(
    db: State<'_, Database>,
    query: Option<String>,
    project_id: Option<String>,
    include_archived: Option<bool>,
) -> Result<Vec<Conversation>, AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
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
pub(crate) fn rename_conversation(
    db: State<'_, Database>,
    id: String,
    title: String,
) -> Result<(), AppError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::Validation {
            message: "Conversation title cannot be empty.".to_string(),
        });
    }

    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    conn.execute(
        "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
        params![title, current_timestamp(), id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Update conversation failed: {}", e),
    })?;
    Ok(())
}

#[tauri::command]
pub(crate) fn move_conversation_to_project(
    db: State<'_, Database>,
    id: String,
    project_id: String,
) -> Result<(), AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let project_id = resolve_project_id(&conn, Some(project_id.as_str()))?;
    conn.execute(
        "UPDATE conversations SET project_id = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
        params![project_id, current_timestamp(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn archive_conversation(
    db: State<'_, Database>,
    id: String,
) -> Result<(), AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
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
pub(crate) fn unarchive_conversation(
    db: State<'_, Database>,
    id: String,
) -> Result<(), AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    conn.execute(
        "UPDATE conversations SET archived_at = NULL, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![current_timestamp(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn pin_conversation(
    db: State<'_, Database>,
    id: String,
) -> Result<(), AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let timestamp = current_timestamp();
    conn.execute(
        "UPDATE conversations SET pinned_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn unpin_conversation(
    db: State<'_, Database>,
    id: String,
) -> Result<(), AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    conn.execute(
        "UPDATE conversations SET pinned_at = NULL, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![current_timestamp(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn delete_conversation(
    db: State<'_, Database>,
    id: String,
) -> Result<(), AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
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
pub(crate) fn get_conversation_generations(
    db: State<'_, Database>,
    conversation_id: String,
) -> Result<Vec<GenerationResult>, AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;

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
