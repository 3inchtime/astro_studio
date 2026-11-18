use crate::current_timestamp;
use crate::error::AppError;
use crate::models::*;
use rusqlite::{params, OptionalExtension};

fn row_to_prompt_favorite(row: &rusqlite::Row) -> rusqlite::Result<PromptFavorite> {
    Ok(PromptFavorite {
        id: row.get("id")?,
        prompt: row.get("prompt")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub(crate) fn create_prompt_favorite(
    conn: &rusqlite::Connection,
    prompt: String,
) -> Result<PromptFavorite, AppError> {
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(AppError::Validation {
            message: "Prompt cannot be empty".to_string(),
        });
    }

    let timestamp = current_timestamp();

    if let Some(existing) = conn
        .query_row(
            "SELECT id, prompt, created_at, updated_at FROM prompt_favorites WHERE prompt = ?1 COLLATE NOCASE",
            params![&prompt],
            row_to_prompt_favorite,
        )
        .optional()
        .map_err(|e| AppError::Database {
            message: format!("Query prompt favorite failed: {}", e),
        })?
    {
        conn.execute(
            "UPDATE prompt_favorites SET prompt = ?1, updated_at = ?2 WHERE id = ?3",
            params![&prompt, &timestamp, &existing.id],
        )
        .map_err(|e| AppError::Database {
            message: format!("Update prompt favorite failed: {}", e),
        })?;

        return Ok(PromptFavorite {
            prompt,
            updated_at: timestamp,
            ..existing
        });
    }

    let id = uuid::Uuid::new_v4().to_string();
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| AppError::Database {
            message: format!("Begin transaction failed: {}", e),
        })?;
    tx.execute(
        "INSERT INTO prompt_favorites (id, prompt, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![&id, &prompt, &timestamp],
    )
    .map_err(|e| AppError::Database {
        message: format!("Insert prompt favorite failed: {}", e),
    })?;
    tx.execute(
        "INSERT OR IGNORE INTO prompt_folder_favorites (folder_id, prompt_favorite_id, added_at) VALUES ('default', ?1, ?2)",
        params![&id, &timestamp],
    )
    .map_err(|e| AppError::Database {
        message: format!("Add to default folder failed: {}", e),
    })?;
    tx.commit().map_err(|e| AppError::Database {
        message: format!("Commit transaction failed: {}", e),
    })?;

    Ok(PromptFavorite {
        id,
        prompt,
        created_at: timestamp.clone(),
        updated_at: timestamp,
    })
}

pub(crate) fn get_prompt_favorites(
    conn: &rusqlite::Connection,
    query: Option<String>,
    folder_id: Option<String>,
) -> Result<Vec<PromptFavorite>, AppError> {
    let query = query
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let folder_id = folder_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let pattern = query.map(|query| format!("%{query}%"));
    let (join, order) = if folder_id.is_some() {
        ("JOIN prompt_folder_favorites pff ON pff.prompt_favorite_id = pf.id AND pff.folder_id = ?1", "pff.added_at DESC, pf.updated_at DESC")
    } else {
        ("", "pf.updated_at DESC, pf.created_at DESC")
    };
    let sql = format!(
        "SELECT pf.id, pf.prompt, pf.created_at, pf.updated_at FROM prompt_favorites pf {join}
                WHERE (?2 IS NULL OR pf.prompt LIKE ?2) ORDER BY {order}"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| AppError::Database {
        message: format!("Prepare query failed: {e}"),
    })?;
    let rows = stmt
        .query_map(params![folder_id, pattern], row_to_prompt_favorite)
        .map_err(|e| AppError::Database {
            message: format!("Query favorites failed: {e}"),
        })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub(crate) fn delete_prompt_favorite(
    conn: &rusqlite::Connection,
    id: String,
) -> Result<(), AppError> {
    conn.execute("DELETE FROM prompt_favorites WHERE id = ?1", params![id])
        .map_err(|e| AppError::Database {
            message: format!("Delete prompt favorite failed: {}", e),
        })?;
    Ok(())
}

pub(crate) fn create_prompt_folder(
    conn: &rusqlite::Connection,
    name: String,
) -> Result<Folder, AppError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation {
            message: "Folder name cannot be empty".to_string(),
        });
    }

    super::PROMPT_FOLDERS
        .create(conn, name)
        .map_err(|e| AppError::Database {
            message: format!("Insert prompt folder failed: {e}"),
        })
}

pub(crate) fn rename_prompt_folder(
    conn: &rusqlite::Connection,
    id: String,
    name: String,
) -> Result<(), AppError> {
    if id == "default" {
        return Err(AppError::Validation {
            message: "Default folder cannot be renamed".to_string(),
        });
    }
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation {
            message: "Folder name cannot be empty".to_string(),
        });
    }

    super::PROMPT_FOLDERS
        .rename(conn, &id, &name)
        .map_err(|e| AppError::Database {
            message: format!("Update prompt folder failed: {e}"),
        })
}

pub(crate) fn delete_prompt_folder(
    conn: &rusqlite::Connection,
    id: String,
) -> Result<(), AppError> {
    if id == "default" {
        return Err(AppError::Validation {
            message: "Default folder cannot be deleted".to_string(),
        });
    }

    super::PROMPT_FOLDERS
        .delete(conn, &id)
        .map_err(|e| AppError::Database {
            message: format!("Delete prompt folder failed: {e}"),
        })
}

pub(crate) fn get_prompt_folders(conn: &rusqlite::Connection) -> Result<Vec<Folder>, AppError> {
    super::PROMPT_FOLDERS
        .list(conn)
        .map_err(|e| AppError::Database {
            message: format!("Query folders failed: {e}"),
        })
}

pub(crate) fn add_prompt_favorite_to_folders(
    conn: &rusqlite::Connection,
    favorite_id: String,
    folder_ids: Vec<String>,
) -> Result<(), AppError> {
    super::PROMPT_FOLDERS
        .add(conn, &favorite_id, &folder_ids)
        .map_err(|e| AppError::Database {
            message: format!("Add to folder failed: {e}"),
        })
}

pub(crate) fn remove_prompt_favorite_from_folders(
    conn: &rusqlite::Connection,
    favorite_id: String,
    folder_ids: Vec<String>,
) -> Result<(), AppError> {
    super::PROMPT_FOLDERS
        .remove(conn, &favorite_id, &folder_ids)
        .map_err(|e| AppError::Database {
            message: format!("Remove from folder failed: {e}"),
        })
}

pub(crate) fn get_prompt_favorite_folders(
    conn: &rusqlite::Connection,
    favorite_id: String,
) -> Result<Vec<String>, AppError> {
    super::PROMPT_FOLDERS
        .containing(conn, &favorite_id)
        .map_err(|e| AppError::Database {
            message: format!("Query folder ids failed: {e}"),
        })
}
