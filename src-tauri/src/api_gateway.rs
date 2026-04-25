use crate::config::ApiConfig;
use crate::models::*;

#[async_trait::async_trait]
pub trait ImageEngine: Send + Sync {
    async fn generate(
        &self,
        api_key: &str,
        base_url: &str,
        prompt: &str,
        size: &str,
        quality: &str,
    ) -> Result<Vec<Vec<u8>>, String>;
    fn name(&self) -> &str;
}

pub struct GptImageEngine {
    client: reqwest::Client,
}

impl GptImageEngine {
    pub fn new(config: &ApiConfig) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(config.timeout_secs))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }
}

#[async_trait::async_trait]
impl ImageEngine for GptImageEngine {
    async fn generate(
        &self,
        api_key: &str,
        base_url: &str,
        prompt: &str,
        size: &str,
        quality: &str,
    ) -> Result<Vec<Vec<u8>>, String> {
        let url = format!("{}/images/generations", base_url.trim_end_matches('/'));

        if api_key.is_empty() {
            return Err("API key not set".to_string());
        }

        let request_body = serde_json::json!({
            "model": ENGINE_GPT_IMAGE_2,
            "prompt": prompt,
            "n": 1,
            "size": size,
            "quality": quality,
        });

        log::info!("Sending image generation request to {} — model: {}, size: {}, quality: {}", url, ENGINE_GPT_IMAGE_2, size, quality);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let body_text = response
            .text()
            .await
            .map_err(|e| format!("Read body failed: {}", e))?;

        if !status.is_success() {
            log::error!("API error {} — response: {}", status, &body_text[..body_text.len().min(500)]);
            return Err(format!("API error {}: {}", status, body_text));
        }

        log::info!("API responded {} ({} bytes)", status, body_text.len());

        let api_response: OpenAiImageResponse = serde_json::from_str(&body_text)
            .map_err(|e| format!("Parse response failed: {}. Body: {}", e, &body_text[..body_text.len().min(300)]))?;

        let mut images = Vec::new();
        for data in &api_response.data {
            if let Some(b64) = &data.b64_json {
                let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
                    .map_err(|e| format!("Base64 decode failed: {}", e))?;
                images.push(bytes);
            }
        }

        if images.is_empty() {
            return Err(format!("No images returned. Response: {}", &body_text[..body_text.len().min(500)]));
        }

        log::info!("Decoded {} image(s) from response", images.len());
        Ok(images)
    }

    fn name(&self) -> &str {
        #[allow(dead_code)]
        ENGINE_GPT_IMAGE_2
    }
}
