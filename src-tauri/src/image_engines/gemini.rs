use crate::model_registry::is_gemini_model;
use crate::models::*;
use serde_json::Value;

pub(crate) struct GeminiInlineImage {
    pub(crate) mime_type: String,
    pub(crate) data: String,
}

pub(crate) fn request_was_closed_before_completion(error: &str) -> bool {
    error.contains("connection closed before message completed")
}

pub(crate) fn augment_transport_error(model: &str, error: &str) -> String {
    if !is_gemini_model(model) || !request_was_closed_before_completion(error) {
        return error.to_string();
    }

    let recovery_hint = match model {
        ENGINE_NANO_BANANA_PRO | GEMINI_MODEL_NANO_BANANA_PRO => {
            "The provider closed the request before Gemini finished responding. Try switching to Nano Banana 2 or using 1024x1024 for this prompt."
        }
        _ => {
            "The provider closed the request before Gemini finished responding. Try using 1024x1024 or a simpler prompt."
        }
    };

    format!("{error}\n\n{recovery_hint}")
}

pub(crate) fn build_request_body(
    prompt: &str,
    inline_images: &[GeminiInlineImage],
    options: &GptImageRequestOptions,
) -> Value {
    let mut parts = vec![serde_json::json!({ "text": prompt })];
    for image in inline_images {
        parts.push(serde_json::json!({
            "inlineData": {
                "mimeType": image.mime_type,
                "data": image.data,
            }
        }));
    }

    let mut generation_config = serde_json::Map::new();
    generation_config.insert(
        "responseModalities".to_string(),
        serde_json::json!(["IMAGE"]),
    );
    generation_config.insert(
        "candidateCount".to_string(),
        serde_json::json!(options.image_count),
    );

    let mut image_config = serde_json::Map::new();
    if let Some(aspect_ratio) = aspect_ratio_for_size(&options.size) {
        image_config.insert("aspectRatio".to_string(), serde_json::json!(aspect_ratio));
    }
    if !image_config.is_empty() {
        generation_config.insert("imageConfig".to_string(), Value::Object(image_config));
    }

    serde_json::json!({
        "contents": [{ "parts": parts }],
        "generationConfig": Value::Object(generation_config),
    })
}

pub(crate) fn parse_images(response: &Value) -> Result<Vec<Vec<u8>>, String> {
    let mut images = Vec::new();

    if let Some(candidates) = response.get("candidates").and_then(Value::as_array) {
        for candidate in candidates {
            if let Some(parts) = candidate
                .get("content")
                .and_then(|content| content.get("parts"))
                .and_then(Value::as_array)
            {
                for part in parts {
                    if let Some(data) = part
                        .get("inlineData")
                        .and_then(|inline| inline.get("data"))
                        .and_then(Value::as_str)
                    {
                        let bytes = base64::Engine::decode(
                            &base64::engine::general_purpose::STANDARD,
                            data,
                        )
                        .map_err(|e| format!("Base64 decode failed: {}", e))?;
                        images.push(bytes);
                    }
                }
            }
        }
    }

    if images.is_empty() {
        return Err("Gemini response did not include any image data".to_string());
    }

    Ok(images)
}

fn aspect_ratio_for_size(size: &str) -> Option<&'static str> {
    match size {
        "1024x1024" => Some("1:1"),
        "1536x1024" => Some("3:2"),
        "1024x1536" => Some("2:3"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use serde_json::json;

    #[test]
    fn builds_generate_request_body() {
        let body = build_request_body(
            "Draw a striped glass tiger",
            &[],
            &GptImageRequestOptions {
                size: DEFAULT_IMAGE_SIZE.to_string(),
                quality: DEFAULT_IMAGE_QUALITY.to_string(),
                background: DEFAULT_IMAGE_BACKGROUND.to_string(),
                output_format: DEFAULT_OUTPUT_FORMAT.to_string(),
                output_compression: DEFAULT_OUTPUT_COMPRESSION,
                moderation: DEFAULT_IMAGE_MODERATION.to_string(),
                input_fidelity: DEFAULT_INPUT_FIDELITY.to_string(),
                stream: DEFAULT_IMAGE_STREAM,
                partial_images: DEFAULT_PARTIAL_IMAGES,
                image_count: 2,
            },
        );

        assert_eq!(
            body["contents"][0]["parts"][0]["text"],
            "Draw a striped glass tiger"
        );
        assert_eq!(body["generationConfig"]["candidateCount"], 2);
        assert_eq!(
            body["generationConfig"]["responseModalities"],
            json!(["IMAGE"])
        );
        assert!(body["generationConfig"]["imageConfig"]
            .get("outputMimeType")
            .is_none());
    }

    #[test]
    fn builds_edit_request_body_with_inline_images_and_aspect_ratio() {
        let body = build_request_body(
            "Use these references for a clean product render",
            &[GeminiInlineImage {
                mime_type: "image/webp".to_string(),
                data: "cmVmZXJlbmNlLWJ5dGVz".to_string(),
            }],
            &GptImageRequestOptions {
                size: "1536x1024".to_string(),
                quality: "ignored-for-gemini".to_string(),
                background: "ignored-for-gemini".to_string(),
                output_format: "ignored-for-gemini".to_string(),
                output_compression: 44,
                moderation: "ignored-for-gemini".to_string(),
                input_fidelity: "ignored-for-gemini".to_string(),
                stream: true,
                partial_images: 2,
                image_count: 3,
            },
        );

        assert_eq!(
            body,
            json!({
                "contents": [{
                    "parts": [
                        { "text": "Use these references for a clean product render" },
                        {
                            "inlineData": {
                                "mimeType": "image/webp",
                                "data": "cmVmZXJlbmNlLWJ5dGVz",
                            }
                        }
                    ]
                }],
                "generationConfig": {
                    "responseModalities": ["IMAGE"],
                    "candidateCount": 3,
                    "imageConfig": {
                        "aspectRatio": "3:2",
                    }
                },
            })
        );
    }

    #[test]
    fn request_body_omits_image_config_for_unknown_size() {
        let body = build_request_body(
            "Draw a square icon",
            &[],
            &GptImageRequestOptions {
                size: "auto".to_string(),
                quality: DEFAULT_IMAGE_QUALITY.to_string(),
                background: DEFAULT_IMAGE_BACKGROUND.to_string(),
                output_format: DEFAULT_OUTPUT_FORMAT.to_string(),
                output_compression: DEFAULT_OUTPUT_COMPRESSION,
                moderation: DEFAULT_IMAGE_MODERATION.to_string(),
                input_fidelity: DEFAULT_INPUT_FIDELITY.to_string(),
                stream: DEFAULT_IMAGE_STREAM,
                partial_images: DEFAULT_PARTIAL_IMAGES,
                image_count: 1,
            },
        );

        assert!(body["generationConfig"].get("imageConfig").is_none());
    }

    #[test]
    fn transport_errors_include_manual_recovery_hint() {
        let message = augment_transport_error(
            ENGINE_NANO_BANANA_PRO,
            "Request failed for https://new.suxi.ai/v1beta/models/gemini-3-pro-image-preview:generateContent [request send failure]: error sending request for url (https://new.suxi.ai/v1beta/models/gemini-3-pro-image-preview:generateContent) <- client error (SendRequest) <- connection closed before message completed",
        );

        assert!(
            message.contains("The provider closed the request before Gemini finished responding.")
        );
        assert!(message.contains("Try switching to Nano Banana 2"));
    }

    #[test]
    fn transport_error_leaves_unrelated_errors_unchanged() {
        let error = "Request failed for https://api.openai.com/v1/images/generations: dns error";

        assert_eq!(augment_transport_error(ENGINE_GPT_IMAGE_2, error), error);
        assert_eq!(augment_transport_error(ENGINE_NANO_BANANA, error), error);
    }

    #[test]
    fn parses_inline_image_response() {
        let response = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "inlineData": {
                            "mimeType": "image/png",
                            "data": base64::engine::general_purpose::STANDARD.encode(b"png-bytes"),
                        }
                    }]
                }
            }]
        });

        let images = parse_images(&response).unwrap();

        assert_eq!(images, vec![b"png-bytes".to_vec()]);
    }

    #[test]
    fn parses_inline_images_from_all_candidates_and_parts() {
        let response = json!({
            "candidates": [
                {
                    "content": {
                        "parts": [
                            { "text": "intermediate text" },
                            {
                                "inlineData": {
                                    "mimeType": "image/png",
                                    "data": base64::engine::general_purpose::STANDARD.encode(b"first"),
                                }
                            }
                        ]
                    }
                },
                {
                    "content": {
                        "parts": [{
                            "inlineData": {
                                "mimeType": "image/jpeg",
                                "data": base64::engine::general_purpose::STANDARD.encode(b"second"),
                            }
                        }]
                    }
                }
            ]
        });

        let images = parse_images(&response).unwrap();

        assert_eq!(images, vec![b"first".to_vec(), b"second".to_vec()]);
    }
}
