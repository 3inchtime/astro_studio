use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptAgentSuggestedParams {
    pub model: Option<String>,
    pub size: Option<String>,
    pub quality: Option<String>,
    pub background: Option<String>,
    pub output_format: Option<String>,
    pub moderation: Option<String>,
    pub image_count: Option<u8>,
}

impl Default for PromptAgentSuggestedParams {
    fn default() -> Self {
        Self {
            model: None,
            size: None,
            quality: None,
            background: None,
            output_format: None,
            moderation: None,
            image_count: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptAgentSession {
    pub id: String,
    pub conversation_id: Option<String>,
    pub project_id: Option<String>,
    pub status: String,
    pub original_prompt: String,
    pub draft_prompt: Option<String>,
    pub accepted_prompt: Option<String>,
    pub selected_skill_ids: Vec<String>,
    pub suggested_params: PromptAgentSuggestedParams,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptAgentMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub draft_prompt: Option<String>,
    pub selected_skill_ids: Vec<String>,
    pub suggested_params: PromptAgentSuggestedParams,
    pub ready_to_generate: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartPromptAgentSessionRequest {
    pub prompt: String,
    pub config_id: String,
    pub conversation_id: Option<String>,
    pub project_id: Option<String>,
    pub source_image_paths: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SendPromptAgentMessageRequest {
    pub session_id: String,
    pub message: String,
    pub config_id: String,
    pub source_image_paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromptAgentTurnResponse {
    pub session: PromptAgentSession,
    pub messages: Vec<PromptAgentMessage>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct PromptAgentDecision {
    pub reply: String,
    pub draft_prompt: Option<String>,
    pub selected_skill_ids: Vec<String>,
    pub suggested_params: PromptAgentDecisionParams,
    pub ready_to_generate: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct PromptAgentDecisionParams {
    pub model: Option<String>,
    pub size: Option<String>,
    pub quality: Option<String>,
    pub background: Option<String>,
    pub output_format: Option<String>,
    pub moderation: Option<String>,
    pub image_count: Option<u8>,
}

impl From<PromptAgentDecisionParams> for PromptAgentSuggestedParams {
    fn from(value: PromptAgentDecisionParams) -> Self {
        Self {
            model: value.model,
            size: value.size,
            quality: value.quality,
            background: value.background,
            output_format: value.output_format,
            moderation: value.moderation,
            image_count: value.image_count,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PromptAgentRunInput {
    pub user_message: String,
    pub conversation_summary: String,
    pub previous_draft_prompt: Option<String>,
    pub source_image_count: usize,
}
