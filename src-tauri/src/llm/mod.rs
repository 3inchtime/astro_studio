mod anthropic;
mod openai;

use crate::error::AppError;
use crate::models::LlmConfig;
use async_trait::async_trait;

pub const LLM_REQUEST_TIMEOUT_SECS: u64 = 30;

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(&self, system_prompt: &str, user_message: &str) -> Result<String, AppError>;
}

pub fn create_llm_client(config: &LlmConfig) -> Result<Box<dyn LlmClient>, AppError> {
    match config.protocol.as_str() {
        "openai" => {
            let client = openai::OpenAiLlmClient::new(config)?;
            Ok(Box::new(client))
        }
        "anthropic" => {
            let client = anthropic::AnthropicLlmClient::new(config)?;
            Ok(Box::new(client))
        }
        other => Err(AppError::Validation {
            message: format!("Unknown LLM protocol: {}", other),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::LlmConfig;

    fn test_config(protocol: &str) -> LlmConfig {
        LlmConfig {
            id: "test-id".to_string(),
            name: "Test Model".to_string(),
            protocol: protocol.to_string(),
            model: "test-model".to_string(),
            api_key: "test-key".to_string(),
            base_url: "https://api.example.com".to_string(),
            capability: "text".to_string(),
            enabled: true,
        }
    }

    #[test]
    fn llm_config_json_round_trip() {
        let config = LlmConfig {
            id: uuid::Uuid::new_v4().to_string(),
            name: "GPT-4o".to_string(),
            protocol: "openai".to_string(),
            model: "gpt-4o".to_string(),
            api_key: "sk-test-key".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            capability: "text".to_string(),
            enabled: true,
        };

        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: LlmConfig = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(config, deserialized);
    }

    #[test]
    fn create_llm_client_openai_protocol() {
        let config = test_config("openai");
        let result = create_llm_client(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn create_llm_client_anthropic_protocol() {
        let config = test_config("anthropic");
        let result = create_llm_client(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn create_llm_client_unknown_protocol() {
        let config = test_config("unknown");
        let result = create_llm_client(&config);
        assert!(result.is_err());
        match result {
            Err(AppError::Validation { message }) => {
                assert!(message.contains("Unknown LLM protocol"));
            }
            _ => panic!("Expected Validation error"),
        }
    }
}
