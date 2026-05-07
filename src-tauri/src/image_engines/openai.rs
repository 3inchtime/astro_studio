use crate::models::*;
use serde_json::Value;
use std::path::Path;

pub(crate) fn build_generation_request_body(
    model: &str,
    prompt: &str,
    options: &GptImageRequestOptions,
) -> Value {
    build_generation_request_body_with_controls(
        model,
        prompt,
        &options.size,
        &options.quality,
        &options.background,
        &options.output_format,
        options.output_compression,
        &options.moderation,
        options.stream,
        options.partial_images,
        options.image_count,
    )
}

pub(crate) fn build_generation_request_body_with_controls(
    model: &str,
    prompt: &str,
    size: &str,
    quality: &str,
    background: &str,
    output_format: &str,
    output_compression: u8,
    moderation: &str,
    stream: bool,
    partial_images: u8,
    image_count: u8,
) -> Value {
    serde_json::json!({
        "model": model,
        "prompt": prompt,
        "n": image_count,
        "size": size,
        "quality": quality,
        "background": background,
        "output_format": output_format,
        "output_compression": output_compression,
        "moderation": moderation,
        "stream": stream,
        "partial_images": partial_images,
    })
}

pub(crate) fn build_edit_text_fields(
    model: &str,
    prompt: &str,
    options: &GptImageRequestOptions,
) -> Vec<(&'static str, String)> {
    build_edit_text_fields_with_controls(
        model,
        prompt,
        &options.size,
        &options.quality,
        &options.background,
        &options.input_fidelity,
        &options.output_format,
        options.output_compression,
        &options.moderation,
        options.stream,
        options.partial_images,
        options.image_count,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_edit_text_fields_with_controls(
    model: &str,
    prompt: &str,
    size: &str,
    quality: &str,
    background: &str,
    input_fidelity: &str,
    output_format: &str,
    output_compression: u8,
    moderation: &str,
    stream: bool,
    partial_images: u8,
    image_count: u8,
) -> Vec<(&'static str, String)> {
    vec![
        ("model", model.to_string()),
        ("prompt", prompt.to_string()),
        ("n", image_count.to_string()),
        ("size", size.to_string()),
        ("quality", quality.to_string()),
        ("background", background.to_string()),
        ("input_fidelity", input_fidelity.to_string()),
        ("output_format", output_format.to_string()),
        ("output_compression", output_compression.to_string()),
        ("moderation", moderation.to_string()),
        ("stream", stream.to_string()),
        ("partial_images", partial_images.to_string()),
    ]
}

pub(crate) fn image_endpoint_url(endpoint_url: &str) -> String {
    endpoint_url.trim().to_string()
}

pub(crate) fn edit_image_part_field_name() -> &'static str {
    "image"
}

pub(crate) fn mime_type_for_path(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn generation_request_body_includes_gpt_image_2_control_parameters() {
        let body = build_generation_request_body_with_controls(
            "gpt-image-2",
            "A cinematic observatory above the clouds",
            "1536x1024",
            "high",
            "auto",
            "webp",
            DEFAULT_OUTPUT_COMPRESSION,
            DEFAULT_IMAGE_MODERATION,
            DEFAULT_IMAGE_STREAM,
            DEFAULT_PARTIAL_IMAGES,
            2,
        );

        assert_eq!(
            body,
            json!({
                "model": "gpt-image-2",
                "prompt": "A cinematic observatory above the clouds",
                "n": 2,
                "size": "1536x1024",
                "quality": "high",
                "background": "auto",
                "output_format": "webp",
                "output_compression": 100,
                "moderation": "auto",
                "stream": false,
                "partial_images": 0,
            })
        );
    }

    #[test]
    fn generation_request_body_preserves_custom_control_parameters() {
        let body = build_generation_request_body_with_controls(
            "gpt-image-2",
            "Render four product colorways",
            "1024x1536",
            "medium",
            "transparent",
            "jpeg",
            72,
            "low",
            true,
            2,
            3,
        );

        assert_eq!(
            body,
            json!({
                "model": "gpt-image-2",
                "prompt": "Render four product colorways",
                "n": 3,
                "size": "1024x1536",
                "quality": "medium",
                "background": "transparent",
                "output_format": "jpeg",
                "output_compression": 72,
                "moderation": "low",
                "stream": true,
                "partial_images": 2,
            })
        );
    }

    #[test]
    fn edit_text_fields_include_gpt_image_2_control_parameters() {
        let fields = build_edit_text_fields_with_controls(
            "gpt-image-2",
            "Keep the logo crisp and make the background nocturnal",
            "1024x1024",
            "auto",
            "auto",
            "high",
            "png",
            DEFAULT_OUTPUT_COMPRESSION,
            DEFAULT_IMAGE_MODERATION,
            DEFAULT_IMAGE_STREAM,
            DEFAULT_PARTIAL_IMAGES,
            1,
        );
        let fields: HashMap<_, _> = fields.into_iter().collect();

        assert_eq!(fields.get("model").map(String::as_str), Some("gpt-image-2"));
        assert_eq!(
            fields.get("prompt").map(String::as_str),
            Some("Keep the logo crisp and make the background nocturnal")
        );
        assert_eq!(fields.get("n").map(String::as_str), Some("1"));
        assert_eq!(fields.get("size").map(String::as_str), Some("1024x1024"));
        assert_eq!(fields.get("quality").map(String::as_str), Some("auto"));
        assert_eq!(fields.get("background").map(String::as_str), Some("auto"));
        assert_eq!(
            fields.get("input_fidelity").map(String::as_str),
            Some("high")
        );
        assert_eq!(fields.get("output_format").map(String::as_str), Some("png"));
        assert_eq!(
            fields.get("output_compression").map(String::as_str),
            Some("100")
        );
        assert_eq!(fields.get("moderation").map(String::as_str), Some("auto"));
        assert_eq!(fields.get("stream").map(String::as_str), Some("false"));
        assert_eq!(fields.get("partial_images").map(String::as_str), Some("0"));
    }

    #[test]
    fn edit_text_fields_preserve_multipart_field_order_and_custom_controls() {
        let fields = build_edit_text_fields_with_controls(
            "gpt-image-2",
            "Blend the source images into one scene",
            "1536x1024",
            "high",
            "opaque",
            "low",
            "webp",
            61,
            "low",
            true,
            1,
            4,
        );

        assert_eq!(
            fields,
            vec![
                ("model", "gpt-image-2".to_string()),
                (
                    "prompt",
                    "Blend the source images into one scene".to_string()
                ),
                ("n", "4".to_string()),
                ("size", "1536x1024".to_string()),
                ("quality", "high".to_string()),
                ("background", "opaque".to_string()),
                ("input_fidelity", "low".to_string()),
                ("output_format", "webp".to_string()),
                ("output_compression", "61".to_string()),
                ("moderation", "low".to_string()),
                ("stream", "true".to_string()),
                ("partial_images", "1".to_string()),
            ]
        );
    }

    #[test]
    fn image_endpoint_urls_are_left_unchanged_for_every_base_url() {
        assert_eq!(
            image_endpoint_url("https://api.302.ai/v1/images/generations"),
            "https://api.302.ai/v1/images/generations"
        );
        assert_eq!(
            image_endpoint_url("https://api.302.ai/v1/images/edits"),
            "https://api.302.ai/v1/images/edits"
        );
        assert_eq!(
            image_endpoint_url("https://new.suxi.ai/v1/images/generations"),
            "https://new.suxi.ai/v1/images/generations"
        );
        assert_eq!(
            image_endpoint_url(
                "https://api.302.ai/v1/images/generations?response_format=b64_json&async=true"
            ),
            "https://api.302.ai/v1/images/generations?response_format=b64_json&async=true"
        );
        assert_eq!(
            image_endpoint_url("https://api.openai.com/v1/images/generations"),
            "https://api.openai.com/v1/images/generations"
        );
    }

    #[test]
    fn edit_image_parts_use_documented_image_field_name_for_every_source() {
        assert_eq!(edit_image_part_field_name(), "image");
    }
}
