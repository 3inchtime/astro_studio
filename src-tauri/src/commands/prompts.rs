use crate::current_timestamp;
use crate::db::Database;
use crate::error::AppError;
use crate::models::*;
use rusqlite::{params, OptionalExtension};
use tauri::State;

fn row_to_prompt_favorite(row: &rusqlite::Row) -> rusqlite::Result<PromptFavorite> {
    Ok(PromptFavorite {
        id: row.get("id")?,
        prompt: row.get("prompt")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

#[tauri::command]
pub(crate) fn create_prompt_favorite(
    db: State<'_, Database>,
    prompt: String,
) -> Result<PromptFavorite, AppError> {
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(AppError::Validation {
            message: "Prompt cannot be empty".to_string(),
        });
    }

    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
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
    let tx = conn.unchecked_transaction().map_err(|e| AppError::Database {
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

#[tauri::command]
pub(crate) fn get_prompt_favorites(
    db: State<'_, Database>,
    query: Option<String>,
    folder_id: Option<String>,
) -> Result<Vec<PromptFavorite>, AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let query = query
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let folder_id = folder_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    match (query, folder_id) {
        (Some(query), Some(folder_id)) => {
            let pattern = format!("%{}%", query);
            let mut stmt = conn
                .prepare(
                    "SELECT pf.id, pf.prompt, pf.created_at, pf.updated_at
                     FROM prompt_favorites pf
                     JOIN prompt_folder_favorites pff ON pff.prompt_favorite_id = pf.id
                     WHERE pff.folder_id = ?1 AND pf.prompt LIKE ?2
                     ORDER BY pff.added_at DESC, pf.updated_at DESC",
                )
                .map_err(|e| AppError::Database {
                    message: format!("Prepare query failed: {}", e),
                })?;
            let favorites = stmt
                .query_map(params![folder_id, pattern], row_to_prompt_favorite)
                .map_err(|e| AppError::Database {
                    message: format!("Query favorites failed: {}", e),
                })?
                .filter_map(|row| row.ok())
                .collect();
            Ok(favorites)
        }
        (None, Some(folder_id)) => {
            let mut stmt = conn
                .prepare(
                    "SELECT pf.id, pf.prompt, pf.created_at, pf.updated_at
                     FROM prompt_favorites pf
                     JOIN prompt_folder_favorites pff ON pff.prompt_favorite_id = pf.id
                     WHERE pff.folder_id = ?1
                     ORDER BY pff.added_at DESC, pf.updated_at DESC",
                )
                .map_err(|e| AppError::Database {
                    message: format!("Prepare query failed: {}", e),
                })?;
            let favorites = stmt
                .query_map(params![folder_id], row_to_prompt_favorite)
                .map_err(|e| AppError::Database {
                    message: format!("Query favorites failed: {}", e),
                })?
                .filter_map(|row| row.ok())
                .collect();
            Ok(favorites)
        }
        (Some(query), None) => {
            let pattern = format!("%{}%", query);
            let mut stmt = conn
                .prepare(
                    "SELECT id, prompt, created_at, updated_at FROM prompt_favorites
                     WHERE prompt LIKE ?1
                     ORDER BY updated_at DESC, created_at DESC",
                )
                .map_err(|e| AppError::Database {
                    message: format!("Prepare query failed: {}", e),
                })?;
            let favorites = stmt
                .query_map(params![pattern], row_to_prompt_favorite)
                .map_err(|e| AppError::Database {
                    message: format!("Query favorites failed: {}", e),
                })?
                .filter_map(|row| row.ok())
                .collect();
            Ok(favorites)
        }
        (None, None) => {
            let mut stmt = conn
                .prepare(
                    "SELECT id, prompt, created_at, updated_at FROM prompt_favorites
                     ORDER BY updated_at DESC, created_at DESC",
                )
                .map_err(|e| AppError::Database {
                    message: format!("Prepare query failed: {}", e),
                })?;
            let favorites = stmt
                .query_map([], row_to_prompt_favorite)
                .map_err(|e| AppError::Database {
                    message: format!("Query favorites failed: {}", e),
                })?
                .filter_map(|row| row.ok())
                .collect();
            Ok(favorites)
        }
    }
}

#[tauri::command]
pub(crate) fn delete_prompt_favorite(
    db: State<'_, Database>,
    id: String,
) -> Result<(), AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    conn.execute("DELETE FROM prompt_favorites WHERE id = ?1", params![id])
        .map_err(|e| AppError::Database {
            message: format!("Delete prompt favorite failed: {}", e),
        })?;
    Ok(())
}

#[tauri::command]
pub(crate) fn create_prompt_folder(
    db: State<'_, Database>,
    name: String,
) -> Result<Folder, AppError> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation {
            message: "Folder name cannot be empty".to_string(),
        });
    }

    let id = uuid::Uuid::new_v4().to_string();
    let created_at = current_timestamp();
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    conn.execute(
        "INSERT INTO prompt_folders (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![id, name, &created_at],
    )
    .map_err(|e| AppError::Database {
        message: format!("Insert prompt folder failed: {}", e),
    })?;
    Ok(Folder {
        id,
        name,
        created_at,
    })
}

#[tauri::command]
pub(crate) fn rename_prompt_folder(
    db: State<'_, Database>,
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
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    conn.execute(
        "UPDATE prompt_folders SET name = ?1 WHERE id = ?2",
        params![name, id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Update prompt folder failed: {}", e),
    })?;
    Ok(())
}

#[tauri::command]
pub(crate) fn delete_prompt_folder(
    db: State<'_, Database>,
    id: String,
) -> Result<(), AppError> {
    if id == "default" {
        return Err(AppError::Validation {
            message: "Default folder cannot be deleted".to_string(),
        });
    }
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    conn.execute(
        "DELETE FROM prompt_folders WHERE id = ?1",
        params![id],
    )
    .map_err(|e| AppError::Database {
        message: format!("Delete prompt folder failed: {}", e),
    })?;
    Ok(())
}

#[tauri::command]
pub(crate) fn get_prompt_folders(db: State<'_, Database>) -> Result<Vec<Folder>, AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let mut stmt = conn
        .prepare("SELECT id, name, created_at FROM prompt_folders ORDER BY created_at ASC")
        .map_err(|e| AppError::Database {
            message: format!("Prepare query failed: {}", e),
        })?;
    let folders = stmt
        .query_map([], |row| {
            Ok(Folder {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
            })
        })
        .map_err(|e| AppError::Database {
            message: format!("Query folders failed: {}", e),
        })?
        .filter_map(|row| row.ok())
        .collect();
    Ok(folders)
}

#[tauri::command]
pub(crate) fn add_prompt_favorite_to_folders(
    db: State<'_, Database>,
    favorite_id: String,
    folder_ids: Vec<String>,
) -> Result<(), AppError> {
    let added_at = current_timestamp();
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    for folder_id in &folder_ids {
        conn.execute(
            "INSERT OR IGNORE INTO prompt_folder_favorites (folder_id, prompt_favorite_id, added_at) VALUES (?1, ?2, ?3)",
            params![folder_id, favorite_id, &added_at],
        )
        .map_err(|e| AppError::Database {
            message: format!("Add to folder failed: {}", e),
        })?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn remove_prompt_favorite_from_folders(
    db: State<'_, Database>,
    favorite_id: String,
    folder_ids: Vec<String>,
) -> Result<(), AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    for folder_id in &folder_ids {
        conn.execute(
            "DELETE FROM prompt_folder_favorites WHERE folder_id = ?1 AND prompt_favorite_id = ?2",
            params![folder_id, favorite_id],
        )
        .map_err(|e| AppError::Database {
            message: format!("Remove from folder failed: {}", e),
        })?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn get_prompt_favorite_folders(
    db: State<'_, Database>,
    favorite_id: String,
) -> Result<Vec<String>, AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let mut stmt = conn
        .prepare("SELECT folder_id FROM prompt_folder_favorites WHERE prompt_favorite_id = ?1")
        .map_err(|e| AppError::Database {
            message: format!("Prepare query failed: {}", e),
        })?;
    let folder_ids = stmt
        .query_map(params![favorite_id], |row| row.get(0))
        .map_err(|e| AppError::Database {
            message: format!("Query folder ids failed: {}", e),
        })?
        .filter_map(|row| row.ok())
        .collect();
    Ok(folder_ids)
}
