use crate::error::AppError;
use crate::llm::{LlmClient, LLM_REQUEST_TIMEOUT_SECS};
use crate::models::LlmConfig;
use async_trait::async_trait;
use std::time::Duration;

pub struct OpenAiLlmClient {
    client: reqwest::Client,
    config: LlmConfig,
}

impl OpenAiLlmClient {
    pub fn new(config: &LlmConfig) -> Result<Self, AppError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(LLM_REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(|e| AppError::Network {
                endpoint: String::new(),
                reason: e.to_string(),
            })?;
        Ok(Self {
            client,
            config: config.clone(),
        })
    }

    /// Send an HTTP request with one retry on 5xx server errors and network errors.
    async fn send_request(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<(u16, String), AppError> {
        let mut last_error = None;
        for attempt in 0..2u8 {
            if attempt > 0 {
                log::warn!(
                    "OpenAI LLM request failed, retrying (attempt {})...",
                    attempt + 1
                );
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
            match self.execute_request(url, body).await {
                Ok((status, body_text)) if status < 500 || attempt >= 1 => {
                    return Ok((status, body_text));
                }
                Ok((status, body_text)) => {
                    last_error = Some(AppError::Api {
                        status,
                        endpoint: url.to_string(),
                        body_preview: body_text.chars().take(500).collect(),
                    });
                }
                Err(e @ AppError::Network { .. }) => {
                    if attempt >= 1 {
                        return Err(e);
                    }
                    last_error = Some(e);
                }
                Err(e) => return Err(e),
            }
        }
        Err(last_error.unwrap())
    }

    async fn execute_request(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<(u16, String), AppError> {
        let response = self
            .client
            .post(url)
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key),
            )
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| AppError::Network {
                endpoint: url.to_string(),
                reason: e.to_string(),
            })?;

        let status = response.status().as_u16();
        let body_text = response.text().await.map_err(|e| AppError::Network {
            endpoint: url.to_string(),
            reason: e.to_string(),
        })?;

        Ok((status, body_text))
    }
}

#[async_trait]
impl LlmClient for OpenAiLlmClient {
    async fn chat(&self, system_prompt: &str, user_message: &str) -> Result<String, AppError> {
        let url = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );

        let body = serde_json::json!({
            "model": self.config.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_message}
            ]
        });

        let (status, body_text) = self.send_request(&url, &body).await?;

        if status >= 400 {
            return Err(AppError::Api {
                status,
                endpoint: url,
                body_preview: body_text.chars().take(500).collect(),
            });
        }

        let parsed: serde_json::Value =
            serde_json::from_str(&body_text).map_err(|_e| AppError::Api {
                status,
                endpoint: url.clone(),
                body_preview: body_text.chars().take(500).collect(),
            })?;

        parsed["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::Api {
                status: 200,
                endpoint: url,
                body_preview: body_text.chars().take(500).collect(),
            })
    }
}
