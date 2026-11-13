pub mod anthropic;
pub mod openai;

use crate::error::AppError;
use crate::models::LlmConfig;
use async_trait::async_trait;

pub const LLM_REQUEST_TIMEOUT_SECS: u64 = 30;
pub const MULTIMODAL_TIMEOUT_SECS: u64 = 60;

// Provider responses and transport diagnostics are untrusted: they may echo an
// API key, request headers or a credential-bearing URL. Only fixed descriptions
// and the origin cross the IPC/log boundary, including on malformed 2xx replies.
fn safe_endpoint(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https"))
        .map(|url| url.origin().ascii_serialization())
        .unwrap_or_default()
}

fn api_error(url: &str, status: u16) -> AppError {
    let message = match status {
        401 | 403 => {
            "Provider authentication failed. Check the configured credential and permissions."
        }
        404 => "Provider endpoint or model was not found. Check the service configuration.",
        429 => "Provider rate limit or quota was exceeded. Try again later.",
        500..=599 => "Provider service is unavailable. Try again later.",
        400..=499 => "Provider rejected the request. Check the model and request settings.",
        _ => "Provider returned an invalid response.",
    };
    AppError::Api {
        status,
        endpoint: safe_endpoint(url),
        body_preview: message.into(),
    }
}

fn network_error(url: &str, error: &reqwest::Error) -> AppError {
    let reason = if error.is_timeout() {
        "Provider request timed out."
    } else if error.is_connect() {
        "Could not connect to the provider."
    } else if error.is_builder() {
        "Invalid provider request configuration."
    } else {
        "Provider communication failed."
    };
    AppError::Network {
        endpoint: safe_endpoint(url),
        reason: reason.into(),
    }
}

pub struct ImageData {
    pub data: Vec<u8>,
    pub media_type: String,
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn chat(&self, system_prompt: &str, user_message: &str) -> Result<String, AppError>;

    async fn chat_with_images(
        &self,
        system_prompt: &str,
        user_message: &str,
        _images: &[ImageData],
    ) -> Result<String, AppError> {
        // Default: ignore images, delegate to text-only chat
        self.chat(system_prompt, user_message).await
    }
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

    fn error_fixture(status: u16, body: String) -> (String, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            loop {
                let mut buf = [0; 2048];
                let read = stream.read(&mut buf).unwrap();
                assert!(read > 0);
                request.extend_from_slice(&buf[..read]);
                if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]);
                    let length: usize = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse().unwrap())
                        })
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            write!(stream, "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        });
        (format!("http://url-user:url-password@{address}/private-path?token=query-secret#private-fragment"), server)
    }

    #[tokio::test]
    async fn provider_errors_never_expose_echoed_keys_or_private_urls() {
        for protocol in ["openai", "anthropic"] {
            for with_images in [false, true] {
                for status in [401, 200] {
                    let body = serde_json::json!({"error": "Incorrect API key: raw-provider-secret; escaped secret: \\u0073ecret; supplied URL password=url-password token=query-secret"}).to_string();
                    let (base_url, server) = error_fixture(status, body);
                    let mut config = test_config(protocol);
                    config.base_url = base_url;
                    config.api_key = "raw-provider-secret".into();
                    let client = create_llm_client(&config).unwrap();
                    let error = if with_images {
                        client
                            .chat_with_images(
                                "system",
                                "user",
                                &[ImageData {
                                    data: vec![1, 2, 3],
                                    media_type: "image/png".into(),
                                }],
                            )
                            .await
                            .unwrap_err()
                    } else {
                        client.chat("system", "user").await.unwrap_err()
                    };
                    server.join().unwrap();
                    let wire = serde_json::to_string(&error).unwrap();
                    for forbidden in [
                        "raw-provider-secret",
                        "url-user",
                        "url-password",
                        "query-secret",
                        "private-path",
                        "private-fragment",
                        "Incorrect API key",
                        "u0073ecret",
                    ] {
                        assert!(
                            !wire.contains(forbidden),
                            "{protocol} images={with_images} status={status}: {wire}"
                        );
                    }
                    assert!(
                        matches!(error, AppError::Api { status: actual, .. } if actual == status)
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn transport_errors_never_expose_private_urls() {
        for protocol in ["openai", "anthropic"] {
            let mut config = test_config(protocol);
            config.base_url =
                "http://url-user:url-password@127.0.0.1:1/private-path?token=query-secret".into();
            // Force a deterministic reqwest error before any network request,
            // independent of the machine's system proxy configuration.
            config.api_key = "invalid\nheader-secret".into();
            let client = create_llm_client(&config).unwrap();
            let error = client.chat("system", "user").await.unwrap_err();
            let wire = serde_json::to_string(&error).unwrap();
            for forbidden in [
                "url-user",
                "url-password",
                "query-secret",
                "private-path",
                "header-secret",
            ] {
                assert!(!wire.contains(forbidden), "{protocol}: {wire}");
            }
            assert!(matches!(error, AppError::Network { .. }));
        }
    }

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
