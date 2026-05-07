use crate::config::ApiConfig;
use crate::image_engines::{gemini, openai, provider_for_model, ImageProvider};
use crate::models::*;
use reqwest::multipart::{Form, Part};
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

pub struct EngineImagesResult {
    pub images: Vec<Vec<u8>>,
    pub response_file: Option<String>,
}

fn missing_image_count(requested: u8, received: usize) -> Option<u8> {
    let requested = requested as usize;
    (received < requested).then(|| (requested - received) as u8)
}

#[async_trait::async_trait]
pub trait ImageEngine: Send + Sync {
    async fn generate(
        &self,
        generation_id: &str,
        model: &str,
        api_key: &str,
        endpoint_url: &str,
        prompt: &str,
        options: &GptImageRequestOptions,
        db: Option<&crate::db::Database>,
        log_dir: Option<&std::path::Path>,
    ) -> Result<EngineImagesResult, String>;

    async fn edit(
        &self,
        generation_id: &str,
        model: &str,
        api_key: &str,
        endpoint_url: &str,
        prompt: &str,
        source_image_paths: &[String],
        options: &GptImageRequestOptions,
        db: Option<&crate::db::Database>,
        log_dir: Option<&std::path::Path>,
    ) -> Result<EngineImagesResult, String>;
}

pub struct GptImageEngine {
    client: reqwest::Client,
    edit_client: reqwest::Client,
    max_retries: u32,
    timeout_secs: Option<u64>,
}

impl GptImageEngine {
    pub fn new(config: &ApiConfig) -> Self {
        let timeout_secs = Self::normalize_timeout_secs(config.timeout_secs);
        let client = Self::build_client(timeout_secs);
        let edit_client = Self::build_client(None);

        Self {
            client,
            edit_client,
            max_retries: config.max_retries,
            timeout_secs,
        }
    }

    fn normalize_timeout_secs(timeout_secs: u64) -> Option<u64> {
        match timeout_secs {
            0 => None,
            // Older releases persisted 120s as the implicit default. Treat it as "no limit"
            // so upgraded clients stop aborting long-running image requests.
            120 => None,
            secs => Some(secs.max(1)),
        }
    }

    fn build_client(timeout_secs: Option<u64>) -> reqwest::Client {
        let mut builder = reqwest::Client::builder();

        if let Some(timeout_secs) = timeout_secs {
            builder = builder.timeout(Duration::from_secs(timeout_secs));
        }

        builder.build().unwrap_or_else(|e| {
            log::warn!(
                "Failed to build configured HTTP client: {}, using default client",
                e
            );
            reqwest::Client::new()
        })
    }
}

#[async_trait::async_trait]
impl ImageEngine for GptImageEngine {
    async fn generate(
        &self,
        generation_id: &str,
        model: &str,
        api_key: &str,
        endpoint_url: &str,
        prompt: &str,
        options: &GptImageRequestOptions,
        db: Option<&crate::db::Database>,
        log_dir: Option<&std::path::Path>,
    ) -> Result<EngineImagesResult, String> {
        match provider_for_model(model) {
            ImageProvider::Gemini => {
                return self
                    .request_gemini_images(
                        generation_id,
                        model,
                        api_key,
                        endpoint_url,
                        prompt,
                        &[],
                        options,
                        db,
                        log_dir,
                    )
                    .await;
            }
            ImageProvider::OpenAi => {}
        }

        let url = openai::image_endpoint_url(endpoint_url);

        if let Some(db) = db {
            let masked_key = if api_key.len() > 8 {
                format!("{}...{}", &api_key[..3], &api_key[api_key.len() - 4..])
            } else {
                "sk-****".to_string()
            };
            let req_meta = serde_json::json!({
                "url": &url,
                "model": model,
                "size": &options.size,
                "quality": &options.quality,
                "background": &options.background,
                "output_format": &options.output_format,
                "output_compression": options.output_compression,
                "moderation": &options.moderation,
                "stream": options.stream,
                "partial_images": options.partial_images,
                "image_count": options.image_count,
                "api_key": masked_key,
            });
            let _ = db.insert_log(
                "api_request",
                "info",
                &format!("POST {} — model: {}, size: {}", url, model, options.size),
                Some(generation_id),
                Some(&req_meta.to_string()),
                None,
            );
        }

        if api_key.is_empty() {
            return Err("API key not set".to_string());
        }

        let mut images = Vec::with_capacity(options.image_count as usize);
        let mut response_files = Vec::new();

        while let Some(batch_count) = missing_image_count(options.image_count, images.len()) {
            let mut batch_options = options.clone();
            batch_options.image_count = batch_count;

            let request_body = openai::build_generation_request_body(model, prompt, &batch_options);

            log::info!(
                "Sending image generation request to {} — model: {}, size: {}, quality: {}, background: {}, output_format: {}, output_compression: {}, moderation: {}, stream: {}, partial_images: {}, count: {}",
                url,
                model,
                batch_options.size,
                batch_options.quality,
                batch_options.background,
                batch_options.output_format,
                batch_options.output_compression,
                batch_options.moderation,
                batch_options.stream,
                batch_options.partial_images,
                batch_options.image_count
            );

            let mut last_error = None;
            let mut batch_images = None;
            let mut batch_response_file = None;

            for attempt in 0..=self.max_retries {
                let response = self
                    .client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json")
                    .json(&request_body)
                    .send()
                    .await;

                let response = match response {
                    Ok(response) => response,
                    Err(e) => {
                        let error = self.format_request_error(&url, &e, self.timeout_secs);
                        if let Some(db) = db {
                            let _ = db.insert_log(
                                "api_response", "error",
                                &error,
                                Some(generation_id),
                                Some(&serde_json::json!({"attempt": attempt + 1, "max_retries": self.max_retries}).to_string()),
                                None,
                            );
                        }
                        if attempt < self.max_retries {
                            log::warn!(
                                "{}; retrying ({}/{})",
                                error,
                                attempt + 1,
                                self.max_retries
                            );
                            last_error = Some(error);
                            continue;
                        }
                        return Err(error);
                    }
                };

                let status = response.status();
                let body_text = response
                    .text()
                    .await
                    .map_err(|e| format!("Read body failed: {}", e))?;

                if !status.is_success() {
                    let response_file = self.write_response_body(log_dir, &body_text);
                    let error = format!("API error {} from {}: {}", status, url, body_text);
                    log::error!(
                        "API error {} from {} — response preview: {}",
                        status,
                        url,
                        Self::preview_text(&body_text, 500)
                    );
                    if let Some(db) = db {
                        let _ = db.insert_log(
                            "api_response",
                            "error",
                            &error,
                            Some(generation_id),
                            Some(
                                &serde_json::json!({
                                    "url": &url,
                                    "status": status.as_u16(),
                                    "body_size": body_text.len(),
                                    "body_preview": Self::preview_text(&body_text, 500),
                                })
                                .to_string(),
                            ),
                            response_file.as_deref(),
                        );
                    }
                    if status.is_server_error() && attempt < self.max_retries {
                        log::warn!(
                            "Retrying image generation ({}/{})",
                            attempt + 1,
                            self.max_retries
                        );
                        last_error = Some(error);
                        continue;
                    }
                    return Err(error);
                }

                log::info!("API responded {} ({} bytes)", status, body_text.len());

                batch_response_file = self.log_response_body(
                    db,
                    log_dir,
                    Some(generation_id),
                    status.as_u16(),
                    &body_text,
                );

                let decoded_images = self.decode_images_from_response(&body_text).await?;
                log::info!("Decoded {} image(s) from response", decoded_images.len());
                batch_images = Some(decoded_images);
                break;
            }

            let batch_images = batch_images
                .ok_or_else(|| last_error.unwrap_or_else(|| "Request failed".to_string()))?;
            if let Some(response_file) = batch_response_file {
                response_files.push(response_file);
            }

            let batch_image_count = batch_images.len();
            Self::append_images_up_to_requested_count(
                &mut images,
                batch_images,
                options.image_count,
            );

            if let Some(remaining) = missing_image_count(options.image_count, images.len()) {
                let message = format!(
                    "API returned {}/{} requested image(s) for this batch; requesting {} remaining image(s)",
                    batch_image_count, batch_count, remaining
                );
                log::warn!("{}", message);
                if let Some(db) = db {
                    let _ = db.insert_log(
                        "generation",
                        "warn",
                        &message,
                        Some(generation_id),
                        Some(
                            &serde_json::json!({
                                "batch_image_count": batch_image_count,
                                "batch_requested_count": batch_count,
                                "remaining_image_count": remaining,
                                "requested_image_count": options.image_count,
                            })
                            .to_string(),
                        ),
                        None,
                    );
                }
            }
        }

        let response_file = self.recoverable_response_file(log_dir, response_files, &images);
        Ok(EngineImagesResult {
            images,
            response_file,
        })
    }

    async fn edit(
        &self,
        generation_id: &str,
        model: &str,
        api_key: &str,
        endpoint_url: &str,
        prompt: &str,
        source_image_paths: &[String],
        options: &GptImageRequestOptions,
        db: Option<&crate::db::Database>,
        log_dir: Option<&std::path::Path>,
    ) -> Result<EngineImagesResult, String> {
        match provider_for_model(model) {
            ImageProvider::Gemini => {
                if source_image_paths.is_empty() {
                    return Err("At least one source image is required for editing.".to_string());
                }

                let prepared_images = self.prepare_edit_images(source_image_paths).await?;
                return self
                    .request_gemini_images(
                        generation_id,
                        model,
                        api_key,
                        endpoint_url,
                        prompt,
                        &prepared_images,
                        options,
                        db,
                        log_dir,
                    )
                    .await;
            }
            ImageProvider::OpenAi => {}
        }

        let url = openai::image_endpoint_url(endpoint_url);

        if source_image_paths.is_empty() {
            return Err("At least one source image is required for editing.".to_string());
        }

        if let Some(db) = db {
            let masked_key = if api_key.len() > 8 {
                format!("{}...{}", &api_key[..3], &api_key[api_key.len() - 4..])
            } else {
                "sk-****".to_string()
            };
            let req_meta = serde_json::json!({
                "url": &url,
                "model": model,
                "source_image_count": source_image_paths.len(),
                "size": &options.size,
                "quality": &options.quality,
                "background": &options.background,
                "input_fidelity": &options.input_fidelity,
                "output_format": &options.output_format,
                "output_compression": options.output_compression,
                "moderation": &options.moderation,
                "stream": options.stream,
                "partial_images": options.partial_images,
                "image_count": options.image_count,
                "api_key": masked_key,
            });
            let _ = db.insert_log(
                "api_request",
                "info",
                &format!(
                    "POST {} — model: {}, source_images: {}",
                    url,
                    model,
                    source_image_paths.len()
                ),
                Some(generation_id),
                Some(&req_meta.to_string()),
                None,
            );
        }

        if api_key.is_empty() {
            return Err("API key not set".to_string());
        }

        let prepared_images = self.prepare_edit_images(source_image_paths).await?;

        let mut images = Vec::with_capacity(options.image_count as usize);
        let mut response_files = Vec::new();

        while let Some(batch_count) = missing_image_count(options.image_count, images.len()) {
            let mut batch_options = options.clone();
            batch_options.image_count = batch_count;

            log::info!(
                "Sending image edit request to {} — model: {}, source_images: {}, size: {}, quality: {}, background: {}, input_fidelity: {}, output_format: {}, output_compression: {}, moderation: {}, stream: {}, partial_images: {}, count: {}",
                url,
                model,
                prepared_images.len(),
                batch_options.size,
                batch_options.quality,
                batch_options.background,
                batch_options.input_fidelity,
                batch_options.output_format,
                batch_options.output_compression,
                batch_options.moderation,
                batch_options.stream,
                batch_options.partial_images,
                batch_options.image_count
            );

            let mut last_error = None;
            let mut batch_images = None;
            let mut batch_response_file = None;

            for attempt in 0..=self.max_retries {
                let form = self.build_edit_form(model, prompt, &batch_options, &prepared_images)?;

                let response = self
                    .edit_client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .multipart(form)
                    .send()
                    .await;

                let response = match response {
                    Ok(response) => response,
                    Err(e) => {
                        let error = self.format_request_error(&url, &e, None);
                        if let Some(db) = db {
                            let _ = db.insert_log(
                                "api_response",
                                "error",
                                &error,
                                Some(generation_id),
                                Some(
                                    &serde_json::json!({
                                        "attempt": attempt + 1,
                                        "max_retries": self.max_retries
                                    })
                                    .to_string(),
                                ),
                                None,
                            );
                        }
                        if attempt < self.max_retries {
                            log::warn!(
                                "{}; retrying ({}/{})",
                                error,
                                attempt + 1,
                                self.max_retries
                            );
                            last_error = Some(error);
                            continue;
                        }
                        return Err(error);
                    }
                };

                let status = response.status();
                let body_text = response
                    .text()
                    .await
                    .map_err(|e| format!("Read body failed: {}", e))?;

                if !status.is_success() {
                    let response_file = self.write_response_body(log_dir, &body_text);
                    let error = format!("API error {} from {}: {}", status, url, body_text);
                    log::error!(
                        "API error {} from {} — response preview: {}",
                        status,
                        url,
                        Self::preview_text(&body_text, 500)
                    );
                    if let Some(db) = db {
                        let _ = db.insert_log(
                            "api_response",
                            "error",
                            &error,
                            Some(generation_id),
                            Some(
                                &serde_json::json!({
                                    "url": &url,
                                    "status": status.as_u16(),
                                    "body_size": body_text.len(),
                                    "body_preview": Self::preview_text(&body_text, 500),
                                })
                                .to_string(),
                            ),
                            response_file.as_deref(),
                        );
                    }
                    if status.is_server_error() && attempt < self.max_retries {
                        log::warn!("Retrying image edit ({}/{})", attempt + 1, self.max_retries);
                        last_error = Some(error);
                        continue;
                    }
                    return Err(error);
                }

                log::info!("API responded {} ({} bytes)", status, body_text.len());

                batch_response_file = self.log_response_body(
                    db,
                    log_dir,
                    Some(generation_id),
                    status.as_u16(),
                    &body_text,
                );

                let decoded_images = self.decode_images_from_response(&body_text).await?;
                log::info!(
                    "Decoded {} image(s) from edit response",
                    decoded_images.len()
                );
                batch_images = Some(decoded_images);
                break;
            }

            let batch_images = batch_images
                .ok_or_else(|| last_error.unwrap_or_else(|| "Request failed".to_string()))?;
            if let Some(response_file) = batch_response_file {
                response_files.push(response_file);
            }

            let batch_image_count = batch_images.len();
            Self::append_images_up_to_requested_count(
                &mut images,
                batch_images,
                options.image_count,
            );

            if let Some(remaining) = missing_image_count(options.image_count, images.len()) {
                let message = format!(
                    "API returned {}/{} requested image(s) for this edit batch; requesting {} remaining image(s)",
                    batch_image_count, batch_count, remaining
                );
                log::warn!("{}", message);
                if let Some(db) = db {
                    let _ = db.insert_log(
                        "generation",
                        "warn",
                        &message,
                        Some(generation_id),
                        Some(
                            &serde_json::json!({
                                "batch_image_count": batch_image_count,
                                "batch_requested_count": batch_count,
                                "remaining_image_count": remaining,
                                "requested_image_count": options.image_count,
                            })
                            .to_string(),
                        ),
                        None,
                    );
                }
            }
        }

        let response_file = self.recoverable_response_file(log_dir, response_files, &images);
        Ok(EngineImagesResult {
            images,
            response_file,
        })
    }
}

impl GptImageEngine {
    async fn request_gemini_images(
        &self,
        generation_id: &str,
        model: &str,
        api_key: &str,
        endpoint_url: &str,
        prompt: &str,
        source_images: &[PreparedEditImage],
        options: &GptImageRequestOptions,
        db: Option<&crate::db::Database>,
        log_dir: Option<&std::path::Path>,
    ) -> Result<EngineImagesResult, String> {
        let url = openai::image_endpoint_url(endpoint_url);

        if let Some(db) = db {
            let masked_key = if api_key.len() > 8 {
                format!("{}...{}", &api_key[..3], &api_key[api_key.len() - 4..])
            } else {
                "sk-****".to_string()
            };
            let req_meta = serde_json::json!({
                "url": &url,
                "model": model,
                "source_image_count": source_images.len(),
                "size": &options.size,
                "quality": &options.quality,
                "background": &options.background,
                "output_format": &options.output_format,
                "output_compression": options.output_compression,
                "moderation": &options.moderation,
                "image_count": options.image_count,
                "api_key": masked_key,
            });
            let _ = db.insert_log(
                "api_request",
                "info",
                &format!(
                    "POST {} — Gemini model: {}, source_images: {}",
                    url,
                    model,
                    source_images.len()
                ),
                Some(generation_id),
                Some(&req_meta.to_string()),
                None,
            );
        }

        if api_key.is_empty() {
            return Err("API key not set".to_string());
        }

        let mut images = Vec::with_capacity(options.image_count as usize);
        let mut response_files = Vec::new();

        while let Some(batch_count) = missing_image_count(options.image_count, images.len()) {
            let mut batch_options = options.clone();
            batch_options.image_count = batch_count;
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
            let request_body = gemini::build_request_body(prompt, &inline_images, &batch_options);

            let mut last_error = None;
            let mut batch_images = None;
            let mut batch_response_file = None;

            for attempt in 0..=self.max_retries {
                let response = self
                    .client
                    .post(&url)
                    .header("x-goog-api-key", api_key)
                    .header("Content-Type", "application/json")
                    .json(&request_body)
                    .send()
                    .await;

                let response = match response {
                    Ok(response) => response,
                    Err(e) => {
                        let error = gemini::augment_transport_error(
                            model,
                            &self.format_request_error(&url, &e, self.timeout_secs),
                        );
                        if let Some(db) = db {
                            let _ = db.insert_log(
                                "api_response",
                                "error",
                                &error,
                                Some(generation_id),
                                Some(
                                    &serde_json::json!({
                                        "attempt": attempt + 1,
                                        "max_retries": self.max_retries
                                    })
                                    .to_string(),
                                ),
                                None,
                            );
                        }
                        if attempt < self.max_retries {
                            last_error = Some(error);
                            continue;
                        }
                        return Err(error);
                    }
                };

                let status = response.status();
                let body_text = response
                    .text()
                    .await
                    .map_err(|e| format!("Read body failed: {}", e))?;

                if !status.is_success() {
                    let response_file = self.write_response_body(log_dir, &body_text);
                    let error = format!("API error {} from {}: {}", status, url, body_text);
                    if let Some(db) = db {
                        let _ = db.insert_log(
                            "api_response",
                            "error",
                            &error,
                            Some(generation_id),
                            Some(
                                &serde_json::json!({
                                    "url": &url,
                                    "status": status.as_u16(),
                                    "body_size": body_text.len(),
                                    "body_preview": Self::preview_text(&body_text, 500),
                                })
                                .to_string(),
                            ),
                            response_file.as_deref(),
                        );
                    }
                    if status.is_server_error() && attempt < self.max_retries {
                        last_error = Some(error);
                        continue;
                    }
                    return Err(error);
                }

                batch_response_file = self.log_response_body(
                    db,
                    log_dir,
                    Some(generation_id),
                    status.as_u16(),
                    &body_text,
                );
                let decoded_images = self.decode_images_from_response(&body_text).await?;
                batch_images = Some(decoded_images);
                break;
            }

            let batch_images = batch_images
                .ok_or_else(|| last_error.unwrap_or_else(|| "Request failed".to_string()))?;
            if let Some(response_file) = batch_response_file {
                response_files.push(response_file);
            }

            Self::append_images_up_to_requested_count(
                &mut images,
                batch_images,
                options.image_count,
            );
        }

        let response_file = self.recoverable_response_file(log_dir, response_files, &images);
        Ok(EngineImagesResult {
            images,
            response_file,
        })
    }

    fn append_images_up_to_requested_count(
        images: &mut Vec<Vec<u8>>,
        batch_images: Vec<Vec<u8>>,
        requested_count: u8,
    ) {
        let remaining = (requested_count as usize).saturating_sub(images.len());
        images.extend(batch_images.into_iter().take(remaining));
    }

    fn recoverable_response_file(
        &self,
        log_dir: Option<&std::path::Path>,
        response_files: Vec<String>,
        images: &[Vec<u8>],
    ) -> Option<String> {
        if response_files.len() <= 1 {
            return response_files.into_iter().next();
        }

        let data = images
            .iter()
            .map(|bytes| {
                serde_json::json!({
                    "b64_json": base64::Engine::encode(
                        &base64::engine::general_purpose::STANDARD,
                        bytes
                    )
                })
            })
            .collect::<Vec<_>>();
        let body_text = serde_json::json!({ "data": data }).to_string();
        self.write_response_body(log_dir, &body_text)
    }

    fn format_request_error(
        &self,
        fallback_url: &str,
        error: &reqwest::Error,
        timeout_secs: Option<u64>,
    ) -> String {
        let request_url = error.url().map(|url| url.as_str()).unwrap_or(fallback_url);

        let mut reasons = Vec::new();
        if error.is_timeout() {
            match timeout_secs {
                Some(timeout_secs) => reasons.push(format!("timeout after {}s", timeout_secs)),
                None => reasons.push("request timeout".to_string()),
            }
        }
        if error.is_connect() {
            reasons.push("connection error".to_string());
        }
        if error.is_request() {
            reasons.push("request send failure".to_string());
        }
        if error.is_body() {
            reasons.push("body read/write failure".to_string());
        }
        if error.is_decode() {
            reasons.push("response decode failure".to_string());
        }
        if error.is_redirect() {
            reasons.push("redirect failure".to_string());
        }
        if let Some(status) = error.status() {
            reasons.push(format!("http {}", status));
        }

        let chain = Self::error_chain(error);
        if reasons.is_empty() {
            format!("Request failed for {}: {}", request_url, chain)
        } else {
            format!(
                "Request failed for {} [{}]: {}",
                request_url,
                reasons.join(", "),
                chain
            )
        }
    }

    fn error_chain(error: &dyn std::error::Error) -> String {
        let mut chain = Vec::new();
        let mut current = Some(error);

        while let Some(err) = current {
            let message = err.to_string();
            if !message.is_empty() && chain.last() != Some(&message) {
                chain.push(message);
            }
            current = err.source();
        }

        chain.join(" <- ")
    }

    fn preview_text(value: &str, max_chars: usize) -> String {
        let preview: String = value.chars().take(max_chars).collect();
        if value.chars().count() > max_chars {
            format!("{}...", preview)
        } else {
            preview
        }
    }

    async fn download_image(&self, url: &str) -> Result<Vec<u8>, String> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }

        response
            .bytes()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|e| e.to_string())
    }

    fn log_response_body(
        &self,
        db: Option<&crate::db::Database>,
        log_dir: Option<&std::path::Path>,
        generation_id: Option<&str>,
        status: u16,
        body_text: &str,
    ) -> Option<String> {
        let response_file = self.write_response_body(log_dir, body_text);
        if let Some(db) = db {
            let resp_meta = serde_json::json!({
                "status": status,
                "body_size": body_text.len(),
            });
            let _ = db.insert_log(
                "api_response",
                "info",
                &format!("{} — {} bytes", status, body_text.len()),
                generation_id,
                Some(&resp_meta.to_string()),
                response_file.as_deref(),
            );

            return response_file;
        }

        None
    }

    fn write_response_body(
        &self,
        log_dir: Option<&std::path::Path>,
        body_text: &str,
    ) -> Option<String> {
        let dir = log_dir?;
        let logs_dir = dir.join("logs").join("responses");
        let _ = std::fs::create_dir_all(&logs_dir);
        let ts = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let filename = format!("{}_{}.json", ts, chrono::Local::now().timestamp_millis());
        let path = logs_dir.join(&filename);
        let _ = std::fs::write(&path, body_text);
        Some(path.to_string_lossy().to_string())
    }

    pub(crate) async fn decode_images_from_response(
        &self,
        body_text: &str,
    ) -> Result<Vec<Vec<u8>>, String> {
        if let Ok(api_response) = serde_json::from_str::<OpenAiImageResponse>(body_text) {
            let mut images = Vec::new();
            for data in &api_response.data {
                if let Some(b64) = &data.b64_json {
                    let bytes =
                        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
                            .map_err(|e| format!("Base64 decode failed: {}", e))?;
                    images.push(bytes);
                    continue;
                }

                if let Some(image_url) = &data.url {
                    let bytes = self
                        .download_image(image_url)
                        .await
                        .map_err(|e| format!("Download image failed: {}", e))?;
                    images.push(bytes);
                }
            }

            if !images.is_empty() {
                return Ok(images);
            }
        }

        let value: Value = serde_json::from_str(body_text).map_err(|e| {
            format!(
                "Parse response failed: {}. Body: {}",
                e,
                &body_text[..body_text.len().min(300)]
            )
        })?;

        gemini::parse_images(&value).map_err(|error| {
            format!(
                "{}. Response: {}",
                error,
                &body_text[..body_text.len().min(500)]
            )
        })
    }

    async fn prepare_edit_images(
        &self,
        source_image_paths: &[String],
    ) -> Result<Vec<PreparedEditImage>, String> {
        let mut prepared = Vec::with_capacity(source_image_paths.len());

        for path in source_image_paths {
            let bytes = tokio::fs::read(path)
                .await
                .map_err(|e| format!("Read source image failed ({}): {}", path, e))?;
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
        &self,
        model: &str,
        prompt: &str,
        options: &GptImageRequestOptions,
        source_images: &[PreparedEditImage],
    ) -> Result<Form, String> {
        let mut form = Form::new();
        for (key, value) in openai::build_edit_text_fields(model, prompt, options) {
            form = form.text(key, value);
        }

        for image in source_images {
            let part = Part::bytes(image.bytes.clone())
                .file_name(image.file_name.clone())
                .mime_str(&image.mime_type)
                .map_err(|e| e.to_string())?;

            form = form.part(openai::edit_image_part_field_name(), part);
        }

        Ok(form)
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

    #[test]
    fn missing_image_count_requests_only_the_remaining_images() {
        assert_eq!(missing_image_count(2, 0), Some(2));
        assert_eq!(missing_image_count(2, 1), Some(1));
        assert_eq!(missing_image_count(2, 2), None);
        assert_eq!(missing_image_count(2, 3), None);
    }
}
