use crate::models::*;

pub(crate) fn normalize_image_model(model: &str) -> &'static str {
    match model {
        ENGINE_GPT_IMAGE_2 => ENGINE_GPT_IMAGE_2,
        ENGINE_GPT_IMAGE_SUNBURST => ENGINE_GPT_IMAGE_SUNBURST,
        ENGINE_GPT_IMAGE_FLARE => ENGINE_GPT_IMAGE_FLARE,
        ENGINE_NANO_BANANA_21 => ENGINE_NANO_BANANA_21,
        ENGINE_GROK => ENGINE_GROK,
        ENGINE_SEEDREAM_PRO => ENGINE_SEEDREAM_PRO,
        ENGINE_SEEDREAM_FLASH => ENGINE_SEEDREAM_FLASH,
        ENGINE_NANO_BANANA | GEMINI_MODEL_NANO_BANANA => ENGINE_NANO_BANANA,
        ENGINE_NANO_BANANA_2 | GEMINI_MODEL_NANO_BANANA_2 | "gemini-3.1-flash-image-preview" => {
            ENGINE_NANO_BANANA_2
        }
        ENGINE_NANO_BANANA_PRO | GEMINI_MODEL_NANO_BANANA_PRO | "gemini-3-pro-image-preview" => {
            ENGINE_NANO_BANANA_PRO
        }
        _ => DEFAULT_IMAGE_MODEL,
    }
}

pub(crate) fn is_gemini_model(model: &str) -> bool {
    matches!(
        normalize_image_model(model),
        ENGINE_NANO_BANANA | ENGINE_NANO_BANANA_2 | ENGINE_NANO_BANANA_PRO | ENGINE_NANO_BANANA_21
    )
}

fn gemini_provider_model_id(model: &str) -> &'static str {
    match normalize_image_model(model) {
        ENGINE_NANO_BANANA_21 => ENGINE_NANO_BANANA_21,
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
        if model != ENGINE_NANO_BANANA_21 {
            options.output_format = DEFAULT_OUTPUT_FORMAT.to_string();
        }
        options.output_compression = DEFAULT_OUTPUT_COMPRESSION;
        options.moderation = DEFAULT_IMAGE_MODERATION.to_string();
        options.input_fidelity = DEFAULT_INPUT_FIDELITY.to_string();
    }

    if is_gemini_model(model) || is_seedream_model(model) {
        options.image_count = 1;
    }
    if is_seedream_model(model) || model == ENGINE_GROK {
        options.background = DEFAULT_IMAGE_BACKGROUND.to_string();
        options.moderation = DEFAULT_IMAGE_MODERATION.to_string();
        options.input_fidelity = DEFAULT_INPUT_FIDELITY.to_string();
        options.stream = false;
        options.partial_images = 0;
    }
    if model == ENGINE_GROK {
        options.output_format = "jpeg".to_string();
    }
    if is_seedream_model(model) {
        options.quality = DEFAULT_IMAGE_QUALITY.to_string();
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

    if model == ENGINE_NANO_BANANA_21 {
        let url = "https://generativelanguage.googleapis.com/v1beta/interactions";
        return EndpointSettings {
            mode: ENDPOINT_MODE_BASE_URL.to_string(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            generation_url: url.to_string(),
            edit_url: url.to_string(),
        };
    }
    if model == ENGINE_GROK || is_seedream_model(model) {
        let base = if model == ENGINE_GROK {
            "https://api.x.ai/v1"
        } else {
            "https://ark.cn-beijing.volces.com/api/v3"
        };
        return EndpointSettings {
            mode: ENDPOINT_MODE_BASE_URL.to_string(),
            base_url: base.to_string(),
            generation_url: build_image_endpoint_url(base, ImageEndpointKind::Generate),
            edit_url: build_image_endpoint_url(
                base,
                if is_seedream_model(model) {
                    ImageEndpointKind::Generate
                } else {
                    ImageEndpointKind::Edit
                },
            ),
        };
    }
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
        ENGINE_NANO_BANANA_2 => &["gemini-3.1-flash-image-preview", GEMINI_MODEL_NANO_BANANA_2],
        ENGINE_NANO_BANANA_PRO => &["gemini-3-pro-image-preview", GEMINI_MODEL_NANO_BANANA_PRO],
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

    if model == ENGINE_NANO_BANANA_21 {
        return if settings.mode == ENDPOINT_MODE_FULL_URL {
            match kind {
                ImageEndpointKind::Generate => settings.generation_url.clone(),
                ImageEndpointKind::Edit => settings.edit_url.clone(),
            }
        } else {
            format!("{}/interactions", settings.base_url.trim_end_matches('/'))
        };
    }
    if is_seedream_model(model) {
        return if settings.mode == ENDPOINT_MODE_FULL_URL {
            settings.generation_url.clone()
        } else {
            build_image_endpoint_url(&settings.base_url, ImageEndpointKind::Generate)
        };
    }
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
            return endpoint;
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

pub(crate) fn is_seedream_model(model: &str) -> bool {
    matches!(model, ENGINE_SEEDREAM_PRO | ENGINE_SEEDREAM_FLASH)
}

pub(crate) fn migrate_official_gemini_url(url: &str) -> String {
    for (old, new) in [
        ("gemini-3.1-flash-image-preview", GEMINI_MODEL_NANO_BANANA_2),
        ("gemini-3-pro-image-preview", GEMINI_MODEL_NANO_BANANA_PRO),
    ] {
        let old_url = format!("{DEFAULT_GEMINI_MODELS_URL}/{old}:generateContent");
        if url == old_url {
            return format!("{DEFAULT_GEMINI_MODELS_URL}/{new}:generateContent");
        }
    }
    url.to_string()
}

pub(crate) fn validate_request_options(
    model: &str,
    options: &GptImageRequestOptions,
    source_count: usize,
) -> Result<(), crate::error::AppError> {
    use crate::image_engines::{provider_for_model, ImageProvider};
    let invalid = || crate::error::AppError::Validation {
        message: "Unsupported image parameters for the selected model.".to_string(),
    };
    let provider = provider_for_model(model);
    let max_sources = match provider {
        ImageProvider::Xai => 5,
        ImageProvider::Seedream => 10,
        ImageProvider::Gemini => 14,
        ImageProvider::OpenAi => 16,
        ImageProvider::Unknown => return Err(invalid()),
    };
    if source_count > max_sources {
        return Err(crate::error::AppError::Validation {
            message: format!("This model supports at most {max_sources} reference images."),
        });
    }
    let quality_ok = match provider {
        ImageProvider::OpenAi
            if matches!(model, ENGINE_GPT_IMAGE_FLARE | ENGINE_GPT_IMAGE_SUNBURST) =>
        {
            ["auto", "low", "medium", "high", "xhigh", "max"].contains(&options.quality.as_str())
        }
        ImageProvider::OpenAi => {
            ["auto", "low", "medium", "high"].contains(&options.quality.as_str())
        }
        ImageProvider::Xai => ["auto", "low", "medium"].contains(&options.quality.as_str()),
        _ => options.quality == "auto",
    };
    if !quality_ok
        || !["auto", "opaque", "transparent"].contains(&options.background.as_str())
        || !["png", "jpeg", "webp"].contains(&options.output_format.as_str())
        || (options.background == "transparent" && options.output_format == "jpeg")
        || (provider == ImageProvider::Seedream && options.output_format == "webp")
    {
        return Err(invalid());
    }
    if let Some(ratio) = &options.aspect_ratio {
        let supported = [
            "19.5:9", "9:19.5", "20:9", "9:20", "1:1", "1:4", "4:1", "1:8", "8:1", "1:2", "2:1",
            "2:3", "3:2", "3:4", "4:3", "4:5", "5:4", "9:16", "16:9", "21:9",
        ];
        if !supported.contains(&ratio.as_str())
            || provider == ImageProvider::OpenAi
            || provider == ImageProvider::Seedream
            || (model != ENGINE_NANO_BANANA_21
                && ["1:4", "4:1", "1:8", "8:1"].contains(&ratio.as_str()))
            || (provider == ImageProvider::Xai && ["4:5", "5:4", "21:9"].contains(&ratio.as_str()))
            || (provider == ImageProvider::Gemini
                && ["19.5:9", "9:19.5", "20:9", "9:20"].contains(&ratio.as_str()))
        {
            return Err(invalid());
        }
    }
    if let Some(resolution) = &options.resolution {
        let allowed: &[&str] = match provider {
            ImageProvider::Xai => &["1K", "2K"],
            ImageProvider::Seedream => &["auto", "1K", "1.5K", "2K"],
            ImageProvider::Gemini if model == ENGINE_NANO_BANANA => &["1K"],
            ImageProvider::Gemini if model == ENGINE_NANO_BANANA_2 => &["512", "1K", "2K", "4K"],
            ImageProvider::Gemini => &["1K", "2K", "4K"],
            _ => &[],
        };
        if !allowed.contains(&resolution.as_str()) {
            return Err(invalid());
        }
    }
    if provider == ImageProvider::OpenAi && options.size != "auto" {
        let (w, h) = options
            .size
            .split_once('x')
            .and_then(|(w, h)| Some((w.parse::<u64>().ok()?, h.parse::<u64>().ok()?)))
            .ok_or_else(invalid)?;
        if w == 0
            || h == 0
            || w > 3840
            || h > 3840
            || w % 16 != 0
            || h % 16 != 0
            || w.max(h) > w.min(h) * 3
            || !(655360..=8294400).contains(&(w * h))
        {
            return Err(invalid());
        }
    }
    Ok(())
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
                aspect_ratio: None,
                resolution: None,
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
        assert_eq!(options.image_count, 1);
        assert_eq!(options.quality, DEFAULT_IMAGE_QUALITY);
        assert_eq!(options.background, DEFAULT_IMAGE_BACKGROUND);
        assert_eq!(options.output_format, DEFAULT_OUTPUT_FORMAT);
        assert_eq!(options.output_compression, DEFAULT_OUTPUT_COMPRESSION);
        assert_eq!(options.moderation, DEFAULT_IMAGE_MODERATION);
        assert_eq!(options.input_fidelity, DEFAULT_INPUT_FIDELITY);
    }

    #[test]
    fn latest_models_keep_distinct_routing_endpoints_and_validation() {
        use crate::image_engines::{provider_for_model, ImageProvider};
        assert_eq!(provider_for_model(ENGINE_GROK), ImageProvider::Xai);
        assert_eq!(
            provider_for_model(ENGINE_SEEDREAM_PRO),
            ImageProvider::Seedream
        );
        assert_eq!(
            provider_for_model("future-unregistered-model"),
            ImageProvider::Unknown
        );
        assert_eq!(
            normalize_image_model("gemini-3-pro-image-preview"),
            ENGINE_NANO_BANANA_PRO
        );
        for model in [
            ENGINE_NANO_BANANA_21,
            ENGINE_GROK,
            ENGINE_SEEDREAM_PRO,
            ENGINE_SEEDREAM_FLASH,
        ] {
            let defaults = default_endpoint_settings_for_model(model);
            assert_eq!(
                image_endpoint_url_for_model_settings(
                    model,
                    &defaults,
                    ImageEndpointKind::Generate
                ),
                defaults.generation_url
            );
            assert_eq!(
                image_endpoint_url_for_model_settings(model, &defaults, ImageEndpointKind::Edit),
                defaults.edit_url
            );
        }
        let mut options = GptImageRequestOptions {
            aspect_ratio: Some("16:9".to_string()),
            resolution: Some("2K".to_string()),
            size: "auto".into(),
            quality: "medium".into(),
            background: "auto".into(),
            output_format: "jpeg".into(),
            output_compression: 100,
            moderation: "auto".into(),
            input_fidelity: "high".into(),
            stream: false,
            partial_images: 0,
            image_count: 1,
        };
        assert!(validate_request_options(ENGINE_GROK, &options, 5).is_ok());
        assert!(validate_request_options(ENGINE_GROK, &options, 6).is_err());
        options.quality = "max".into();
        assert!(validate_request_options(ENGINE_GROK, &options, 1).is_err());
        options.aspect_ratio = None;
        options.resolution = None;
        assert!(validate_request_options(ENGINE_GPT_IMAGE_FLARE, &options, 1).is_ok());
        assert!(validate_request_options(ENGINE_GPT_IMAGE_2, &options, 1).is_err());
        options.size = "999999999999999999999999x2".into();
        assert!(validate_request_options(ENGINE_GPT_IMAGE_FLARE, &options, 1).is_err());
    }
}
