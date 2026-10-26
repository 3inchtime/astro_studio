//! JSON protocols that resemble Images API responses but have distinct request contracts.
use crate::api_gateway::PreparedEditImage;
use crate::models::*;
use serde_json::{json, Value};

pub(crate) fn data_uri(image: &PreparedEditImage) -> String {
    format!(
        "data:{};base64,{}",
        image.mime_type(),
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, image.bytes())
    )
}

pub(crate) fn build_request(
    model: &str,
    prompt: &str,
    sources: &[PreparedEditImage],
    options: &GptImageRequestOptions,
) -> Value {
    if model == ENGINE_NANO_BANANA_21 {
        let mut input = vec![json!({"type": "text", "text": prompt})];
        input.extend(sources.iter().map(|image| json!({"type":"image", "mime_type": image.mime_type(), "data": base64::Engine::encode(&base64::engine::general_purpose::STANDARD, image.bytes())})));
        let mut format =
            json!({"type":"image", "mime_type": format!("image/{}", options.output_format)});
        if let Some(ratio) = &options.aspect_ratio {
            format["aspect_ratio"] = json!(ratio);
        }
        if let Some(resolution) = &options.resolution {
            format["image_size"] = json!(resolution);
        }
        return json!({"model":model, "input":input, "response_format":format, "store":false});
    }
    if model == ENGINE_GROK {
        let mut body = json!({"model":model, "prompt":prompt, "n":options.image_count, "quality":options.quality, "response_format":"b64_json"});
        if let Some(ratio) = &options.aspect_ratio {
            body["aspect_ratio"] = json!(ratio);
        }
        if let Some(resolution) = &options.resolution {
            body["resolution"] = json!(resolution.to_ascii_lowercase());
        }
        match sources.len() {
            0 => {}
            1 => body["image"] = json!({"url":data_uri(&sources[0]), "type":"image_url"}),
            _ => {
                body["images"] = json!(sources
                    .iter()
                    .map(|image| json!({"url":data_uri(image), "type":"image_url"}))
                    .collect::<Vec<_>>())
            }
        }
        return body;
    }
    let mut body = json!({"model":model, "prompt":prompt, "size":options.resolution.as_deref().unwrap_or("2K"), "response_format":"b64_json", "output_format":options.output_format});
    if !sources.is_empty() {
        body["image"] = json!(sources.iter().map(data_uri).collect::<Vec<_>>());
    }
    body
}

/// Interactions returns raw model_output steps; SDK output_image is a convenience getter.
pub(crate) fn parse_interaction_images(
    response: &Value,
    is_cancelled: &(dyn Fn() -> bool + Send + Sync),
) -> Result<Vec<Vec<u8>>, String> {
    if response
        .get("status")
        .and_then(Value::as_str)
        .is_some_and(|status| status != "completed")
    {
        return Err("Image interaction did not complete".to_string());
    }
    let mut images = Vec::new();
    if let Some(steps) = response.get("steps").and_then(Value::as_array) {
        for step in steps {
            if step.get("type").and_then(Value::as_str) != Some("model_output") {
                continue;
            }
            if let Some(content) = step.get("content").and_then(Value::as_array) {
                for part in content {
                    if is_cancelled() {
                        return Err("Image response decoding was cancelled".to_string());
                    }
                    if part.get("type").and_then(Value::as_str) != Some("image")
                        || part.get("is_thought").and_then(Value::as_bool) == Some(true)
                    {
                        continue;
                    }
                    if let Some(data) = part.get("data").and_then(Value::as_str) {
                        let bytes = base64::Engine::decode(
                            &base64::engine::general_purpose::STANDARD,
                            data,
                        )
                        .map_err(|_| "Invalid image data".to_string())?;
                        images.push(bytes);
                    }
                }
            }
        }
    }
    if images.is_empty() {
        return Err("Image interaction did not include an image".to_string());
    }
    Ok(images)
}
