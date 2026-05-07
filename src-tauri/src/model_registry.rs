use crate::models::*;

pub(crate) fn normalize_image_model(model: &str) -> &'static str {
    match model {
        ENGINE_GPT_IMAGE_2 => ENGINE_GPT_IMAGE_2,
        ENGINE_NANO_BANANA | GEMINI_MODEL_NANO_BANANA => ENGINE_NANO_BANANA,
        ENGINE_NANO_BANANA_2 | GEMINI_MODEL_NANO_BANANA_2 => ENGINE_NANO_BANANA_2,
        ENGINE_NANO_BANANA_PRO | GEMINI_MODEL_NANO_BANANA_PRO => ENGINE_NANO_BANANA_PRO,
        _ => DEFAULT_IMAGE_MODEL,
    }
}

pub(crate) fn is_gemini_model(model: &str) -> bool {
    matches!(
        normalize_image_model(model),
        ENGINE_NANO_BANANA | ENGINE_NANO_BANANA_2 | ENGINE_NANO_BANANA_PRO
    )
}

fn gemini_provider_model_id(model: &str) -> &'static str {
    match normalize_image_model(model) {
        ENGINE_NANO_BANANA => GEMINI_MODEL_NANO_BANANA,
        ENGINE_NANO_BANANA_2 => GEMINI_MODEL_NANO_BANANA_2,
        ENGINE_NANO_BANANA_PRO => GEMINI_MODEL_NANO_BANANA_PRO,
        _ => GEMINI_MODEL_NANO_BANANA,
    }
}

pub(crate) fn sanitize_request_options_for_model(
    model: &str,
    mut options: GptImageRequestOptions,
) -> GptImageRequestOptions {
    if is_gemini_model(model) {
        options.quality = DEFAULT_IMAGE_QUALITY.to_string();
        options.background = DEFAULT_IMAGE_BACKGROUND.to_string();
        options.output_format = DEFAULT_OUTPUT_FORMAT.to_string();
        options.output_compression = DEFAULT_OUTPUT_COMPRESSION;
        options.moderation = DEFAULT_IMAGE_MODERATION.to_string();
        options.input_fidelity = DEFAULT_INPUT_FIDELITY.to_string();
    }

    options
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ImageEndpointKind {
    Generate,
    Edit,
}

pub(crate) fn normalize_endpoint_mode(mode: &str) -> &'static str {
    match mode {
        ENDPOINT_MODE_FULL_URL => ENDPOINT_MODE_FULL_URL,
        _ => ENDPOINT_MODE_BASE_URL,
    }
}

pub(crate) fn endpoint_value_or_default(value: Option<String>, default_value: &str) -> String {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_value.to_string())
}

fn build_image_endpoint_url(base_url: &str, kind: ImageEndpointKind) -> String {
    let path = match kind {
        ImageEndpointKind::Generate => "images/generations",
        ImageEndpointKind::Edit => "images/edits",
    };
    format!("{}/{}", base_url.trim_end_matches('/'), path)
}

pub(crate) fn default_endpoint_settings_for_model(model: &str) -> EndpointSettings {
    let model = normalize_image_model(model);

    if is_gemini_model(model) {
        let provider_model = gemini_provider_model_id(model);
        let generation_url =
            format!("{DEFAULT_GEMINI_MODELS_URL}/{provider_model}:generateContent");
        return EndpointSettings {
            mode: ENDPOINT_MODE_BASE_URL.to_string(),
            base_url: DEFAULT_GEMINI_MODELS_URL.to_string(),
            generation_url: generation_url.clone(),
            edit_url: generation_url,
        };
    }

    EndpointSettings {
        mode: ENDPOINT_MODE_BASE_URL.to_string(),
        base_url: DEFAULT_BASE_URL.to_string(),
        generation_url: DEFAULT_GENERATION_URL.to_string(),
        edit_url: DEFAULT_EDIT_URL.to_string(),
    }
}

pub(crate) fn model_setting_key(model: &str, suffix: &str) -> String {
    format!("model_config::{}::{}", normalize_image_model(model), suffix)
}

pub(crate) fn legacy_model_setting_ids(model: &str) -> &'static [&'static str] {
    match normalize_image_model(model) {
        ENGINE_NANO_BANANA => &[GEMINI_MODEL_NANO_BANANA],
        ENGINE_NANO_BANANA_2 => &[GEMINI_MODEL_NANO_BANANA_2],
        ENGINE_NANO_BANANA_PRO => &[GEMINI_MODEL_NANO_BANANA_PRO],
        _ => &[],
    }
}

fn normalize_gemini_endpoint_url(endpoint_url: &str, model: &str) -> String {
    let endpoint = endpoint_url.trim().trim_end_matches('/');
    let model = gemini_provider_model_id(model);

    if endpoint.ends_with(":generateContent") {
        endpoint.to_string()
    } else if endpoint.ends_with(model) {
        format!("{endpoint}:generateContent")
    } else {
        format!("{endpoint}/{model}:generateContent")
    }
}

pub(crate) fn image_endpoint_url_for_model_settings(
    model: &str,
    settings: &EndpointSettings,
    kind: ImageEndpointKind,
) -> String {
    let model = normalize_image_model(model);

    if settings.mode == ENDPOINT_MODE_FULL_URL {
        if is_gemini_model(model) {
            let endpoint = match kind {
                ImageEndpointKind::Generate => settings.generation_url.clone(),
                ImageEndpointKind::Edit => {
                    if settings.edit_url.trim().is_empty() {
                        settings.generation_url.clone()
                    } else {
                        settings.edit_url.clone()
                    }
                }
            };
            return normalize_gemini_endpoint_url(&endpoint, model);
        }

        return match kind {
            ImageEndpointKind::Generate => settings.generation_url.clone(),
            ImageEndpointKind::Edit => settings.edit_url.clone(),
        };
    }

    if is_gemini_model(model) {
        return normalize_gemini_endpoint_url(&settings.base_url, model);
    }

    build_image_endpoint_url(&settings.base_url, kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_url_mode_builds_image_endpoint_paths() {
        assert_eq!(
            build_image_endpoint_url("https://api.example.test/v1/", ImageEndpointKind::Generate),
            "https://api.example.test/v1/images/generations"
        );
        assert_eq!(
            build_image_endpoint_url("https://api.example.test/v1", ImageEndpointKind::Edit),
            "https://api.example.test/v1/images/edits"
        );
    }

    #[test]
    fn endpoint_mode_normalizes_to_supported_values() {
        assert_eq!(
            normalize_endpoint_mode(ENDPOINT_MODE_FULL_URL),
            ENDPOINT_MODE_FULL_URL
        );
        assert_eq!(
            normalize_endpoint_mode("unsupported"),
            ENDPOINT_MODE_BASE_URL
        );
    }

    #[test]
    fn full_url_mode_uses_separate_generation_and_edit_urls() {
        let settings = EndpointSettings {
            mode: ENDPOINT_MODE_FULL_URL.to_string(),
            base_url: "https://unused.example.test/v1".to_string(),
            generation_url: "https://gateway.example.test/create".to_string(),
            edit_url: "https://gateway.example.test/edit".to_string(),
        };

        assert_eq!(
            image_endpoint_url_for_model_settings(
                ENGINE_GPT_IMAGE_2,
                &settings,
                ImageEndpointKind::Generate,
            ),
            "https://gateway.example.test/create"
        );
        assert_eq!(
            image_endpoint_url_for_model_settings(
                ENGINE_GPT_IMAGE_2,
                &settings,
                ImageEndpointKind::Edit,
            ),
            "https://gateway.example.test/edit"
        );
    }

    #[test]
    fn normalize_image_model_accepts_gemini_nanobanana_models() {
        assert_eq!(
            normalize_image_model(ENGINE_NANO_BANANA),
            ENGINE_NANO_BANANA
        );
        assert_eq!(
            normalize_image_model(GEMINI_MODEL_NANO_BANANA),
            ENGINE_NANO_BANANA
        );
        assert_eq!(
            normalize_image_model(ENGINE_NANO_BANANA_PRO),
            ENGINE_NANO_BANANA_PRO
        );
        assert_eq!(
            normalize_image_model(ENGINE_NANO_BANANA_2),
            ENGINE_NANO_BANANA_2
        );
        assert_eq!(
            normalize_image_model(GEMINI_MODEL_NANO_BANANA_2),
            ENGINE_NANO_BANANA_2
        );
        assert_eq!(
            normalize_image_model(GEMINI_MODEL_NANO_BANANA_PRO),
            ENGINE_NANO_BANANA_PRO
        );
    }

    #[test]
    fn gemini_base_url_mode_builds_generate_content_endpoint() {
        let settings = EndpointSettings {
            mode: ENDPOINT_MODE_BASE_URL.to_string(),
            base_url: DEFAULT_GEMINI_MODELS_URL.to_string(),
            generation_url: String::new(),
            edit_url: String::new(),
        };

        assert_eq!(
            image_endpoint_url_for_model_settings(
                ENGINE_NANO_BANANA,
                &settings,
                ImageEndpointKind::Generate,
            ),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent"
        );
        assert_eq!(
            image_endpoint_url_for_model_settings(
                ENGINE_NANO_BANANA,
                &settings,
                ImageEndpointKind::Edit,
            ),
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent"
        );
    }

    #[test]
    fn gemini_models_drop_unsupported_request_controls() {
        let options = sanitize_request_options_for_model(
            ENGINE_NANO_BANANA,
            GptImageRequestOptions {
                size: "1536x1024".to_string(),
                quality: "high".to_string(),
                background: "transparent".to_string(),
                output_format: "webp".to_string(),
                output_compression: 75,
                moderation: "low".to_string(),
                input_fidelity: "low".to_string(),
                stream: false,
                partial_images: 0,
                image_count: 3,
            },
        );

        assert_eq!(options.size, "1536x1024");
        assert_eq!(options.image_count, 3);
        assert_eq!(options.quality, DEFAULT_IMAGE_QUALITY);
        assert_eq!(options.background, DEFAULT_IMAGE_BACKGROUND);
        assert_eq!(options.output_format, DEFAULT_OUTPUT_FORMAT);
        assert_eq!(options.output_compression, DEFAULT_OUTPUT_COMPRESSION);
        assert_eq!(options.moderation, DEFAULT_IMAGE_MODERATION);
        assert_eq!(options.input_fidelity, DEFAULT_INPUT_FIDELITY);
    }
}
