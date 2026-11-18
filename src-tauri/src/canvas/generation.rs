use super::documents::read_canvas_document;
use crate::{error::AppError, models::CanvasDocumentContent};
use rusqlite::{params, OptionalExtension};
use std::fs;

#[derive(serde::Serialize)]
pub(crate) struct CanvasGenerationStatus {
    generation_id: Option<String>,
    status: String,
    error: Option<String>,
    error_code: Option<String>,
    images: Vec<crate::models::GeneratedImage>,
}

// Resolve by the durable request identity, including when the frontend never
// received the submission response. Only expose jobs associated with this file.
pub(crate) fn read_canvas_generation_status(
    conn: &rusqlite::Connection,
    document_id: &str,
    request_id: &str,
) -> Result<Option<CanvasGenerationStatus>, AppError> {
    let document = read_canvas_document(conn, document_id)?;
    let bytes = fs::read(&document.document_path)?;
    let content: CanvasDocumentContent =
        serde_json::from_slice(&bytes).map_err(|error| AppError::Validation {
            message: format!("Read canvas request association failed: {error}"),
        })?;
    if !content.generation_requests.iter().any(|request| {
        request
            .get("request_id")
            .and_then(serde_json::Value::as_str)
            == Some(request_id)
    }) {
        return Err(AppError::Validation {
            message: "Generation request does not belong to this canvas.".to_string(),
        });
    }

    let job = conn
        .query_row(
            "SELECT j.generation_id, j.status, COALESCE(j.error_message, g.error_message),
                c.project_id, g.deleted_at, c.deleted_at, j.error_code
         FROM generation_jobs j
         JOIN generations g ON g.id = j.generation_id
         LEFT JOIN conversations c ON c.id = g.conversation_id
         WHERE j.client_request_id = ?1 AND (
             CASE WHEN json_type(j.source_ref_json, '$.request_fingerprint') IS NULL
                  THEN COALESCE(json_extract(j.source_ref_json,'$.document_id'),
                                json_extract(j.source_ref_json,'$.canvas_document_id'), ?2) = ?2
                  ELSE j.source_kind = 'canvas' AND json_extract(j.source_ref_json,'$.document_id') = ?2
             END)",
            params![request_id, document_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .optional()?;
    let Some((
        generation_id,
        status,
        error,
        project_id,
        deleted_at,
        conversation_deleted_at,
        error_code,
    )) = job
    else {
        return Ok(None);
    };
    if project_id.as_deref() != Some(document.project_id.as_str()) {
        return Err(AppError::Validation {
            message: "Generation result belongs to a different project.".to_string(),
        });
    }
    if deleted_at.is_some() || conversation_deleted_at.is_some() {
        return Ok(Some(CanvasGenerationStatus {
            generation_id: Some(generation_id),
            status: "failed".to_string(),
            error: Some("The generation result has been deleted.".to_string()),
            error_code: None,
            images: vec![],
        }));
    }
    let generation = conn.query_row(
        "SELECT * FROM generations WHERE id = ?1",
        [&generation_id],
        crate::collections::images::row_to_generation,
    )?;
    let mut results =
        crate::collections::images::generation_results_with_images(conn, vec![generation])?;
    let images = results
        .pop()
        .map(|result| result.images)
        .unwrap_or_default();
    Ok(Some(CanvasGenerationStatus {
        generation_id: Some(generation_id),
        status,
        error,
        error_code,
        images,
    }))
}

#[cfg(test)]
mod tests;
