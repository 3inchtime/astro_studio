use crate::storage::artifacts::{ArtifactColumn, ArtifactKind};
use crate::{db::Database, file_manager};
use rusqlite::{params, Connection};

fn trash_cutoff_timestamp(retention_days: u32) -> String {
    (chrono::Utc::now() - chrono::Duration::days(retention_days as i64))
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn permanently_delete_generation_files_and_records(
    app: &tauri::AppHandle,
    conn: &Connection,
    generation_id: &str,
) -> Result<(), String> {
    let fm =
        file_manager::FileManager::new(crate::storage::asset_root(app).map_err(|e| e.to_string())?);
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    crate::canvas_assets::ensure_generation_unreferenced(&tx, generation_id)?;
    let paths = {
        let mut stmt = tx
            .prepare("SELECT file_path, thumbnail_path FROM images WHERE generation_id=?1")
            .map_err(|e| e.to_string())?;
        let files = stmt
            .query_map([generation_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })
            .map_err(|e| e.to_string())?;
        files
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|e| e.to_string())?
    };
    // Foreign-key cascades remove the image rows together with the generation.
    // Leave every original intact if the database rejects the deletion.
    tx.execute("DELETE FROM generations WHERE id=?1", [generation_id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    for (file, thumbnail) in paths {
        if let Err(error) = fm.delete_image(&file, thumbnail.as_deref()) {
            log::warn!("Could not clean up deleted generation {generation_id}: {error}");
        }
    }

    Ok(())
}

/// Automatic cleanup is conservative: unreadable references also retain the asset.
pub(crate) fn purge_candidates(conn: &Connection, cutoff: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT id FROM generations WHERE deleted_at IS NOT NULL AND deleted_at <= ?1")
        .map_err(|e| e.to_string())?;
    let ids: Vec<String> = stmt
        .query_map(params![cutoff], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(ids
        .into_iter()
        .filter(|id| crate::canvas_assets::ensure_generation_unreferenced(conn, id).is_ok())
        .collect())
}

pub(crate) fn purge_trashed_generations(
    app: &tauri::AppHandle,
    db: &Database,
    retention_days: u32,
) -> Result<u64, String> {
    let cutoff = trash_cutoff_timestamp(retention_days);
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    let ids = purge_candidates(&conn, &cutoff)?;

    let mut deleted = 0;
    for id in &ids {
        permanently_delete_generation_files_and_records(app, &conn, id)?;
        deleted += 1;
    }

    Ok(deleted)
}

pub(crate) const GALLERY_ARTIFACTS: &[ArtifactColumn] = &[
    ArtifactColumn::new("images", "id", "file_path", ArtifactKind::File),
    ArtifactColumn::new("images", "id", "thumbnail_path", ArtifactKind::File),
];

#[tauri::command]
pub(crate) fn delete_generation(
    app: tauri::AppHandle,
    db: tauri::State<'_, Database>,
    id: String,
) -> Result<(), String> {
    crate::generation_execution::delete_with_jobs(
        &app,
        db.inner(),
        crate::generation_execution::DeletionScope::Generation(&id),
        |conn| {
            conn.execute(
                "UPDATE generations SET deleted_at=?1 WHERE id=?2 AND deleted_at IS NULL",
                params![crate::current_timestamp(), id],
            )?;
            Ok(())
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn restore_generation(db: tauri::State<'_, Database>, id: String) -> Result<(), String> {
    let _library_operation = crate::storage::write_operation::<String>()?;
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
    let _library_operation = crate::storage::write_operation::<String>()?;
    log::info!("Permanently deleting generation {}", id);
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    permanently_delete_generation_files_and_records(&app, &conn, &id)
}
