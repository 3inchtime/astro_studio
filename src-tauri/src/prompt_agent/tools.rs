use crate::prompt_agent::skills::{built_in_skills, skill_hints};
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum PromptAgentToolError {
    #[error("No matching prompt skills were found")]
    NoMatchingSkills,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListPromptSkillsArgs {}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApplyPromptSkillsArgs {
    pub skill_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SuggestGenerationParamsArgs {
    pub visual_goal: String,
    pub needs_transparency: bool,
    pub needs_multiple_variations: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ListPromptSkillsTool;

impl Tool for ListPromptSkillsTool {
    const NAME: &'static str = "list_prompt_skills";
    type Error = PromptAgentToolError;
    type Args = ListPromptSkillsArgs;
    type Output = serde_json::Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "List the available Astro Studio prompt skills and when to use each one."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok(json!({ "skills": built_in_skills() }))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApplyPromptSkillsTool;

impl Tool for ApplyPromptSkillsTool {
    const NAME: &'static str = "apply_prompt_skills";
    type Error = PromptAgentToolError;
    type Args = ApplyPromptSkillsArgs;
    type Output = serde_json::Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Return detailed prompt-writing instructions for selected prompt skills."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "skill_ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Prompt skill ids to apply"
                    }
                },
                "required": ["skill_ids"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let skills = skill_hints(&args.skill_ids);
        if skills.is_empty() {
            return Err(PromptAgentToolError::NoMatchingSkills);
        }
        Ok(json!({ "applied_skills": skills }))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SuggestGenerationParamsTool;

impl Tool for SuggestGenerationParamsTool {
    const NAME: &'static str = "suggest_generation_params";
    type Error = PromptAgentToolError;
    type Args = SuggestGenerationParamsArgs;
    type Output = serde_json::Value;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description:
                "Suggest safe Astro Studio image generation parameters based on the visual goal."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "visual_goal": { "type": "string" },
                    "needs_transparency": { "type": "boolean" },
                    "needs_multiple_variations": { "type": "boolean" }
                },
                "required": ["visual_goal", "needs_transparency", "needs_multiple_variations"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let background = if args.needs_transparency {
            "transparent"
        } else {
            "auto"
        };
        let image_count = if args.needs_multiple_variations { 2 } else { 1 };

        Ok(json!({
            "size": "auto",
            "quality": "auto",
            "background": background,
            "output_format": "png",
            "moderation": "auto",
            "image_count": image_count
        }))
    }
}
