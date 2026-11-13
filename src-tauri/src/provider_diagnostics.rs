use crate::{db::Database, error::AppError, model_registry::*, models::*};
use reqwest::{Method, Url};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticCheck {
    pub status: &'static str,
    pub code: &'static str,
}
#[derive(Debug, Serialize)]
pub struct ProviderDiagnosticResult {
    pub local: DiagnosticCheck,
    pub transport: DiagnosticCheck,
    pub authentication: DiagnosticCheck,
    pub model: DiagnosticCheck,
    pub generation: DiagnosticCheck,
}
fn check(status: &'static str, code: &'static str) -> DiagnosticCheck {
    DiagnosticCheck { status, code }
}
fn validation(message: &str) -> AppError {
    AppError::Validation {
        message: message.into(),
    }
}
fn saved_profile(db: &Database, model: &str, id: &str) -> Result<ModelProviderProfile, AppError> {
    if normalize_image_model(model) != model {
        return Err(validation("Unknown image model."));
    }
    crate::commands::settings::read_model_provider_profiles_state(db, model)?
        .profiles
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| validation("Saved provider no longer exists."))
}
fn validated_url(value: &str) -> Result<Url, AppError> {
    let url = Url::parse(value).map_err(|_| validation("Invalid provider URL."))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(validation(
            "Use an HTTP or HTTPS URL without embedded credentials or fragments.",
        ));
    }
    Ok(url)
}
fn probe_target(model: &str, profile: &ModelProviderProfile) -> Result<(Method, Url), AppError> {
    if profile.api_key.trim().is_empty() {
        return Err(validation("Save an API key before testing."));
    }
    let settings = &profile.endpoint_settings;
    let generation =
        image_endpoint_url_for_model_settings(model, settings, ImageEndpointKind::Generate);
    validated_url(&generation)?;
    if settings.mode == ENDPOINT_MODE_FULL_URL || is_seedream_model(model) {
        return Ok((Method::HEAD, validated_url(&generation)?));
    }
    let url = if is_gemini_model(model) && model != ENGINE_NANO_BANANA_21 {
        generation.trim_end_matches(":generateContent").to_string()
    } else {
        format!("{}/models/{model}", settings.base_url.trim_end_matches('/'))
    };
    Ok((Method::GET, validated_url(&url)?))
}
async fn probe_profile(
    model: &str,
    profile: &ModelProviderProfile,
    timeout: Duration,
) -> ProviderDiagnosticResult {
    let mut result = ProviderDiagnosticResult {
        local: check("passed", "valid"),
        transport: check("unverified", "not_checked"),
        authentication: check("unverified", "not_checked"),
        model: check("unverified", "not_checked"),
        generation: check("unverified", "not_generated"),
    };
    let (method, url) = match probe_target(model, profile) {
        Ok(target) => target,
        Err(_) => {
            result.local = check("failed", "invalid_config");
            return result;
        }
    };
    // Never forward credentials through a redirect, including same-origin redirects.
    let client = match reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(5))
        .timeout(timeout)
        .build()
    {
        Ok(client) => client,
        Err(_) => {
            result.transport = check("failed", "client_error");
            return result;
        }
    };
    let head_only = method == Method::HEAD;
    let mut request = client.request(method, url.clone());
    if !head_only {
        request = if is_gemini_model(model) {
            request.header("x-goog-api-key", &profile.api_key)
        } else {
            request.bearer_auth(&profile.api_key)
        };
    }
    let mut response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            result.transport = check(
                "failed",
                if error.is_timeout() {
                    "timeout"
                } else {
                    "connection_failed"
                },
            );
            return result;
        }
    };
    result.transport = check("passed", "reachable");
    if head_only {
        result.authentication.code = "head_only";
        result.model.code = "head_only";
        return result;
    }
    let status = response.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        result.authentication = check("failed", "auth_rejected");
        return result;
    }
    if !status.is_success() {
        let code = match status.as_u16() {
            429 => "rate_limited",
            404 => "model_endpoint_missing",
            300..=399 => "redirect_blocked",
            _ => "provider_error",
        };
        result.authentication.code = code;
        result.model.code = code;
        return result;
    }
    // A successful HTML gateway page is not evidence that a model exists.
    let mut body = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) if body.len() + chunk.len() <= 1024 * 1024 => {
                body.extend_from_slice(&chunk)
            }
            Ok(None) => break,
            _ => {
                result.model.code = "unrecognized_response";
                return result;
            }
        }
    }
    let body: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(body) => body,
        Err(_) => {
            result.model.code = "unrecognized_response";
            return result;
        }
    };
    let expected = url.path().rsplit('/').next().unwrap_or(model);
    let reported = body
        .get("id")
        .or_else(|| body.get("name"))
        .and_then(|id| id.as_str())
        .unwrap_or("");
    if reported == expected || reported == format!("models/{expected}") {
        result.authentication = check("passed", "accepted");
        result.model = check("passed", "available");
    } else {
        result.model.code = "unrecognized_response";
    }
    result
}
#[tauri::command]
pub async fn diagnose_image_provider(
    db: State<'_, Database>,
    model: String,
    provider_id: String,
) -> Result<ProviderDiagnosticResult, AppError> {
    let profile = saved_profile(db.inner(), &model, &provider_id)?;
    Ok(probe_profile(&model, &profile, Duration::from_secs(15)).await)
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderSmokePlan {
    pub model: String,
    pub provider_id: String,
    pub provider_name: String,
    pub endpoint_host: String,
    pub fingerprint: String,
    pub prompt: String,
    pub image_count: u8,
    pub size: Option<String>,
    pub quality: Option<String>,
    pub resolution: Option<String>,
    pub aspect_ratio: Option<String>,
    pub output_format: String,
}
fn smoke_plan(model: &str, profile: &ModelProviderProfile) -> Result<ProviderSmokePlan, AppError> {
    probe_target(model, profile)?;
    let endpoint = validated_url(&image_endpoint_url_for_model_settings(
        model,
        &profile.endpoint_settings,
        ImageEndpointKind::Generate,
    ))?;
    let native = is_gemini_model(model) || is_seedream_model(model) || model == ENGINE_GROK;
    Ok(ProviderSmokePlan {
        model: model.into(),
        provider_id: profile.id.clone(),
        provider_name: profile.name.clone(),
        endpoint_host: endpoint.host_str().unwrap_or("").into(),
        fingerprint: format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(model, profile))
                    .map_err(|_| validation("Cannot prepare test."))?
            )
        ),
        prompt: "A small blue circle on a plain white background.".into(),
        image_count: 1,
        size: if native {
            None
        } else {
            Some("1024x1024".into())
        },
        quality: if native && model != ENGINE_GROK {
            None
        } else {
            Some("low".into())
        },
        resolution: if native {
            Some(
                if model == ENGINE_NANO_BANANA_2 {
                    "512"
                } else {
                    "1K"
                }
                .into(),
            )
        } else {
            None
        },
        aspect_ratio: if native && !is_seedream_model(model) {
            Some("1:1".into())
        } else {
            None
        },
        output_format: if model == ENGINE_GROK { "jpeg" } else { "png" }.into(),
    })
}
#[tauri::command]
pub fn get_provider_smoke_plan(
    db: State<'_, Database>,
    model: String,
    provider_id: String,
) -> Result<ProviderSmokePlan, AppError> {
    smoke_plan(&model, &saved_profile(db.inner(), &model, &provider_id)?)
}
#[tauri::command]
pub async fn run_provider_smoke_test(
    app: tauri::AppHandle,
    db: State<'_, Database>,
    engine: State<'_, crate::api_gateway::GptImageEngine>,
    model: String,
    provider_id: String,
    fingerprint: String,
    client_request_id: String,
) -> Result<GenerateResult, AppError> {
    let _write = crate::storage::write_operation::<AppError>()?;
    let plan = get_provider_smoke_plan(db.clone(), model, provider_id)?;
    if fingerprint != plan.fingerprint {
        return Err(validation(
            "Provider changed. Review a new test plan before generating.",
        ));
    }
    let conversation = crate::commands::conversations::create_conversation(
        db.clone(),
        Some(format!("Connection test · {}", plan.provider_name)),
        None,
    )?;
    crate::generation_execution::submit_generation_request(
        &app,
        db.inner(),
        engine.inner(),
        crate::generation_execution::GenerationExecutionRequest {
            prompt: plan.prompt,
            model: Some(plan.model.clone()),
            provider_profile_id: Some(plan.provider_id),
            client_request_id: Some(client_request_id),
            advanced_options: if matches!(
                plan.model.as_str(),
                ENGINE_GPT_IMAGE_26_FLARE | ENGINE_GPT_IMAGE_26_SUNBURST
            ) {
                Some(ImageAdvancedOptions {
                    stream: Some(false),
                    partial_images: Some(0),
                    ..Default::default()
                })
            } else {
                None
            },
            size: plan.size,
            quality: plan.quality,
            resolution: plan.resolution,
            aspect_ratio: plan.aspect_ratio,
            output_format: Some(plan.output_format),
            image_count: Some(1),
            conversation_id: Some(conversation.id),
            ..Default::default()
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    fn profile(url: String) -> ModelProviderProfile {
        ModelProviderProfile {
            id: "fixture".into(),
            name: "Fixture".into(),
            api_key: "secret-never-report".into(),
            endpoint_settings: EndpointSettings {
                mode: "base_url".into(),
                base_url: url,
                generation_url: String::new(),
                edit_url: String::new(),
            },
        }
    }
    async fn fixture(
        status: u16,
        body: &'static str,
        delay_ms: u64,
    ) -> (String, tokio::task::JoinHandle<String>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = vec![0; 8192];
            let count = socket.read(&mut bytes).await.unwrap();
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            let response = format!(
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            String::from_utf8_lossy(&bytes[..count]).to_string()
        });
        (url, task)
    }
    #[tokio::test]
    async fn diagnostics_get_model_once_and_never_generate() {
        let (url, task) = fixture(200, "{\"id\":\"gpt-image-2\"}", 0).await;
        let result = probe_profile("gpt-image-2", &profile(url), Duration::from_secs(1)).await;
        assert_eq!(result.model.status, "passed");
        assert_eq!(result.generation.status, "unverified");
        let request = task.await.unwrap();
        assert!(request.starts_with("GET /models/gpt-image-2 "));
        assert!(!request.contains("POST"));
    }
    #[tokio::test]
    async fn diagnostics_classify_auth_rate_limit_and_missing_model_without_raw_errors() {
        for (status, auth, model) in [
            (401, "failed", "unverified"),
            (403, "failed", "unverified"),
            (429, "unverified", "unverified"),
            (404, "unverified", "unverified"),
        ] {
            let (url, task) = fixture(status, "secret-never-report", 0).await;
            let result = probe_profile("gpt-image-2", &profile(url), Duration::from_secs(1)).await;
            assert_eq!(result.transport.status, "passed");
            assert_eq!(result.authentication.status, auth);
            assert_eq!(result.model.status, model);
            assert!(!serde_json::to_string(&result)
                .unwrap()
                .contains("secret-never-report"));
            task.await.unwrap();
        }
    }
    #[tokio::test]
    async fn head_only_does_not_claim_authentication_or_model_support() {
        let (url, task) = fixture(200, "", 0).await;
        let mut saved = profile(url.clone());
        saved.endpoint_settings.mode = "full_url".into();
        saved.endpoint_settings.generation_url = format!("{url}/custom");
        let result = probe_profile("gpt-image-2", &saved, Duration::from_secs(1)).await;
        assert_eq!(result.transport.status, "passed");
        assert_eq!(result.authentication.status, "unverified");
        assert_eq!(result.model.status, "unverified");
        assert!(task.await.unwrap().starts_with("HEAD /custom "));
    }
    #[tokio::test]
    async fn timeout_is_redacted_and_does_not_retry() {
        let (url, task) = fixture(200, "", 100).await;
        let result = probe_profile("gpt-image-2", &profile(url), Duration::from_millis(10)).await;
        assert_eq!(result.transport.code, "timeout");
        assert_eq!(result.generation.status, "unverified");
        task.await.unwrap();
    }
    #[test]
    fn gemini_probe_uses_mapped_model_and_smoke_has_fixed_safe_parameters() {
        let mut saved = profile("https://generativelanguage.googleapis.com/v1beta/models".into());
        let (_, url) = probe_target("nano-banana", &saved).unwrap();
        assert!(url.path().ends_with("/gemini-2.5-flash-image"));
        saved.endpoint_settings.base_url = "file:///tmp/secret".into();
        assert!(probe_target("gpt-image-2", &saved).is_err());
        let plan = smoke_plan("gpt-image-2", &profile("https://example.test/v1".into())).unwrap();
        assert_eq!(plan.image_count, 1);
        assert_eq!(plan.quality.as_deref(), Some("low"));
        assert_eq!(plan.size.as_deref(), Some("1024x1024"));
    }
}

#[cfg(test)]
mod safety_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[tokio::test]
    async fn redirect_does_not_forward_credentials_or_follow_location() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let destination = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target = destination.local_addr().unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut connection, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let read = connection.read(&mut request).await.unwrap();
            assert!(read > 0);
            connection.write_all(format!("HTTP/1.1 302 Found\r\nLocation: http://{target}/secret\r\nContent-Length: 0\r\n\r\n").as_bytes()).await.unwrap();
        });
        let profile = ModelProviderProfile {
            id: "p".into(),
            name: "P".into(),
            api_key: "secret".into(),
            endpoint_settings: EndpointSettings {
                mode: "base_url".into(),
                base_url: base,
                generation_url: String::new(),
                edit_url: String::new(),
            },
        };
        let result = probe_profile(ENGINE_GPT_IMAGE_2, &profile, Duration::from_secs(1)).await;
        assert_eq!(result.authentication.code, "redirect_blocked");
        assert!(
            tokio::time::timeout(Duration::from_millis(20), destination.accept())
                .await
                .is_err()
        );
        server.await.unwrap();
    }
    #[tokio::test]
    async fn unavailable_transport_does_not_claim_auth_failure() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        let profile = ModelProviderProfile {
            id: "p".into(),
            name: "P".into(),
            api_key: "secret".into(),
            endpoint_settings: EndpointSettings {
                mode: "base_url".into(),
                base_url: base,
                generation_url: String::new(),
                edit_url: String::new(),
            },
        };
        let result = probe_profile(ENGINE_GPT_IMAGE_2, &profile, Duration::from_secs(1)).await;
        assert_eq!(result.transport.status, "failed");
        assert_eq!(result.authentication.status, "unverified");
        let (method, _) = probe_target(ENGINE_SEEDREAM_FLASH, &profile).unwrap();
        assert_eq!(method, Method::HEAD);
    }
    #[test]
    fn all_smoke_options_are_supported_without_saved_defaults() {
        for model in [
            ENGINE_GPT_IMAGE_2,
            ENGINE_GPT_IMAGE_26_FLARE,
            ENGINE_NANO_BANANA,
            ENGINE_NANO_BANANA_2,
            ENGINE_NANO_BANANA_PRO,
            ENGINE_NANO_BANANA_21,
            ENGINE_GROK,
            ENGINE_SEEDREAM_PRO,
            ENGINE_SEEDREAM_FLASH,
        ] {
            let profile = ModelProviderProfile {
                id: "p".into(),
                name: "P".into(),
                api_key: "secret".into(),
                endpoint_settings: default_endpoint_settings_for_model(model),
            };
            let plan = smoke_plan(model, &profile).unwrap();
            let options = sanitize_request_options_for_model(
                model,
                GptImageRequestOptions {
                    advanced_options: if matches!(
                        model,
                        ENGINE_GPT_IMAGE_26_FLARE | ENGINE_GPT_IMAGE_26_SUNBURST
                    ) {
                        Some(ImageAdvancedOptions {
                            stream: Some(false),
                            partial_images: Some(0),
                            ..Default::default()
                        })
                    } else {
                        None
                    },
                    progress_context: None,
                    prepared_overlay: None,
                    aspect_ratio: plan.aspect_ratio,
                    resolution: plan.resolution,
                    size: plan.size.unwrap_or_else(|| DEFAULT_IMAGE_SIZE.into()),
                    quality: plan.quality.unwrap_or_else(|| DEFAULT_IMAGE_QUALITY.into()),
                    background: DEFAULT_IMAGE_BACKGROUND.into(),
                    output_format: plan.output_format,
                    output_compression: 100,
                    moderation: DEFAULT_IMAGE_MODERATION.into(),
                    input_fidelity: DEFAULT_INPUT_FIDELITY.into(),
                    stream: false,
                    partial_images: 0,
                    image_count: 1,
                },
            );
            validate_request_options(model, &options, 0)
                .unwrap_or_else(|error| panic!("{model}: {error}"));
        }
    }
}
