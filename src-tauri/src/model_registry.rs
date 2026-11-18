use crate::model_catalog::{ImageModelSpec, ImageProtocol, CATALOG};
use crate::models::*;

pub(crate) fn image_model(model: &str) -> Option<ImageModelSpec<'static>> {
    CATALOG.image_model(model)
}

pub(crate) fn require_image_model(
    model: &str,
) -> Result<ImageModelSpec<'static>, crate::error::AppError> {
    image_model(model).ok_or_else(|| crate::error::AppError::Validation {
        message: "Unknown image model.".into(),
    })
}

pub(crate) fn normalize_image_model(model: &str) -> &'static str {
    image_model(model).map_or(CATALOG.default_image_model.as_str(), |m| m.id)
}

pub(crate) fn supports_advanced_options(model: &str) -> bool {
    image_model(model).is_some_and(|m| m.profile.advanced_options)
}

pub(crate) fn default_wire_resolution(model: &str) -> Option<String> {
    let entry = image_model(model)?;
    // Only these protocols supplied a resolution when older jobs omitted it.
    matches!(
        entry.profile.protocol,
        ImageProtocol::XaiImages | ImageProtocol::SeedreamImages
    )
    .then(|| entry.profile.parameter_defaults.resolution.clone())
    .flatten()
}

pub(crate) fn is_gemini_model(model: &str) -> bool {
    image_model(model).is_some_and(|m| {
        matches!(
            m.profile.protocol,
            ImageProtocol::GeminiInteractions | ImageProtocol::GeminiGenerateContent
        )
    })
}

pub(crate) fn sanitize_request_options_for_model(
    model: &str,
    mut options: GptImageRequestOptions,
) -> GptImageRequestOptions {
    let Some(model) = image_model(model) else {
        return options;
    };
    let caps = &model.profile.parameter_capabilities;
    let legacy = &model.profile.accepted_request_values;
    for (values, accepted, field) in [
        (&caps.qualities, &legacy.qualities, &mut options.quality),
        (
            &caps.backgrounds,
            &legacy.backgrounds,
            &mut options.background,
        ),
        (
            &caps.moderation_levels,
            &legacy.moderation_levels,
            &mut options.moderation,
        ),
        (
            &caps.input_fidelity_options,
            &legacy.input_fidelity_options,
            &mut options.input_fidelity,
        ),
        (
            &caps.output_formats,
            &legacy.output_formats,
            &mut options.output_format,
        ),
    ] {
        if let [fixed] = values.as_slice() {
            if !accepted.contains(field) {
                field.clone_from(fixed);
            }
        }
    }
    if let [fixed] = caps.image_counts.as_slice() {
        options.image_count = *fixed;
    }
    if matches!(
        model.profile.protocol,
        ImageProtocol::GeminiInteractions | ImageProtocol::GeminiGenerateContent
    ) {
        options.output_compression = DEFAULT_OUTPUT_COMPRESSION;
    }
    if matches!(
        model.profile.protocol,
        ImageProtocol::SeedreamImages | ImageProtocol::XaiImages
    ) {
        options.stream = false;
        options.partial_images = 0;
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

fn base_endpoint(model: ImageModelSpec<'_>, base: &str, kind: ImageEndpointKind) -> String {
    let base = base.trim().trim_end_matches('/');
    if model.profile.protocol == ImageProtocol::GeminiGenerateContent {
        if base.ends_with(":generateContent") {
            return base.to_string();
        }
        if base.ends_with(model.provider_model_id) {
            return format!("{base}:generateContent");
        }
    }
    let template = match kind {
        ImageEndpointKind::Generate => &model.endpoints.generation_path,
        ImageEndpointKind::Edit => &model.endpoints.edit_path,
    };
    format!(
        "{base}/{}",
        template.replace("{model}", model.provider_model_id)
    )
}

pub(crate) fn default_endpoint_settings_for_model(model: &str) -> EndpointSettings {
    let entry = image_model(normalize_image_model(model)).expect("registered default model");
    let base = &entry.endpoints.base_url;
    EndpointSettings {
        mode: ENDPOINT_MODE_BASE_URL.into(),
        base_url: base.clone(),
        generation_url: base_endpoint(entry, base, ImageEndpointKind::Generate),
        edit_url: base_endpoint(entry, base, ImageEndpointKind::Edit),
    }
}

pub(crate) fn model_setting_key(model: &str, suffix: &str) -> String {
    format!("model_config::{}::{}", normalize_image_model(model), suffix)
}

pub(crate) fn legacy_model_setting_ids(model: &str) -> impl Iterator<Item = &'static str> {
    image_model(model).into_iter().flat_map(|m| {
        m.aliases
            .iter()
            .map(String::as_str)
            .chain((m.id != m.provider_model_id).then_some(m.provider_model_id))
    })
}

pub(crate) fn image_endpoint_url_for_model_settings(
    model: &str,
    settings: &EndpointSettings,
    kind: ImageEndpointKind,
) -> String {
    let model = image_model(normalize_image_model(model)).expect("registered default model");
    if settings.mode != ENDPOINT_MODE_FULL_URL {
        return base_endpoint(model, &settings.base_url, kind);
    }
    match kind {
        ImageEndpointKind::Generate => &settings.generation_url,
        ImageEndpointKind::Edit
            if model.profile.protocol == ImageProtocol::SeedreamImages
                || (model.profile.protocol == ImageProtocol::GeminiGenerateContent
                    && settings.edit_url.trim().is_empty()) =>
        {
            &settings.generation_url
        }
        ImageEndpointKind::Edit => &settings.edit_url,
    }
    .clone()
}

pub(crate) fn is_seedream_model(model: &str) -> bool {
    image_model(model).is_some_and(|m| m.profile.protocol == ImageProtocol::SeedreamImages)
}

pub(crate) fn migrate_official_gemini_url(url: &str) -> String {
    if let Some((base, name)) = url
        .strip_suffix(":generateContent")
        .and_then(|url| url.rsplit_once('/'))
    {
        if let Some(m) = image_model(name) {
            if m.profile.protocol == ImageProtocol::GeminiGenerateContent
                && base == m.endpoints.base_url
                && m.aliases.iter().any(|a| a == name)
            {
                return base_endpoint(m, base, ImageEndpointKind::Generate);
            }
        }
    }
    url.to_owned()
}

pub(crate) fn validate_request_options(
    model: &str,
    options: &GptImageRequestOptions,
    source_count: usize,
) -> Result<(), crate::error::AppError> {
    let model = require_image_model(model)?;
    validate_model_options(model, options, source_count)
}

pub(crate) fn validate_model_options(
    model: ImageModelSpec<'_>,
    options: &GptImageRequestOptions,
    source_count: usize,
) -> Result<(), crate::error::AppError> {
    let invalid = || crate::error::AppError::Validation {
        message: "Unsupported image parameters for the selected model.".to_string(),
    };
    let profile = model.profile;
    let caps = &profile.parameter_capabilities;
    let legacy = &profile.accepted_request_values;
    let max_sources = if profile.supports_edit {
        profile.max_reference_images
    } else {
        0
    };
    if source_count > max_sources {
        return Err(crate::error::AppError::Validation {
            message: format!("This model supports at most {max_sources} reference images."),
        });
    }
    let image26 = profile.advanced_options;
    let extended_format = image26 && ["gif", "svg"].contains(&options.output_format.as_str());
    if !(caps.image_counts.contains(&options.image_count)
        || legacy.image_counts.contains(&options.image_count))
        || options.output_compression > 100
        || options.partial_images > 3
        || (options.partial_images > 0 && !options.stream)
        || (extended_format
            && (options.stream || options.partial_images != 0 || options.output_compression != 100))
        || (options.output_format == "svg" && (source_count > 0 || options.image_count != 1))
    {
        return Err(invalid());
    }
    if let Some(a) = &options.advanced_options {
        if !image26
            || a.complexity
                .as_deref()
                .is_some_and(|v| !["low", "medium", "high"].contains(&v))
            || a.frame_count.is_some_and(|v| !(2..=16).contains(&v))
            || a.frame_duration_ms.is_some_and(|v| v < 20)
            || ((a.frame_count.is_some() || a.frame_duration_ms.is_some())
                && options.output_format != "gif")
            || (a.svg.is_some() && options.output_format != "svg")
            || (a.markup_path.is_some() && a.mask_path.is_some())
            || ((a.markup_path.is_some() || a.mask_path.is_some()) && source_count == 0)
            || (a.mask_path.is_some() && options.output_format == "gif")
            || a.svg.as_ref().is_some_and(|svg| {
                svg.model.trim().is_empty()
                    || svg.model.len() > 256
                    || svg.model.chars().any(char::is_control)
                    || svg.reasoning.as_ref().is_some_and(|r| {
                        !["none", "minimal", "low", "medium", "high", "xhigh", "max"]
                            .contains(&r.effort.as_str())
                    })
            })
        {
            return Err(invalid());
        }
    }
    for (value, presets, accepted) in [
        (Some(&options.quality), &caps.qualities, &legacy.qualities),
        (
            Some(&options.background),
            &caps.backgrounds,
            &legacy.backgrounds,
        ),
        (
            Some(&options.output_format),
            &caps.output_formats,
            &legacy.output_formats,
        ),
        (
            Some(&options.input_fidelity),
            &caps.input_fidelity_options,
            &legacy.input_fidelity_options,
        ),
        (
            Some(&options.moderation),
            &caps.moderation_levels,
            &legacy.moderation_levels,
        ),
        (
            options.aspect_ratio.as_ref(),
            &caps.aspect_ratios,
            &legacy.aspect_ratios,
        ),
        (
            options.resolution.as_ref(),
            &caps.resolutions,
            &legacy.resolutions,
        ),
    ] {
        if value.is_some_and(|v| !presets.contains(v) && !accepted.contains(v)) {
            return Err(invalid());
        }
    }
    if options.background == "transparent"
        && ["jpeg", "gif"].contains(&options.output_format.as_str())
    {
        return Err(invalid());
    }
    if profile.protocol == ImageProtocol::OpenaiImages && options.size != "auto" {
        let (w, h) = options
            .size
            .split_once('x')
            .and_then(|(w, h)| Some((w.parse::<u64>().ok()?, h.parse::<u64>().ok()?)))
            .ok_or_else(invalid)?;
        if options.output_format == "gif" {
            if w == 0 || h == 0 || w % 16 != 0 || h % 16 != 0 {
                return Err(invalid());
            }
            return Ok(());
        }
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
            base_endpoint(
                image_model(ENGINE_GPT_IMAGE_2).unwrap(),
                "https://api.example.test/v1/",
                ImageEndpointKind::Generate
            ),
            "https://api.example.test/v1/images/generations"
        );
        assert_eq!(
            base_endpoint(
                image_model(ENGINE_GPT_IMAGE_2).unwrap(),
                "https://api.example.test/v1",
                ImageEndpointKind::Edit
            ),
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
                advanced_options: None,
                prepared_overlay: None,
                progress_context: None,
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
            advanced_options: None,
            prepared_overlay: None,
            progress_context: None,
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

#[cfg(test)]
mod image26_tests {
    use super::*;
    #[test]
    fn image26_models_remain_distinct_and_default_to_flare() {
        for model in [
            "gpt-image-2.6-flare",
            "gpt-image-2.6-sunburst",
            "gpt-image-2.5-flare",
            "gpt-image-2.5-sunburst",
        ] {
            assert_eq!(normalize_image_model(model), model);
            assert_eq!(
                crate::image_engines::provider_for_model(model),
                crate::image_engines::ImageProvider::OpenAi
            );
        }
        assert_eq!(CATALOG.default_image_model, "gpt-image-2.6-flare");
    }
    #[test]
    fn advanced_options_survive_job_serialization() {
        let json = serde_json::json!({"advanced_options":{"complexity":"high","frameCount":12,"frameDurationMs":200,"stream":false}});
        let options: crate::generation_jobs::GenerationJobOptions =
            serde_json::from_value(json.clone()).expect("typed advanced options supported");
        assert_eq!(serde_json::to_value(options).unwrap(), json);
    }
    #[test]
    fn image26_gif_svg_and_count_controls_are_validated() {
        let mut o = crate::generation_lifecycle::image_request_options(
            None,
            None,
            None,
            Some("gif".into()),
            None,
            None,
            None,
            Some(10),
        );
        assert_eq!(o.image_count, 10);
        o.size = "256x256".into();
        assert!(validate_request_options("gpt-image-2.6-flare", &o, 0).is_ok());
        o.output_format = "svg".into();
        o.size = "auto".into();
        o.image_count = 1;
        assert!(validate_request_options("gpt-image-2.6-flare", &o, 0).is_ok());
        assert!(validate_request_options("gpt-image-2.6-flare", &o, 1).is_err());
        o.image_count = 2;
        assert!(validate_request_options("gpt-image-2.6-flare", &o, 0).is_err());
    }
    #[test]
    fn image26_gif_duration_accepts_api_range_above_guide_examples() {
        let mut options = crate::generation_lifecycle::image_request_options(
            None,
            None,
            None,
            Some("gif".into()),
            None,
            None,
            None,
            None,
        );
        for duration in [20, 2001, 655350, u32::MAX] {
            options.advanced_options = Some(ImageAdvancedOptions {
                frame_duration_ms: Some(duration),
                ..ImageAdvancedOptions::default()
            });
            assert!(
                validate_request_options(ENGINE_GPT_IMAGE_26_FLARE, &options, 0).is_ok(),
                "duration {duration}"
            );
        }
        options.advanced_options.as_mut().unwrap().frame_duration_ms = Some(19);
        assert!(validate_request_options(ENGINE_GPT_IMAGE_26_FLARE, &options, 0).is_err());
    }

    #[test]
    fn image26_rejects_incompatible_advanced_controls() {
        let base = crate::generation_lifecycle::image_request_options(
            None, None, None, None, None, None, None, None,
        );
        for value in [
            serde_json::json!({"complexity":"max"}),
            serde_json::json!({"frameCount":9}),
            serde_json::json!({"svg":{"model":"gpt-5"}}),
            serde_json::json!({"markupPath":"/a.png","maskPath":"/b.png"}),
            serde_json::json!({"partialImages":1,"stream":false}),
        ] {
            let mut o = base.clone();
            o.advanced_options = Some(serde_json::from_value(value).unwrap());
            if let Some(a) = &o.advanced_options {
                o.stream = a.stream.unwrap_or(false);
                o.partial_images = a.partial_images.unwrap_or(0);
            }
            assert!(validate_request_options(ENGINE_GPT_IMAGE_26_FLARE, &o, 0).is_err());
        }
        for quality in ["auto", "low", "medium", "high", "xhigh", "max"] {
            let mut o = base.clone();
            o.quality = quality.into();
            assert!(validate_request_options(ENGINE_GPT_IMAGE_26_SUNBURST, &o, 0).is_ok());
        }
        let mut o = base;
        o.output_format = "gif".into();
        o.background = "transparent".into();
        assert!(validate_request_options(ENGINE_GPT_IMAGE_26_FLARE, &o, 0).is_err());
    }
}
