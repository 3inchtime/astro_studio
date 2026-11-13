use crate::current_timestamp;
use crate::db::Database;
use crate::error::AppError;
use crate::models::LlmConfig;
use crate::prompt_agent::runner::run_prompt_agent;
use crate::prompt_agent::types::{
    PromptAgentMessage, PromptAgentRunInput, PromptAgentSession, PromptAgentSuggestedParams,
    PromptAgentTurnResponse, SendPromptAgentMessageRequest, StartPromptAgentSessionRequest,
};
use crate::prompt_agent::vision::{self, VisualContext};
use crate::prompt_agent::{
    PROMPT_AGENT_STATUS_ACCEPTED, PROMPT_AGENT_STATUS_ACTIVE, PROMPT_AGENT_STATUS_CANCELLED,
};
use rusqlite::params;
use tauri::State;

#[tauri::command]
pub(crate) fn get_prompt_agent_health(_db: State<'_, Database>) -> Result<&'static str, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    Ok("ok")
}

fn read_llm_configs(db: &Database) -> Result<Vec<LlmConfig>, AppError> {
    crate::commands::llm::read_llm_configs(db)
}

fn resolve_agent_llm_config<'a>(
    configs: &'a [LlmConfig],
    config_id: &str,
) -> Result<&'a LlmConfig, AppError> {
    let config = configs
        .iter()
        .find(|config| config.id == config_id && config.enabled)
        .ok_or_else(|| AppError::Validation {
            message: "Select an enabled LLM config before using deep thinking mode.".to_string(),
        })?;

    if config.capability != "text" && config.capability != "multimodal" {
        return Err(AppError::Validation {
            message: format!("LLM config '{}' cannot run the prompt agent.", config.name),
        });
    }

    Ok(config)
}

fn serialize_skill_ids(skill_ids: &[String]) -> Result<String, AppError> {
    serde_json::to_string(skill_ids).map_err(|e| AppError::Database {
        message: format!("Serialize prompt agent skill ids failed: {}", e),
    })
}

fn deserialize_skill_ids(value: String) -> Vec<String> {
    serde_json::from_str(&value).unwrap_or_default()
}

fn serialize_suggested_params(params: &PromptAgentSuggestedParams) -> Result<String, AppError> {
    serde_json::to_string(params).map_err(|e| AppError::Database {
        message: format!("Serialize prompt agent params failed: {}", e),
    })
}

fn deserialize_suggested_params(value: String) -> PromptAgentSuggestedParams {
    serde_json::from_str(&value).unwrap_or_default()
}

fn row_to_session(row: &rusqlite::Row) -> rusqlite::Result<PromptAgentSession> {
    let selected_skill_ids_json: String = row.get("selected_skill_ids")?;
    let suggested_params_json: String = row.get("suggested_params")?;
    Ok(PromptAgentSession {
        id: row.get("id")?,
        conversation_id: row.get("conversation_id")?,
        project_id: row.get("project_id")?,
        status: row.get("status")?,
        original_prompt: row.get("original_prompt")?,
        draft_prompt: row.get("draft_prompt")?,
        accepted_prompt: row.get("accepted_prompt")?,
        selected_skill_ids: deserialize_skill_ids(selected_skill_ids_json),
        suggested_params: deserialize_suggested_params(suggested_params_json),
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn row_to_message(row: &rusqlite::Row) -> rusqlite::Result<PromptAgentMessage> {
    let selected_skill_ids_json: String = row.get("selected_skill_ids")?;
    let suggested_params_json: String = row.get("suggested_params")?;
    let ready_to_generate: i64 = row.get("ready_to_generate")?;
    Ok(PromptAgentMessage {
        id: row.get("id")?,
        session_id: row.get("session_id")?,
        role: row.get("role")?,
        content: row.get("content")?,
        draft_prompt: row.get("draft_prompt")?,
        selected_skill_ids: deserialize_skill_ids(selected_skill_ids_json),
        suggested_params: deserialize_suggested_params(suggested_params_json),
        ready_to_generate: ready_to_generate != 0,
        visual_context: row
            .get::<_, Option<String>>("visual_context_json")?
            .map(|raw| {
                serde_json::from_str(&raw).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })
            })
            .transpose()?,
        created_at: row.get("created_at")?,
    })
}

fn load_session(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<PromptAgentSession, AppError> {
    conn.query_row(
        "SELECT * FROM prompt_agent_sessions WHERE id = ?1",
        params![session_id],
        row_to_session,
    )
    .map_err(|e| AppError::Database {
        message: format!("Load prompt agent session failed: {}", e),
    })
}

fn load_messages(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Vec<PromptAgentMessage>, AppError> {
    let mut stmt = conn
        .prepare(
            "SELECT * FROM prompt_agent_messages WHERE session_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(|e| AppError::Database {
            message: format!("Prepare prompt agent message query failed: {}", e),
        })?;
    let rows = stmt
        .query_map(params![session_id], row_to_message)
        .map_err(|e| AppError::Database {
            message: format!("Query prompt agent messages failed: {}", e),
        })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| AppError::Database {
            message: format!("Read prompt agent messages failed: {}", e),
        })
}

fn insert_user_message(
    conn: &rusqlite::Connection,
    session_id: &str,
    content: &str,
    visual_context: Option<&VisualContext>,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO prompt_agent_messages (
            id, session_id, role, content, draft_prompt, selected_skill_ids,
            suggested_params, ready_to_generate, created_at, visual_context_json
        ) VALUES (?1, ?2, 'user', ?3, NULL, '[]', '{}', 0, ?4, ?5)",
        params![
            uuid::Uuid::new_v4().to_string(),
            session_id,
            content,
            current_timestamp(),
            visual_context
                .map(serde_json::to_string)
                .transpose()
                .map_err(|_| AppError::GenerationJobInvalidSnapshot)?
        ],
    )
    .map_err(|e| AppError::Database {
        message: format!("Insert prompt agent user message failed: {}", e),
    })?;
    Ok(())
}

fn update_session_from_assistant(
    conn: &rusqlite::Connection,
    session_id: &str,
    draft_prompt: Option<&str>,
    selected_skill_ids: &[String],
    suggested_params: &PromptAgentSuggestedParams,
) -> Result<(), AppError> {
    conn.execute(
        "UPDATE prompt_agent_sessions
         SET draft_prompt = COALESCE(?1, draft_prompt),
             selected_skill_ids = ?2,
             suggested_params = ?3,
             updated_at = ?4
         WHERE id = ?5",
        params![
            draft_prompt,
            serialize_skill_ids(selected_skill_ids)?,
            serialize_suggested_params(suggested_params)?,
            current_timestamp(),
            session_id
        ],
    )
    .map_err(|e| AppError::Database {
        message: format!("Update prompt agent session failed: {}", e),
    })?;
    Ok(())
}

fn insert_assistant_message(
    conn: &rusqlite::Connection,
    session_id: &str,
    content: &str,
    draft_prompt: Option<&str>,
    selected_skill_ids: &[String],
    suggested_params: &PromptAgentSuggestedParams,
    ready_to_generate: bool,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO prompt_agent_messages (
            id, session_id, role, content, draft_prompt, selected_skill_ids,
            suggested_params, ready_to_generate, created_at
        ) VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            uuid::Uuid::new_v4().to_string(),
            session_id,
            content,
            draft_prompt,
            serialize_skill_ids(selected_skill_ids)?,
            serialize_suggested_params(suggested_params)?,
            if ready_to_generate { 1 } else { 0 },
            current_timestamp()
        ],
    )
    .map_err(|e| AppError::Database {
        message: format!("Insert prompt agent assistant message failed: {}", e),
    })?;
    Ok(())
}

fn summarize_messages(messages: &[PromptAgentMessage]) -> String {
    if messages.is_empty() {
        return "No prior turns".to_string();
    }

    messages
        .iter()
        .map(|message| format!("{}: {}", message.role, message.content))
        .collect::<Vec<_>>()
        .join("\n")
}

#[allow(clippy::too_many_arguments)]
async fn prepare_visual_context(
    app: &tauri::AppHandle,
    db: &Database,
    registry: &crate::commands::generation::SelectedImageRegistry,
    config: &LlmConfig,
    sources: &[String],
    selection: Option<&[String]>,
    text_only: bool,
    user_request: &str,
) -> Result<Option<VisualContext>, AppError> {
    let selected = vision::selected_visual_paths(sources, selection, text_only)?;
    if text_only {
        return Ok(Some(VisualContext {
            version: 1,
            analysis_model: String::new(),
            analysis_protocol: String::new(),
            image_hashes: vec![],
            source_image_indices: vec![],
            observations: vec![],
            preserve: vec![],
            changes: vec![],
            user_request: user_request.into(),
            text_only: true,
        }));
    }
    if selected.is_empty() {
        return Ok(None);
    }
    if config.capability != "multimodal" {
        return Err(vision::vision_error("prompt_vision_multimodal_required"));
    }
    let images = crate::commands::llm::load_registered_images(app, db, registry, &selected)
        .map_err(|_| vision::vision_error("prompt_vision_image_unavailable"))?;
    let client = crate::commands::llm::create_multimodal_llm_client(config)?;
    vision::analyze(
        client.as_ref(),
        &images,
        &config.model,
        &config.protocol,
        user_request,
        &selected
            .iter()
            .map(|path| {
                sources
                    .iter()
                    .position(|source| source == path)
                    .expect("validated subset")
                    + 1
            })
            .collect::<Vec<_>>(),
    )
    .await
    .map(Some)
}

async fn run_and_persist_agent_turn(
    db: &Database,
    config: &LlmConfig,
    session_id: &str,
    user_content: &str,
    source_image_count: usize,
    visual_context: Option<VisualContext>,
) -> Result<PromptAgentTurnResponse, AppError> {
    let (conversation_summary, previous_draft_prompt, previous_message_count) = {
        let conn = db.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        let session = load_session(&conn, session_id)?;
        if session.status != PROMPT_AGENT_STATUS_ACTIVE {
            return Err(AppError::Validation {
                message: "This prompt agent session is no longer active.".into(),
            });
        }
        let messages = load_messages(&conn, session_id)?;
        (
            summarize_messages(&messages),
            session.draft_prompt,
            messages.len(),
        )
    };

    let decision = run_prompt_agent(
        config,
        PromptAgentRunInput {
            user_message: user_content.to_string(),
            conversation_summary,
            previous_draft_prompt,
            source_image_count,
            visual_context: visual_context.clone(),
        },
    )
    .await?;

    let suggested_params = PromptAgentSuggestedParams::from(decision.suggested_params);
    let mut connection = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let conn = connection.transaction()?;
    let session = load_session(&conn, session_id)?;
    if session.status != PROMPT_AGENT_STATUS_ACTIVE {
        return Err(AppError::Validation {
            message: "This prompt agent session is no longer active.".into(),
        });
    }
    if load_messages(&conn, session_id)?.len() != previous_message_count {
        return Err(AppError::Validation {
            message: "This prompt agent session changed while thinking. Reload before retrying."
                .into(),
        });
    }
    insert_user_message(&conn, session_id, user_content, visual_context.as_ref())?;
    update_session_from_assistant(
        &conn,
        session_id,
        decision.draft_prompt.as_deref(),
        &decision.selected_skill_ids,
        &suggested_params,
    )?;
    insert_assistant_message(
        &conn,
        session_id,
        &decision.reply,
        decision.draft_prompt.as_deref(),
        &decision.selected_skill_ids,
        &suggested_params,
        decision.ready_to_generate,
    )?;
    let session = load_session(&conn, session_id)?;
    let messages = load_messages(&conn, session_id)?;
    conn.commit()?;
    Ok(PromptAgentTurnResponse { session, messages })
}

#[tauri::command]
pub(crate) async fn start_prompt_agent_session(
    app: tauri::AppHandle,
    selected_images: State<'_, crate::commands::generation::SelectedImageRegistry>,
    db: State<'_, Database>,
    request: StartPromptAgentSessionRequest,
) -> Result<PromptAgentTurnResponse, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    let prompt = request.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(AppError::Validation {
            message: "Enter an idea before starting deep thinking mode.".to_string(),
        });
    }

    let configs = read_llm_configs(db.inner())?;
    let config = resolve_agent_llm_config(&configs, &request.config_id)?.clone();
    let visual_context = prepare_visual_context(
        &app,
        db.inner(),
        selected_images.inner(),
        &config,
        &request.source_image_paths,
        request.visual_source_image_paths.as_deref(),
        request.text_only,
        &prompt,
    )
    .await?;
    let session_id = uuid::Uuid::new_v4().to_string();
    let timestamp = current_timestamp();

    {
        let conn = db.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        conn.execute(
            "INSERT INTO prompt_agent_sessions (
                id, conversation_id, project_id, status, original_prompt, draft_prompt,
                accepted_prompt, selected_skill_ids, suggested_params, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, '[]', '{}', ?6, ?6)",
            params![
                session_id,
                request.conversation_id,
                request.project_id,
                PROMPT_AGENT_STATUS_ACTIVE,
                prompt,
                timestamp
            ],
        )
        .map_err(|e| AppError::Database {
            message: format!("Create prompt agent session failed: {}", e),
        })?;
    }

    let result = run_and_persist_agent_turn(
        db.inner(),
        &config,
        &session_id,
        &prompt,
        request.source_image_paths.len(),
        visual_context,
    )
    .await;
    if result.is_err() {
        // The caller has no session ID on a failed first turn. Remove only an empty session.
        if let Ok(conn) = db.conn.lock() {
            let _ = conn.execute("DELETE FROM prompt_agent_sessions WHERE id=?1 AND NOT EXISTS (SELECT 1 FROM prompt_agent_messages WHERE session_id=?1)", [&session_id]);
        }
    }
    result
}

#[tauri::command]
pub(crate) async fn send_prompt_agent_message(
    app: tauri::AppHandle,
    selected_images: State<'_, crate::commands::generation::SelectedImageRegistry>,
    db: State<'_, Database>,
    request: SendPromptAgentMessageRequest,
) -> Result<PromptAgentTurnResponse, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    let message = request.message.trim().to_string();
    if message.is_empty() {
        return Err(AppError::Validation {
            message: "Enter a reply before continuing deep thinking mode.".to_string(),
        });
    }

    let configs = read_llm_configs(db.inner())?;
    let config = resolve_agent_llm_config(&configs, &request.config_id)?.clone();

    {
        let conn = db.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        let session = load_session(&conn, &request.session_id)?;
        if session.status != PROMPT_AGENT_STATUS_ACTIVE {
            return Err(AppError::Validation {
                message: "This prompt agent session is no longer active.".to_string(),
            });
        }
    }

    let visual_context = prepare_visual_context(
        &app,
        db.inner(),
        selected_images.inner(),
        &config,
        &request.source_image_paths,
        request.visual_source_image_paths.as_deref(),
        request.text_only,
        &message,
    )
    .await?;

    run_and_persist_agent_turn(
        db.inner(),
        &config,
        &request.session_id,
        &message,
        request.source_image_paths.len(),
        visual_context,
    )
    .await
}

#[tauri::command]
pub(crate) fn accept_prompt_agent_draft(
    db: State<'_, Database>,
    session_id: String,
    accepted_prompt: String,
) -> Result<PromptAgentSession, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    let accepted_prompt = accepted_prompt.trim();
    if accepted_prompt.is_empty() {
        return Err(AppError::Validation {
            message: "Accepted prompt cannot be empty.".to_string(),
        });
    }

    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let changed = conn
        .execute(
            "UPDATE prompt_agent_sessions
         SET status = ?1, accepted_prompt = ?2, draft_prompt = ?2, updated_at = ?3
         WHERE id = ?4 AND status = ?5",
            params![
                PROMPT_AGENT_STATUS_ACCEPTED,
                accepted_prompt,
                current_timestamp(),
                session_id,
                PROMPT_AGENT_STATUS_ACTIVE
            ],
        )
        .map_err(|e| AppError::Database {
            message: format!("Accept prompt agent draft failed: {}", e),
        })?;
    if changed != 1 {
        return Err(AppError::Validation {
            message: "This prompt agent session is no longer active.".into(),
        });
    }
    load_session(&conn, &session_id)
}

#[tauri::command]
pub(crate) fn cancel_prompt_agent_session(
    db: State<'_, Database>,
    session_id: String,
) -> Result<PromptAgentSession, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    conn.execute(
        "UPDATE prompt_agent_sessions SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![
            PROMPT_AGENT_STATUS_CANCELLED,
            current_timestamp(),
            session_id
        ],
    )
    .map_err(|e| AppError::Database {
        message: format!("Cancel prompt agent session failed: {}", e),
    })?;
    load_session(&conn, &session_id)
}

#[tauri::command]
pub(crate) fn get_prompt_agent_session(
    db: State<'_, Database>,
    session_id: String,
) -> Result<PromptAgentTurnResponse, AppError> {
    let _library_operation = crate::storage::write_operation::<AppError>()?;
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    let session = load_session(&conn, &session_id)?;
    let messages = load_messages(&conn, &session_id)?;
    Ok(PromptAgentTurnResponse { session, messages })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn finished_session_never_starts_director_after_vision_returns() {
        let dir =
            std::env::temp_dir().join(format!("astro-vision-status-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::open(&dir.join("test.db")).unwrap();
        db.run_migrations().unwrap();
        // An unsupported protocol is a sentinel: if the director is reached its error differs.
        let config = LlmConfig {
            id: "test".into(),
            name: "test".into(),
            protocol: "must-not-run".into(),
            model: "fixture".into(),
            api_key: String::new(),
            base_url: String::new(),
            capability: "multimodal".into(),
            enabled: true,
        };
        for status in [PROMPT_AGENT_STATUS_ACCEPTED, PROMPT_AGENT_STATUS_CANCELLED] {
            db.conn.lock().unwrap().execute("INSERT OR REPLACE INTO prompt_agent_sessions (id,status,original_prompt,selected_skill_ids,suggested_params,created_at,updated_at) VALUES ('s',?1,'test','[]','{}','now','now')", [status]).unwrap();
            let error = run_and_persist_agent_turn(&db, &config, "s", "change lighting", 1, None)
                .await
                .unwrap_err();
            assert!(error.to_string().contains("no longer active"));
            assert!(load_messages(&db.conn.lock().unwrap(), "s")
                .unwrap()
                .is_empty());
        }
        drop(db);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn visual_context_round_trips_without_raw_files_or_secrets() {
        let dir = std::env::temp_dir().join(format!("astro-vision-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::open(&dir.join("test.db")).unwrap();
        db.run_migrations().unwrap();
        let conn = db.conn.lock().unwrap();
        conn.execute("INSERT INTO prompt_agent_sessions (id,status,original_prompt,selected_skill_ids,suggested_params,created_at,updated_at) VALUES ('s','active','test','[]','{}','now','now')", []).unwrap();
        let context = VisualContext {
            version: 1,
            analysis_model: "vision-model".into(),
            analysis_protocol: "openai".into(),
            image_hashes: vec!["a".repeat(64)],
            source_image_indices: vec![2],
            observations: vec!["Image 1 is red".into()],
            preserve: vec!["shape".into()],
            changes: vec!["lighting".into()],
            user_request: "keep shape".into(),
            text_only: false,
        };
        insert_user_message(&conn, "s", "keep shape", Some(&context)).unwrap();
        let messages = load_messages(&conn, "s").unwrap();
        assert_eq!(messages[0].visual_context.as_ref(), Some(&context));
        let raw = serde_json::to_string(&messages).unwrap();
        assert!(!raw.contains("api_key"));
        assert!(!raw.contains("/tmp/"));
        drop(conn);
        drop(db);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn serializes_and_deserializes_skill_ids() {
        let ids = vec!["photography".to_string(), "composition".to_string()];
        let json = serialize_skill_ids(&ids).expect("skill ids should serialize");
        assert_eq!(deserialize_skill_ids(json), ids);
    }

    #[test]
    fn invalid_params_json_uses_default() {
        let params = deserialize_suggested_params("{".to_string());
        assert_eq!(params, PromptAgentSuggestedParams::default());
    }
}
