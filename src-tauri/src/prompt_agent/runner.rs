use crate::error::AppError;
use crate::models::LlmConfig;
use crate::prompt_agent::tools::{
    ApplyPromptSkillsTool, ListPromptSkillsTool, SuggestGenerationParamsTool,
};
use crate::prompt_agent::types::{PromptAgentDecision, PromptAgentRunInput};
use rig::client::CompletionClient;
use rig::completion::TypedPrompt;
use rig::providers::{anthropic, openai};

fn prompt_agent_preamble() -> &'static str {
    "\
You are Astro Studio's deep-thinking prompt director for AI image generation.
Your job is to help the user develop one final image-generation prompt through dialogue.

Rules:
1. Preserve the user's core intent, subject, named entities, style constraints, and negative constraints.
2. Ask at most one focused follow-up question when important visual information is missing.
3. Use the prompt skill tools when they would improve the final prompt.
4. Prefer GPT Image 2 template skills for structured outputs such as UI mockups, product visuals, maps, infographics, posters, academic figures, technical diagrams, storyboards, branding, or editing workflows.
5. When using template skills, map user facts into concrete fields and ask one precise follow-up only if a missing field materially changes the result.
6. Keep replies concise and practical.
7. The draft_prompt must be directly usable for image generation when ready_to_generate is true.
8. Do not claim that an image has been generated.
9. Do not call image generation. The user must accept the prompt first.
10. Match the user's language unless they explicitly ask for another language.
11. Return selected_skill_ids using only ids from the tool results.
12. Use suggested_params only when the visual goal implies a clear setting; otherwise leave fields null."
}

fn build_user_prompt(input: &PromptAgentRunInput) -> String {
    format!(
        "\
Current user message:
{user_message}

Conversation summary:
{conversation_summary}

Previous draft prompt:
{previous_draft}

Reference/source image count:
{source_image_count}

Respond with a typed decision for the next assistant turn.",
        user_message = input.user_message,
        conversation_summary = input.conversation_summary,
        previous_draft = input.previous_draft_prompt.as_deref().unwrap_or("None"),
        source_image_count = input.source_image_count,
    )
}

async fn run_openai_prompt_agent(
    config: &LlmConfig,
    input: PromptAgentRunInput,
) -> Result<PromptAgentDecision, AppError> {
    let mut builder = openai::Client::builder().api_key(&config.api_key);
    if !config.base_url.trim().is_empty() {
        builder = builder.base_url(config.base_url.trim_end_matches('/'));
    }
    let client = builder.build().map_err(|e| AppError::Validation {
        message: format!("Create Rig OpenAI client failed: {}", e),
    })?;

    let agent = client
        .agent(config.model.as_str())
        .name("astro_prompt_director")
        .description("Develops final image-generation prompts through local prompt skills.")
        .preamble(prompt_agent_preamble())
        .tool(ListPromptSkillsTool)
        .tool(ApplyPromptSkillsTool)
        .tool(SuggestGenerationParamsTool)
        .temperature(0.4)
        .max_tokens(1600)
        .default_max_turns(crate::prompt_agent::PROMPT_AGENT_MAX_TURNS)
        .build();

    agent
        .prompt_typed::<PromptAgentDecision>(build_user_prompt(&input))
        .max_turns(crate::prompt_agent::PROMPT_AGENT_MAX_TURNS)
        .await
        .map_err(|e| AppError::Validation {
            message: format!("Prompt agent failed: {}", e),
        })
}

async fn run_anthropic_prompt_agent(
    config: &LlmConfig,
    input: PromptAgentRunInput,
) -> Result<PromptAgentDecision, AppError> {
    let mut builder = anthropic::Client::builder().api_key(config.api_key.clone());
    if !config.base_url.trim().is_empty() {
        builder = builder.base_url(config.base_url.trim_end_matches('/'));
    }
    let client = builder.build().map_err(|e| AppError::Validation {
        message: format!("Create Rig Anthropic client failed: {}", e),
    })?;

    let agent = client
        .agent(config.model.as_str())
        .name("astro_prompt_director")
        .description("Develops final image-generation prompts through local prompt skills.")
        .preamble(prompt_agent_preamble())
        .tool(ListPromptSkillsTool)
        .tool(ApplyPromptSkillsTool)
        .tool(SuggestGenerationParamsTool)
        .temperature(0.4)
        .max_tokens(1600)
        .default_max_turns(crate::prompt_agent::PROMPT_AGENT_MAX_TURNS)
        .build();

    agent
        .prompt_typed::<PromptAgentDecision>(build_user_prompt(&input))
        .max_turns(crate::prompt_agent::PROMPT_AGENT_MAX_TURNS)
        .await
        .map_err(|e| AppError::Validation {
            message: format!("Prompt agent failed: {}", e),
        })
}

pub async fn run_prompt_agent(
    config: &LlmConfig,
    input: PromptAgentRunInput,
) -> Result<PromptAgentDecision, AppError> {
    match config.protocol.as_str() {
        "openai" => run_openai_prompt_agent(config, input).await,
        "anthropic" => run_anthropic_prompt_agent(config, input).await,
        other => Err(AppError::Validation {
            message: format!("Prompt agent does not support LLM protocol '{}'", other),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preamble_blocks_direct_image_generation() {
        assert!(prompt_agent_preamble().contains("Do not call image generation"));
        assert!(prompt_agent_preamble().contains("accept the prompt"));
    }

    #[test]
    fn user_prompt_includes_source_image_count() {
        let prompt = build_user_prompt(&PromptAgentRunInput {
            user_message: "Create a cinematic cabin".to_string(),
            conversation_summary: "No prior turns".to_string(),
            previous_draft_prompt: None,
            source_image_count: 2,
        });
        assert!(prompt.contains("Create a cinematic cabin"));
        assert!(prompt.contains("2"));
    }
}
