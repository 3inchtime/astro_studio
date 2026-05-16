use crate::current_timestamp;
use crate::db::Database;
use crate::error::AppError;
use crate::models::{LlmConfig, SETTING_LLM_CONFIGS};
use crate::prompt_agent::runner::run_prompt_agent;
use crate::prompt_agent::types::{
    PromptAgentMessage, PromptAgentRunInput, PromptAgentSession, PromptAgentSuggestedParams,
    PromptAgentTurnResponse, SendPromptAgentMessageRequest, StartPromptAgentSessionRequest,
};
use crate::prompt_agent::{
    PROMPT_AGENT_STATUS_ACCEPTED, PROMPT_AGENT_STATUS_ACTIVE, PROMPT_AGENT_STATUS_CANCELLED,
};
use rusqlite::params;
use tauri::State;

#[tauri::command]
pub(crate) fn get_prompt_agent_health(
    _db: State<'_, Database>,
) -> Result<&'static str, AppError> {
    Ok("ok")
}

fn read_llm_configs(db: &Database) -> Result<Vec<LlmConfig>, AppError> {
    match db.get_setting(SETTING_LLM_CONFIGS)? {
        Some(json) => serde_json::from_str(&json).map_err(|e| AppError::Database {
            message: format!("Failed to deserialize LLM configs: {}", e),
        }),
        None => Ok(Vec::new()),
    }
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
        .prepare("SELECT * FROM prompt_agent_messages WHERE session_id = ?1 ORDER BY created_at ASC")
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
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO prompt_agent_messages (
            id, session_id, role, content, draft_prompt, selected_skill_ids,
            suggested_params, ready_to_generate, created_at
        ) VALUES (?1, ?2, 'user', ?3, NULL, '[]', '{}', 0, ?4)",
        params![uuid::Uuid::new_v4().to_string(), session_id, content, current_timestamp()],
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

async fn run_and_persist_agent_turn(
    db: &Database,
    config: &LlmConfig,
    session_id: &str,
    user_content: &str,
    source_image_count: usize,
) -> Result<PromptAgentTurnResponse, AppError> {
    let (conversation_summary, previous_draft_prompt) = {
        let conn = db.conn.lock().map_err(|e| AppError::Database {
            message: format!("Lock failed: {}", e),
        })?;
        insert_user_message(&conn, session_id, user_content)?;
        let session = load_session(&conn, session_id)?;
        let messages = load_messages(&conn, session_id)?;
        (summarize_messages(&messages), session.draft_prompt)
    };

    let decision = run_prompt_agent(
        config,
        PromptAgentRunInput {
            user_message: user_content.to_string(),
            conversation_summary,
            previous_draft_prompt,
            source_image_count,
        },
    )
    .await?;

    let suggested_params = PromptAgentSuggestedParams::from(decision.suggested_params);
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
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
    Ok(PromptAgentTurnResponse { session, messages })
}

#[tauri::command]
pub(crate) async fn start_prompt_agent_session(
    db: State<'_, Database>,
    request: StartPromptAgentSessionRequest,
) -> Result<PromptAgentTurnResponse, AppError> {
    let prompt = request.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err(AppError::Validation {
            message: "Enter an idea before starting deep thinking mode.".to_string(),
        });
    }

    let configs = read_llm_configs(db.inner())?;
    let config = resolve_agent_llm_config(&configs, &request.config_id)?.clone();
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

    run_and_persist_agent_turn(
        db.inner(),
        &config,
        &session_id,
        &prompt,
        request.source_image_paths.len(),
    )
    .await
}

#[tauri::command]
pub(crate) async fn send_prompt_agent_message(
    db: State<'_, Database>,
    request: SendPromptAgentMessageRequest,
) -> Result<PromptAgentTurnResponse, AppError> {
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

    run_and_persist_agent_turn(
        db.inner(),
        &config,
        &request.session_id,
        &message,
        request.source_image_paths.len(),
    )
    .await
}

#[tauri::command]
pub(crate) fn accept_prompt_agent_draft(
    db: State<'_, Database>,
    session_id: String,
    accepted_prompt: String,
) -> Result<PromptAgentSession, AppError> {
    let accepted_prompt = accepted_prompt.trim();
    if accepted_prompt.is_empty() {
        return Err(AppError::Validation {
            message: "Accepted prompt cannot be empty.".to_string(),
        });
    }

    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    conn.execute(
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
    load_session(&conn, &session_id)
}

#[tauri::command]
pub(crate) fn cancel_prompt_agent_session(
    db: State<'_, Database>,
    session_id: String,
) -> Result<PromptAgentSession, AppError> {
    let conn = db.conn.lock().map_err(|e| AppError::Database {
        message: format!("Lock failed: {}", e),
    })?;
    conn.execute(
        "UPDATE prompt_agent_sessions SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![PROMPT_AGENT_STATUS_CANCELLED, current_timestamp(), session_id],
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
