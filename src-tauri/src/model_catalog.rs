//! Versioned built-in capabilities, shared with the frontend. User endpoints,
//! profile selections, and credentials live separately in settings storage.
use serde::Deserialize;
use std::{collections::HashMap, sync::LazyLock};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ImageProtocol {
    OpenaiImages,
    GeminiGenerateContent,
    GeminiInteractions,
    XaiImages,
    SeedreamImages,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProtocolSettings {
    pub base_url: String,
    pub generation_path: String,
    pub edit_path: String,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Capabilities {
    pub qualities: Vec<String>,
    pub backgrounds: Vec<String>,
    pub output_formats: Vec<String>,
    pub moderation_levels: Vec<String>,
    pub input_fidelity_options: Vec<String>,
    pub aspect_ratios: Vec<String>,
    pub resolutions: Vec<String>,
    pub image_counts: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ParameterDefaults {
    pub resolution: Option<String>,
    pub output_format: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImageProfile {
    pub protocol: ImageProtocol,
    pub supports_edit: bool,
    pub max_reference_images: usize,
    pub advanced_options: bool,
    pub parameter_defaults: ParameterDefaults,
    pub parameter_capabilities: Capabilities,
    // Some legacy recipes use valid wire values outside the composer presets.
    #[serde(default)]
    pub accepted_request_values: Capabilities,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageModelDefinition {
    profile: String,
    provider_model_id: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
}

#[derive(Clone, Copy)]
pub(crate) struct ImageModelSpec<'a> {
    pub id: &'a str,
    pub provider_model_id: &'a str,
    pub aliases: &'a [String],
    pub profile: &'a ImageProfile,
    pub endpoints: &'a ProtocolSettings,
}

#[derive(Deserialize)]
pub(crate) struct EnabledLimits {
    pub text: usize,
    pub multimodal: usize,
    pub total: usize,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LlmAdapter {
    Openai,
    Anthropic,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LlmProtocol {
    pub adapter: LlmAdapter,
    pub base_url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LlmCatalog {
    pub protocols: HashMap<String, LlmProtocol>,
    pub enabled_limits: EnabledLimits,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelCatalog {
    schema_version: u32,
    pub default_image_model: String,
    image_protocols: HashMap<ImageProtocol, ProtocolSettings>,
    image_profiles: HashMap<String, ImageProfile>,
    image_models: HashMap<String, ImageModelDefinition>,
    pub llm: LlmCatalog,
}

impl ModelCatalog {
    pub(crate) fn parse(json: &str) -> Result<Self, String> {
        let catalog: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if catalog.schema_version != 1
            || !catalog
                .image_models
                .contains_key(&catalog.default_image_model)
        {
            return Err("Unsupported model catalog or missing default model".into());
        }
        let mut names = std::collections::HashSet::new();
        for (id, model) in &catalog.image_models {
            let profile = catalog
                .image_profiles
                .get(&model.profile)
                .ok_or_else(|| format!("Missing capability profile for {id}"))?;
            if !catalog.image_protocols.contains_key(&profile.protocol) {
                return Err(format!("Missing protocol settings for {id}"));
            }
            for name in std::iter::once(id)
                .chain(model.provider_model_id.iter().filter(|v| *v != id))
                .chain(&model.aliases)
            {
                if !names.insert(name) {
                    return Err(format!("Duplicate model catalog name: {name}"));
                }
            }
        }
        Ok(catalog)
    }

    pub(crate) fn image_model(&self, name: &str) -> Option<ImageModelSpec<'_>> {
        let (id, model) = self.image_models.get_key_value(name).or_else(|| {
            self.image_models.iter().find(|(_, m)| {
                m.provider_model_id.as_deref() == Some(name) || m.aliases.iter().any(|a| a == name)
            })
        })?;
        let profile = &self.image_profiles[&model.profile];
        Some(ImageModelSpec {
            id,
            provider_model_id: model.provider_model_id.as_deref().unwrap_or(id),
            aliases: &model.aliases,
            profile,
            endpoints: &self.image_protocols[&profile.protocol],
        })
    }
}

pub(crate) const MANIFEST_JSON: &str = include_str!("../../shared/model-catalog.v1.json");
pub(crate) static CATALOG: LazyLock<ModelCatalog> =
    LazyLock::new(|| ModelCatalog::parse(MANIFEST_JSON).expect("invalid bundled model catalog"));
