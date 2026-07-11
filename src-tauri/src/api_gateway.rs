use crate::config::ApiConfig;
use crate::image_engines::{gemini, openai, provider_for_model, ImageProvider};
use crate::models::*;
use reqwest::header;
use reqwest::multipart::{Form, Part};
use serde_json::Value;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RetryAfterHint {
    DelaySeconds(u64),
    HttpDate(chrono::DateTime<chrono::Utc>),
    Invalid,
}

pub(crate) fn parse_retry_after(value: Option<&str>) -> Option<RetryAfterHint> {
    let value = value?;
    if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Some(
            value
                .parse::<u64>()
                .map(RetryAfterHint::DelaySeconds)
                .unwrap_or(RetryAfterHint::Invalid),
        );
    }

    Some(
        httpdate::parse_http_date(value)
            .map(|date| RetryAfterHint::HttpDate(chrono::DateTime::<chrono::Utc>::from(date)))
            .unwrap_or(RetryAfterHint::Invalid),
    )
}

fn retry_after_from_headers(headers: &reqwest::header::HeaderMap) -> Option<RetryAfterHint> {
    match headers.get(header::RETRY_AFTER) {
        None => None,
        Some(value) => value
            .to_str()
            .ok()
            .and_then(|value| parse_retry_after(Some(value)))
            .or(Some(RetryAfterHint::Invalid)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EngineCallError {
    pub(crate) code: String,
    pub(crate) sanitized_message: String,
    pub(crate) retry_after: Option<RetryAfterHint>,
    pub(crate) safe_to_retry: bool,
    pub(crate) outcome_ambiguous: bool,
}

impl EngineCallError {
    pub(crate) fn from_http_status(status: u16, retry_after: Option<RetryAfterHint>) -> Self {
        let retry_hint_valid = retry_after != Some(RetryAfterHint::Invalid);
        let (code, message, status_retryable) = match status {
            429 => (
                "rate_limited",
                "The image provider rate limit was reached",
                true,
            ),
            500..=599 => (
                "provider_unavailable",
                "The image provider is temporarily unavailable",
                true,
            ),
            _ => (
                "request_rejected",
                "The image provider rejected the request",
                false,
            ),
        };

        Self {
            code: code.to_string(),
            sanitized_message: message.to_string(),
            retry_after,
            safe_to_retry: status_retryable && retry_hint_valid,
            outcome_ambiguous: false,
        }
    }

    pub(crate) fn network_before_response() -> Self {
        Self {
            code: "network_before_response".to_string(),
            sanitized_message: "The image provider could not be reached".to_string(),
            retry_after: None,
            safe_to_retry: true,
            outcome_ambiguous: false,
        }
    }

    pub(crate) fn provider_outcome_unknown(_detail: &str) -> Self {
        Self {
            code: "provider_outcome_unknown".to_string(),
            sanitized_message: "The image provider outcome could not be confirmed".to_string(),
            retry_after: None,
            safe_to_retry: false,
            outcome_ambiguous: true,
        }
    }

    pub(crate) fn request_rejected() -> Self {
        Self {
            code: "request_rejected".to_string(),
            sanitized_message: "The image request is invalid".to_string(),
            retry_after: None,
            safe_to_retry: false,
            outcome_ambiguous: false,
        }
    }

    pub(crate) fn provider_configuration_invalid() -> Self {
        Self {
            code: "provider_configuration_invalid".to_string(),
            sanitized_message: "The image provider configuration is invalid".to_string(),
            retry_after: None,
            safe_to_retry: false,
            outcome_ambiguous: false,
        }
    }

    fn from_send_error(error: &reqwest::Error) -> Self {
        if error.is_builder() {
            Self::request_rejected()
        } else if error.is_connect() {
            Self::network_before_response()
        } else {
            Self::provider_outcome_unknown("provider request did not complete")
        }
    }
}

/// Complete raw body from exactly one successful provider submission.
///
/// Intentionally does not implement `Debug` or `Serialize`: provider bodies may
/// contain sensitive URLs and belong only to the response-artifact boundary.
pub(crate) struct ProviderAttemptBody {
    pub(crate) body_text: String,
    pub(crate) requested_image_count: u8,
}

#[async_trait::async_trait]
pub trait ImageEngine: Send + Sync {
    async fn generate(
        &self,
        model: &str,
        api_key: &str,
        endpoint_url: &str,
        prompt: &str,
        options: &GptImageRequestOptions,
    ) -> Result<ProviderAttemptBody, EngineCallError>;

    async fn edit(
        &self,
        model: &str,
        api_key: &str,
        endpoint_url: &str,
        prompt: &str,
        source_image_paths: &[String],
        options: &GptImageRequestOptions,
    ) -> Result<ProviderAttemptBody, EngineCallError>;
}

pub struct GptImageEngine {
    client: reqwest::Client,
    edit_client: reqwest::Client,
    download_client: reqwest::Client,
}

struct SafeDownloadDnsResolver;

impl reqwest::dns::Resolve for SafeDownloadDnsResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await
                .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { Box::new(error) })?
                .collect();
            GptImageEngine::validate_resolved_download_addrs(&addrs)?;
            Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

impl GptImageEngine {
    pub fn new(config: &ApiConfig) -> Result<Self, String> {
        let timeout_secs = Self::normalize_timeout_secs(config.timeout_secs);
        let client = Self::build_client(timeout_secs);
        let edit_client = Self::build_client(None);
        let download_client = Self::build_download_client()?;

        Ok(Self {
            client,
            edit_client,
            download_client,
        })
    }

    fn normalize_timeout_secs(timeout_secs: u64) -> Option<u64> {
        match timeout_secs {
            0 | 120 => None,
            seconds => Some(seconds.max(1)),
        }
    }

    fn build_client(timeout_secs: Option<u64>) -> reqwest::Client {
        let mut builder = reqwest::Client::builder();
        if let Some(timeout_secs) = timeout_secs {
            builder = builder.timeout(Duration::from_secs(timeout_secs));
        }
        builder.build().unwrap_or_else(|error| {
            log::warn!(
                "Failed to build configured HTTP client: {}, using default client",
                error
            );
            reqwest::Client::new()
        })
    }

    fn build_download_client() -> Result<reqwest::Client, String> {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .no_proxy()
            .dns_resolver(Arc::new(SafeDownloadDnsResolver))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 3
                    || GptImageEngine::validate_download_url(attempt.url().as_str()).is_err()
                {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .map_err(|error| format!("Failed to build safe image download client: {error}"))
    }

    async fn read_attempt_response(
        response: reqwest::Response,
        requested_image_count: u8,
    ) -> Result<ProviderAttemptBody, EngineCallError> {
        let status = response.status();
        let retry_after = retry_after_from_headers(response.headers());
        if !status.is_success() {
            return Err(EngineCallError::from_http_status(
                status.as_u16(),
                retry_after,
            ));
        }

        let body_text = response
            .text()
            .await
            .map_err(|_| EngineCallError::provider_outcome_unknown("response body read failed"))?;
        Ok(ProviderAttemptBody {
            body_text,
            requested_image_count,
        })
    }

    async fn request_gemini_images(
        &self,
        api_key: &str,
        endpoint_url: &str,
        prompt: &str,
        source_images: &[PreparedEditImage],
        options: &GptImageRequestOptions,
    ) -> Result<ProviderAttemptBody, EngineCallError> {
        if api_key.is_empty() {
            return Err(EngineCallError::provider_configuration_invalid());
        }

        let inline_images = source_images
            .iter()
            .map(|image| gemini::GeminiInlineImage {
                mime_type: image.mime_type.clone(),
                data: base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    &image.bytes,
                ),
            })
            .collect::<Vec<_>>();
        let request_body = gemini::build_request_body(prompt, &inline_images, options);
        let response = self
            .client
            .post(openai::image_endpoint_url(endpoint_url))
            .header("x-goog-api-key", api_key)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|error| EngineCallError::from_send_error(&error))?;

        Self::read_attempt_response(response, options.image_count).await
    }

    pub(crate) async fn decode_images_from_response(
        &self,
        body_text: &str,
    ) -> Result<Vec<Vec<u8>>, String> {
        if let Ok(api_response) = serde_json::from_str::<OpenAiImageResponse>(body_text) {
            let mut images = Vec::new();
            for data in &api_response.data {
                if let Some(encoded) = &data.b64_json {
                    let bytes =
                        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)
                            .map_err(|_| "Provider image data was not valid base64".to_string())?;
                    images.push(bytes);
                    continue;
                }

                if let Some(image_url) = &data.url {
                    images.push(self.download_image(image_url).await?);
                }
            }
            if !images.is_empty() {
                return Ok(images);
            }
        }

        let value: Value = serde_json::from_str(body_text)
            .map_err(|_| "Provider response was not valid JSON".to_string())?;
        gemini::parse_images(&value)
            .map_err(|_| "Provider response did not include decodable image data".to_string())
    }

    async fn prepare_edit_images(
        &self,
        source_image_paths: &[String],
    ) -> Result<Vec<PreparedEditImage>, EngineCallError> {
        let mut prepared = Vec::with_capacity(source_image_paths.len());
        for path in source_image_paths {
            let bytes = tokio::fs::read(path)
                .await
                .map_err(|_| EngineCallError::request_rejected())?;
            let file_name = Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("source-image")
                .to_string();
            prepared.push(PreparedEditImage {
                file_name,
                mime_type: openai::mime_type_for_path(path).to_string(),
                bytes,
            });
        }
        Ok(prepared)
    }

    fn build_edit_form(
        model: &str,
        prompt: &str,
        options: &GptImageRequestOptions,
        source_images: &[PreparedEditImage],
    ) -> Result<Form, EngineCallError> {
        let mut form = Form::new();
        for (key, value) in openai::build_edit_text_fields(model, prompt, options) {
            form = form.text(key, value);
        }
        for image in source_images {
            let part = Part::bytes(image.bytes.clone())
                .file_name(image.file_name.clone())
                .mime_str(&image.mime_type)
                .map_err(|_| EngineCallError::request_rejected())?;
            form = form.part(openai::edit_image_part_field_name(), part);
        }
        Ok(form)
    }

    const MAX_DOWNLOAD_IMAGE_BYTES: u64 = 32 * 1024 * 1024;

    fn validate_download_url(url: &str) -> Result<reqwest::Url, String> {
        let parsed =
            reqwest::Url::parse(url).map_err(|_| "Image download URL is invalid".to_string())?;
        if parsed.scheme() != "https" {
            return Err("Image download URL must use https".to_string());
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| "Image download URL must include a host".to_string())?;
        if host.eq_ignore_ascii_case("localhost") {
            return Err("Image download URL host is not allowed".to_string());
        }
        let ip_host = host.trim_start_matches('[').trim_end_matches(']');
        if ip_host.parse::<IpAddr>().is_ok_and(Self::is_blocked_ip) {
            return Err("Image download URL IP is not allowed".to_string());
        }
        Ok(parsed)
    }

    fn is_blocked_ip(ip: IpAddr) -> bool {
        match ip {
            IpAddr::V4(ip) => {
                ip.is_private()
                    || ip.is_loopback()
                    || ip.is_link_local()
                    || ip.is_unspecified()
                    || ip.is_broadcast()
                    || ip.is_multicast()
                    || ip == Ipv4Addr::new(169, 254, 169, 254)
            }
            IpAddr::V6(ip) => {
                ip.is_loopback()
                    || ip.is_unspecified()
                    || ip.is_multicast()
                    || Self::is_ipv6_unique_local(ip)
                    || Self::is_ipv6_unicast_link_local(ip)
            }
        }
    }

    fn is_ipv6_unique_local(ip: Ipv6Addr) -> bool {
        (ip.segments()[0] & 0xfe00) == 0xfc00
    }

    fn is_ipv6_unicast_link_local(ip: Ipv6Addr) -> bool {
        (ip.segments()[0] & 0xffc0) == 0xfe80
    }

    fn validate_resolved_download_addrs(addrs: &[SocketAddr]) -> Result<(), String> {
        if addrs.is_empty() {
            return Err("Image download host did not resolve".to_string());
        }
        if addrs
            .iter()
            .any(|address| Self::is_blocked_ip(address.ip()))
        {
            return Err("Image download host resolved to a blocked IP".to_string());
        }
        Ok(())
    }

    fn validate_downloaded_image(
        bytes: &[u8],
        content_type: Option<&str>,
        reported_size: u64,
    ) -> Result<(), String> {
        if reported_size > Self::MAX_DOWNLOAD_IMAGE_BYTES
            || bytes.len() as u64 > Self::MAX_DOWNLOAD_IMAGE_BYTES
        {
            return Err("Downloaded image is too large".to_string());
        }
        let content_type = content_type
            .and_then(|value| value.split(';').next())
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .ok_or_else(|| "Downloaded image is missing its media type".to_string())?;
        if !content_type.starts_with("image/") {
            return Err("Downloaded image has an unsupported media type".to_string());
        }
        if !Self::has_image_magic(bytes) {
            return Err("Downloaded content is not a supported image".to_string());
        }
        Ok(())
    }

    fn has_image_magic(bytes: &[u8]) -> bool {
        bytes.starts_with(b"\x89PNG\r\n\x1a\n")
            || bytes.starts_with(b"\xff\xd8\xff")
            || bytes.starts_with(b"GIF87a")
            || bytes.starts_with(b"GIF89a")
            || (bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP")
            || (bytes.len() >= 12 && &bytes[4..12] == b"ftypavif")
    }

    async fn download_image(&self, url: &str) -> Result<Vec<u8>, String> {
        let url = Self::validate_download_url(url)?;
        let mut response = self
            .download_client
            .get(url)
            .send()
            .await
            .map_err(|_| "Image download failed".to_string())?;
        if !response.status().is_success() {
            return Err("Image download was rejected".to_string());
        }
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        if response
            .content_length()
            .is_some_and(|size| size > Self::MAX_DOWNLOAD_IMAGE_BYTES)
        {
            return Err("Downloaded image is too large".to_string());
        }

        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "Image download failed".to_string())?
        {
            bytes.extend_from_slice(&chunk);
            if bytes.len() as u64 > Self::MAX_DOWNLOAD_IMAGE_BYTES {
                return Err("Downloaded image is too large".to_string());
            }
        }
        Self::validate_downloaded_image(&bytes, content_type.as_deref(), bytes.len() as u64)?;
        Ok(bytes)
    }
}

#[async_trait::async_trait]
impl ImageEngine for GptImageEngine {
    async fn generate(
        &self,
        model: &str,
        api_key: &str,
        endpoint_url: &str,
        prompt: &str,
        options: &GptImageRequestOptions,
    ) -> Result<ProviderAttemptBody, EngineCallError> {
        if provider_for_model(model) == ImageProvider::Gemini {
            return self
                .request_gemini_images(api_key, endpoint_url, prompt, &[], options)
                .await;
        }
        if api_key.is_empty() {
            return Err(EngineCallError::provider_configuration_invalid());
        }

        let response = self
            .client
            .post(openai::image_endpoint_url(endpoint_url))
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&openai::build_generation_request_body(
                model, prompt, options,
            ))
            .send()
            .await
            .map_err(|error| EngineCallError::from_send_error(&error))?;
        Self::read_attempt_response(response, options.image_count).await
    }

    async fn edit(
        &self,
        model: &str,
        api_key: &str,
        endpoint_url: &str,
        prompt: &str,
        source_image_paths: &[String],
        options: &GptImageRequestOptions,
    ) -> Result<ProviderAttemptBody, EngineCallError> {
        if api_key.is_empty() {
            return Err(EngineCallError::provider_configuration_invalid());
        }
        if source_image_paths.is_empty() {
            return Err(EngineCallError::request_rejected());
        }

        let prepared_images = self.prepare_edit_images(source_image_paths).await?;
        if provider_for_model(model) == ImageProvider::Gemini {
            return self
                .request_gemini_images(api_key, endpoint_url, prompt, &prepared_images, options)
                .await;
        }

        let response = self
            .edit_client
            .post(openai::image_endpoint_url(endpoint_url))
            .header("Authorization", format!("Bearer {api_key}"))
            .multipart(Self::build_edit_form(
                model,
                prompt,
                options,
                &prepared_images,
            )?)
            .send()
            .await
            .map_err(|error| EngineCallError::from_send_error(&error))?;
        Self::read_attempt_response(response, options.image_count).await
    }
}

struct PreparedEditImage {
    file_name: String,
    mime_type: String,
    bytes: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn test_options(image_count: u8) -> GptImageRequestOptions {
        GptImageRequestOptions {
            size: DEFAULT_IMAGE_SIZE.to_string(),
            quality: DEFAULT_IMAGE_QUALITY.to_string(),
            background: DEFAULT_IMAGE_BACKGROUND.to_string(),
            output_format: DEFAULT_OUTPUT_FORMAT.to_string(),
            output_compression: DEFAULT_OUTPUT_COMPRESSION,
            moderation: DEFAULT_IMAGE_MODERATION.to_string(),
            input_fidelity: DEFAULT_INPUT_FIDELITY.to_string(),
            stream: DEFAULT_IMAGE_STREAM,
            partial_images: DEFAULT_PARTIAL_IMAGES,
            image_count,
        }
    }

    #[test]
    fn provider_errors_preserve_retry_after_shape_and_ambiguity() {
        let seconds = EngineCallError::from_http_status(429, Some(RetryAfterHint::DelaySeconds(3)));
        assert_eq!(seconds.code, "rate_limited");
        assert_eq!(seconds.retry_after, Some(RetryAfterHint::DelaySeconds(3)));
        assert!(seconds.safe_to_retry);
        assert!(!seconds.outcome_ambiguous);

        let date = chrono::DateTime::parse_from_rfc2822("Wed, 21 Oct 2015 07:28:00 GMT")
            .expect("parse HTTP date")
            .with_timezone(&chrono::Utc);
        assert_eq!(
            parse_retry_after(Some("Wed, 21 Oct 2015 07:28:00 GMT")),
            Some(RetryAfterHint::HttpDate(date))
        );
        assert_eq!(
            parse_retry_after(Some("definitely-not-a-delay")),
            Some(RetryAfterHint::Invalid)
        );
        assert_eq!(parse_retry_after(None), None);

        let unknown = EngineCallError::provider_outcome_unknown("connection reset");
        assert_eq!(unknown.code, "provider_outcome_unknown");
        assert!(!unknown.safe_to_retry);
        assert!(unknown.outcome_ambiguous);
        assert!(!unknown.sanitized_message.contains("connection reset"));
    }

    #[test]
    fn invalid_retry_after_does_not_authorize_retry() {
        let error = EngineCallError::from_http_status(429, Some(RetryAfterHint::Invalid));
        assert!(!error.safe_to_retry);
        assert_eq!(error.retry_after, Some(RetryAfterHint::Invalid));
    }

    #[test]
    fn retry_after_accepts_all_http_date_wire_formats() {
        for value in [
            "Sun, 06 Nov 1994 08:49:37 GMT",
            "Sunday, 06-Nov-94 08:49:37 GMT",
            "Sun Nov  6 08:49:37 1994",
        ] {
            assert!(
                matches!(
                    parse_retry_after(Some(value)),
                    Some(RetryAfterHint::HttpDate(_))
                ),
                "expected an HTTP date for {value}"
            );
        }
    }

    #[tokio::test]
    async fn openai_and_gemini_generate_submit_once_and_return_short_raw_response() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        for model in ["gpt-image-2", "nano-banana-2"] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind test server");
            let address = listener.local_addr().expect("server address");
            let submissions = Arc::new(AtomicUsize::new(0));
            let observed_submissions = Arc::clone(&submissions);
            let server = tokio::spawn(async move {
                while let Ok(Ok((mut stream, _))) =
                    tokio::time::timeout(Duration::from_millis(300), listener.accept()).await
                {
                    observed_submissions.fetch_add(1, Ordering::SeqCst);
                    let mut request = vec![0; 8192];
                    let _ = stream.read(&mut request).await.expect("read request");
                    let body = r#"{"data":[{"b64_json":"not-decoded-here"}]}"#;
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    stream
                        .write_all(response.as_bytes())
                        .await
                        .expect("write response");
                }
            });

            let engine = GptImageEngine::new(&ApiConfig {
                timeout_secs: 2,
                max_retries: 5,
            })
            .expect("build engine");
            let result = engine
                .generate(
                    model,
                    "secret-key",
                    &format!("http://{address}/images/generations"),
                    "draw one image",
                    &test_options(2),
                )
                .await
                .expect("raw provider response");

            assert_eq!(result.requested_image_count, 2);
            assert!(result.body_text.contains("not-decoded-here"));
            server.await.expect("join server");
            assert_eq!(submissions.load(Ordering::SeqCst), 1, "model {model}");
        }
    }

    #[test]
    fn download_url_validation_rejects_unsafe_literal_hosts() {
        for url in [
            "http://example.com/image.png",
            "https://localhost/image.png",
            "https://127.0.0.1/image.png",
            "https://10.0.0.5/image.png",
            "https://172.16.0.5/image.png",
            "https://192.168.1.5/image.png",
            "https://169.254.169.254/latest/meta-data",
            "https://[::1]/image.png",
            "https://[fc00::1]/image.png",
            "https://[fe80::1]/image.png",
        ] {
            assert!(
                GptImageEngine::validate_download_url(url).is_err(),
                "{url} should be rejected"
            );
        }
    }

    #[test]
    fn download_url_validation_allows_public_https_literal_hosts() {
        assert!(GptImageEngine::validate_download_url("https://example.com/image.png").is_ok());
        assert!(GptImageEngine::validate_download_url("https://93.184.216.34/image.png").is_ok());
        assert!(GptImageEngine::validate_download_url(
            "https://[2606:2800:220:1:248:1893:25c8:1946]/image.png"
        )
        .is_ok());
    }

    #[test]
    fn download_dns_validation_rejects_private_resolved_addresses() {
        let addrs = [
            SocketAddr::from(([127, 0, 0, 1], 443)),
            SocketAddr::from(([192, 168, 1, 10], 443)),
            SocketAddr::from(([169, 254, 169, 254], 443)),
        ];
        for address in addrs {
            assert!(GptImageEngine::validate_resolved_download_addrs(&[address]).is_err());
        }
    }

    #[test]
    fn download_dns_validation_allows_public_resolved_addresses() {
        assert!(
            GptImageEngine::validate_resolved_download_addrs(&[SocketAddr::from((
                [93, 184, 216, 34],
                443,
            ))])
            .is_ok()
        );
    }

    #[test]
    fn download_client_build_does_not_fall_back_to_default_client() {
        assert!(GptImageEngine::build_download_client().is_ok());
    }

    #[test]
    fn image_download_validation_rejects_size_type_and_magic_mismatches() {
        let png = b"\x89PNG\r\n\x1a\nrest";
        assert!(GptImageEngine::validate_downloaded_image(
            png,
            Some("image/png"),
            GptImageEngine::MAX_DOWNLOAD_IMAGE_BYTES + 1
        )
        .is_err());
        assert!(GptImageEngine::validate_downloaded_image(
            png,
            Some("application/json"),
            png.len() as u64
        )
        .is_err());
        assert!(
            GptImageEngine::validate_downloaded_image(b"not an image", Some("image/png"), 12)
                .is_err()
        );
        assert!(GptImageEngine::validate_downloaded_image(
            png,
            Some("image/png"),
            png.len() as u64
        )
        .is_ok());
    }
}
