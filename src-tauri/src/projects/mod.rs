pub(crate) mod conversations;

use crate::current_timestamp;
use crate::error::AppError;
use crate::models::*;
use rusqlite::params;

const DEFAULT_PROJECT_ID: &str = "default";
const DEFAULT_PROJECT_NAME: &str = "Default Project";
const DEFAULT_NEW_PROJECT_NAME: &str = "New Project";

fn ensure_default_project(conn: &rusqlite::Connection) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO projects (id, name, created_at, updated_at)
         VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'), strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))",
        params![DEFAULT_PROJECT_ID, DEFAULT_PROJECT_NAME],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn resolve_project_id(
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

pub(crate) fn create_project(
    conn: &rusqlite::Connection,
    name: Option<String>,
) -> Result<Project, AppError> {
    let project_id = uuid::Uuid::new_v4().to_string();
    let timestamp = current_timestamp();
    let name = name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_NEW_PROJECT_NAME);

    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO projects (id, name, created_at, updated_at, archived_at, pinned_at, deleted_at)
         VALUES (?1, ?2, ?3, ?3, NULL, NULL, NULL)",
        params![project_id, name, &timestamp],
    )
    .map_err(|e| AppError::Database {
        message: format!("Insert project failed: {}", e),
    })?;

    let project = fetch_project(&tx, &project_id)?;
    tx.commit()?;
    Ok(project)
}

pub(crate) fn get_projects(
    conn: &rusqlite::Connection,
    include_archived: Option<bool>,
) -> Result<Vec<Project>, AppError> {
    ensure_default_project(conn)?;

    let include_archived = include_archived.unwrap_or(false);
    let sql = if include_archived {
        projects_base_sql("p.deleted_at IS NULL")
    } else {
        projects_base_sql("p.deleted_at IS NULL AND p.archived_at IS NULL")
    };

    let mut stmt = conn.prepare(&sql).map_err(|e| AppError::Database {
        message: format!("Prepare projects query failed: {}", e),
    })?;
    let projects = stmt
        .query_map([], row_to_project)
        .map_err(|e| AppError::Database {
            message: format!("Query projects failed: {}", e),
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(projects)
}

pub(crate) fn rename_project(
    conn: &rusqlite::Connection,
    id: String,
    name: String,
) -> Result<(), AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::Validation {
            message: "Project name cannot be empty.".to_string(),
        });
    }

    conn.execute(
        "UPDATE projects SET name = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL",
        params![name, current_timestamp(), id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Update project failed: {}", e),
    })?;
    Ok(())
}

pub(crate) fn archive_project(conn: &rusqlite::Connection, id: String) -> Result<(), AppError> {
    let timestamp = current_timestamp();
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE projects SET archived_at = COALESCE(archived_at, ?1), updated_at = ?1
         WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Archive project failed: {}", e),
    })?;
    tx.execute(
        "UPDATE conversations SET archived_at = COALESCE(archived_at, ?1), updated_at = ?1
         WHERE project_id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Archive conversations failed: {}", e),
    })?;
    tx.commit()?;
    Ok(())
}

pub(crate) fn unarchive_project(conn: &rusqlite::Connection, id: String) -> Result<(), AppError> {
    let timestamp = current_timestamp();
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE projects SET archived_at = NULL, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Unarchive project failed: {}", e),
    })?;
    tx.execute(
        "UPDATE conversations SET archived_at = NULL, updated_at = ?1
         WHERE project_id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Unarchive conversations failed: {}", e),
    })?;
    tx.commit()?;
    Ok(())
}

pub(crate) fn pin_project(conn: &rusqlite::Connection, id: String) -> Result<(), AppError> {
    let timestamp = current_timestamp();
    conn.execute(
        "UPDATE projects SET pinned_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Pin project failed: {}", e),
    })?;
    Ok(())
}

pub(crate) fn unpin_project(conn: &rusqlite::Connection, id: String) -> Result<(), AppError> {
    conn.execute(
        "UPDATE projects SET pinned_at = NULL, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![current_timestamp(), id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Unpin project failed: {}", e),
    })?;
    Ok(())
}

pub(crate) fn delete_project(tx: &rusqlite::Transaction<'_>, id: &str) -> Result<(), AppError> {
    if id == DEFAULT_PROJECT_ID {
        return Err(AppError::Validation {
            message: "The default project cannot be deleted.".to_string(),
        });
    }

    let timestamp = current_timestamp();
    tx.execute(
        "UPDATE projects SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Delete project failed: {}", e),
    })?;
    tx.execute(
        "UPDATE conversations SET deleted_at = ?1, updated_at = ?1 WHERE project_id = ?2 AND deleted_at IS NULL",
        params![timestamp, id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Delete project conversations failed: {}", e),
    })?;
    tx.execute(
        "UPDATE generations SET deleted_at = COALESCE(deleted_at, ?1)
         WHERE conversation_id IN (SELECT id FROM conversations WHERE project_id = ?2)",
        params![timestamp, id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Delete project generations failed: {}", e),
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn archive_rolls_back_project_when_a_conversation_update_fails() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        db.run_migrations().unwrap();
        let conn = db.conn.lock().unwrap();
        let project = create_project(&conn, Some("Fixture".into())).unwrap();
        conversations::create_conversation(&conn, None, Some(project.id.clone())).unwrap();
        conn.execute_batch("CREATE TRIGGER fixture_failure BEFORE UPDATE OF archived_at ON conversations BEGIN SELECT RAISE(ABORT, 'fixture'); END;").unwrap();
        assert!(archive_project(&conn, project.id.clone()).is_err());
        assert!(fetch_project(&conn, &project.id)
            .unwrap()
            .archived_at
            .is_none());
        conn.execute_batch("DROP TRIGGER fixture_failure").unwrap();
        archive_project(&conn, project.id.clone()).unwrap();
        assert!(conversations::get_conversations(
            &conn,
            None,
            Some(project.id.clone()),
            Some(false)
        )
        .unwrap()
        .is_empty());
        unarchive_project(&conn, project.id.clone()).unwrap();
        assert_eq!(
            conversations::get_conversations(&conn, None, Some(project.id), Some(false))
                .unwrap()
                .len(),
            1
        );
    }
}
