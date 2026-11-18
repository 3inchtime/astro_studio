use crate::collections::images;
use crate::current_timestamp;
use crate::error::AppError;
use crate::models::*;
use rusqlite::params;

use super::{ensure_default_project, resolve_project_id, DEFAULT_PROJECT_ID};
const DEFAULT_CONVERSATION_TITLE: &str = "New Conversation";

pub(crate) fn conversation_title_from_prompt(prompt: &str) -> String {
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
        let updated = conn
            .execute(
                "UPDATE conversations
             SET title = CASE WHEN title = ?2 AND NOT EXISTS (
                 SELECT 1 FROM generations WHERE conversation_id = ?1 AND deleted_at IS NULL
             ) THEN ?3 ELSE title END,
             project_id = COALESCE(project_id, ?4), archived_at = NULL, updated_at = ?5
             WHERE id = ?1 AND deleted_at IS NULL",
                params![
                    conv_id,
                    DEFAULT_CONVERSATION_TITLE,
                    conversation_title_from_prompt(prompt),
                    project_id,
                    current_timestamp()
                ],
            )
            .map_err(|e| e.to_string())?;
        if updated != 0 {
            return Ok(conv_id.to_string());
        }
    }

    create_new_conversation(conn, prompt, &project_id)
}

const CONVERSATION_PROJECTION: &str = "SELECT c.id, COALESCE(c.project_id, 'default') as project_id, p.name as project_name, c.title, c.created_at, c.updated_at, \
         c.archived_at, c.pinned_at, c.deleted_at, \
         (SELECT COUNT(*) FROM generations WHERE conversation_id = c.id AND deleted_at IS NULL) as generation_count, \
         (SELECT g.created_at FROM generations g \
          WHERE g.conversation_id = c.id AND g.deleted_at IS NULL ORDER BY g.created_at DESC LIMIT 1) as latest_generation_at, \
         (SELECT i.thumbnail_path FROM generations g JOIN images i ON i.generation_id = g.id \
          WHERE g.conversation_id = c.id AND g.deleted_at IS NULL ORDER BY g.created_at DESC LIMIT 1) as latest_thumbnail \
         FROM conversations c \
         LEFT JOIN projects p ON p.id = c.project_id";

fn conversations_list_sql(where_sql: &str) -> String {
    format!(
        "{CONVERSATION_PROJECTION} WHERE {} AND ( \
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
        &format!("{CONVERSATION_PROJECTION} WHERE c.id = ?1"),
        params![conversation_id],
        row_to_conversation,
    )
    .map_err(|e| e.to_string())
}

pub(crate) fn create_conversation(
    conn: &rusqlite::Connection,
    title: Option<String>,
    project_id: Option<String>,
) -> Result<Conversation, AppError> {
    let tx = conn.unchecked_transaction()?;
    let project_id = resolve_project_id(&tx, project_id.as_deref())?;
    let conv_id = uuid::Uuid::new_v4().to_string();
    let timestamp = current_timestamp();
    let title = title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_CONVERSATION_TITLE);

    tx.execute(
        "INSERT INTO conversations (id, project_id, title, created_at, updated_at, archived_at, pinned_at, deleted_at)
         VALUES (?1, ?2, ?3, ?4, ?4, NULL, NULL, NULL)",
        params![conv_id, project_id, title, &timestamp],
    )
    .map_err(|e| e.to_string())?;

    let conversation = fetch_conversation(&tx, &conv_id)?;
    tx.commit()?;
    Ok(conversation)
}

fn backfill_conversations(conn: &rusqlite::Connection) -> Result<(), AppError> {
    let tx = conn.unchecked_transaction()?;
    ensure_default_project(&tx)?;
    // Backfill: assign orphaned generations to new conversations
    let mut orphans: Vec<(String, String, String)> = Vec::new();
    {
        let mut stmt = tx
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
        tx.execute(
            "INSERT INTO conversations (id, project_id, title, created_at, updated_at, archived_at, pinned_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, ?4, NULL, NULL, NULL)",
            params![conv_id, DEFAULT_PROJECT_ID, title, created_at],
        )
        .map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE generations SET conversation_id = ?1 WHERE id = ?2",
            params![conv_id, gen_id],
        )
        .map_err(|e| e.to_string())?;
    }

    tx.execute(
        "UPDATE conversations SET project_id = ?1 WHERE project_id IS NULL",
        params![DEFAULT_PROJECT_ID],
    )
    .map_err(|e| e.to_string())?;

    tx.commit()?;
    Ok(())
}

pub(crate) fn get_conversations(
    conn: &rusqlite::Connection,
    query: Option<String>,
    project_id: Option<String>,
    include_archived: Option<bool>,
) -> Result<Vec<Conversation>, AppError> {
    backfill_conversations(conn)?;

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

pub(crate) fn rename_conversation(
    conn: &rusqlite::Connection,
    id: String,
    title: String,
) -> Result<(), AppError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(AppError::Validation {
            message: "Conversation title cannot be empty.".to_string(),
        });
    }

    conn.execute(
        "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
        params![title, current_timestamp(), id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Update conversation failed: {}", e),
    })?;
    Ok(())
}

pub(crate) fn move_conversation_to_project(
    conn: &rusqlite::Connection,
    id: String,
    project_id: String,
) -> Result<(), AppError> {
    let project_id = resolve_project_id(conn, Some(project_id.as_str()))?;
    conn.execute(
        "UPDATE conversations SET project_id = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
        params![project_id, current_timestamp(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn archive_conversation(
    conn: &rusqlite::Connection,
    id: String,
) -> Result<(), AppError> {
    let timestamp = current_timestamp();
    conn.execute(
        "UPDATE conversations SET archived_at = COALESCE(archived_at, ?1), updated_at = ?1
         WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn unarchive_conversation(
    conn: &rusqlite::Connection,
    id: String,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE conversations SET archived_at = NULL, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![current_timestamp(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn pin_conversation(conn: &rusqlite::Connection, id: String) -> Result<(), AppError> {
    let timestamp = current_timestamp();
    conn.execute(
        "UPDATE conversations SET pinned_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn unpin_conversation(conn: &rusqlite::Connection, id: String) -> Result<(), AppError> {
    conn.execute(
        "UPDATE conversations SET pinned_at = NULL, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![current_timestamp(), id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn delete_conversation(
    tx: &rusqlite::Transaction<'_>,
    id: &str,
) -> Result<(), AppError> {
    let timestamp = current_timestamp();
    tx.execute(
        "UPDATE conversations SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE generations SET deleted_at = COALESCE(deleted_at, ?1) WHERE conversation_id = ?2",
        params![timestamp, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn get_conversation_generations(
    conn: &rusqlite::Connection,
    conversation_id: String,
) -> Result<Vec<GenerationResult>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT g.*, j.id AS active_job_id FROM generations g
             LEFT JOIN generation_jobs j ON j.generation_id = g.id AND j.status IN ('queued', 'running')
             WHERE g.conversation_id = ?1 AND g.deleted_at IS NULL ORDER BY g.created_at ASC, g.id ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![conversation_id], |row| {
            Ok((
                images::row_to_generation(row)?,
                row.get::<_, Option<String>>("active_job_id")?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let (generations, jobs): (Vec<_>, Vec<_>) = rows.into_iter().unzip();
    let mut results = images::generation_results_with_images(conn, generations)?;
    for (result, job) in results.iter_mut().zip(jobs) {
        result.active_job_id = job;
    }
    Ok(results)
}

#[cfg(test)]
mod title_tests {
    use super::*;
    #[test]
    fn first_generation_preserves_explicit_title_and_names_default_conversation() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE projects(id TEXT PRIMARY KEY,name TEXT,created_at TEXT,updated_at TEXT,deleted_at TEXT);
            CREATE TABLE conversations(id TEXT,project_id TEXT,title TEXT,created_at TEXT,updated_at TEXT,archived_at TEXT,pinned_at TEXT,deleted_at TEXT);
            CREATE TABLE generations(id TEXT,conversation_id TEXT,deleted_at TEXT);
            INSERT INTO projects(id,name) VALUES ('original-project','Original');
            INSERT INTO conversations(id,project_id,title) VALUES ('test','default','Connection test · Saved provider'),('regular','original-project','New Conversation');").unwrap();
        let prompt = "A small blue circle on a plain white background.";
        for id in ["test", "regular"] {
            assert_eq!(
                resolve_conversation_id(&conn, Some(id), None, prompt).unwrap(),
                id
            );
        }
        let title = |id| {
            conn.query_row("SELECT title FROM conversations WHERE id=?1", [id], |r| {
                r.get::<_, String>(0)
            })
            .unwrap()
        };
        let project: String = conn
            .query_row(
                "SELECT project_id FROM conversations WHERE id='regular'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(project, "original-project");
        assert_eq!(title("test"), "Connection test · Saved provider");
        assert_eq!(title("regular"), conversation_title_from_prompt(prompt));
    }
}
