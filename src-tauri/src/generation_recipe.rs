use crate::{
    commands::settings,
    db::Database,
    error::AppError,
    generation_jobs::{self, GenerationJobOptions, GenerationJobRequest},
    models::*,
};
use rusqlite::OptionalExtension;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct GenerationRecipe {
    version: u8,
    generation_id: String,
    provenance: &'static str,
    prompt: String,
    model: String,
    request_kind: String,
    source_image_paths: Vec<String>,
    options: GenerationJobOptions,
    provider_profile_id: Option<String>,
    endpoint_snapshot: Option<String>,
    warnings: Vec<String>,
}
fn invalid(message: &str) -> AppError {
    AppError::Validation {
        message: message.into(),
    }
}

pub(crate) fn saved_job(
    db: &Database,
    generation_id: &str,
) -> Result<Option<GenerationJob>, AppError> {
    let conn = db
        .conn
        .lock()
        .map_err(|_| invalid("Cannot read saved generation"))?;
    let id: Option<String> = conn.query_row("SELECT j.id FROM generation_jobs j JOIN generations g ON g.id=j.generation_id WHERE g.id=?1 AND g.deleted_at IS NULL", [generation_id], |r| r.get(0)).optional().map_err(|_| invalid("Cannot read saved generation"))?;
    id.map(|id| generation_jobs::get_job(&conn, &id))
        .transpose()
}

pub(crate) fn load_recipe(
    db: &Database,
    generation_id: &str,
) -> Result<GenerationRecipe, AppError> {
    let generation = {
        let conn = db
            .conn
            .lock()
            .map_err(|_| invalid("Cannot read saved generation"))?;
        conn.query_row(
            "SELECT * FROM generations WHERE id=?1 AND deleted_at IS NULL",
            [generation_id],
            crate::gallery::row_to_generation,
        )
        .map_err(|_| invalid("Generation is unavailable"))?
    };
    let mut recipe = GenerationRecipe {
        version: 1,
        generation_id: generation.id.clone(),
        provenance: "legacy",
        prompt: generation.prompt.clone(),
        model: generation.engine.clone(),
        request_kind: generation.request_kind.clone(),
        source_image_paths: generation.source_image_paths.clone(),
        options: GenerationJobOptions {
            size: Some(generation.size.clone()),
            quality: Some(generation.quality.clone()),
            background: Some(generation.background.clone()),
            output_format: Some(generation.output_format.clone()),
            output_compression: u8::try_from(generation.output_compression).ok(),
            moderation: Some(generation.moderation.clone()),
            input_fidelity: Some(generation.input_fidelity.clone()),
            image_count: u8::try_from(generation.image_count).ok(),
            ..Default::default()
        },
        provider_profile_id: None,
        endpoint_snapshot: None,
        warnings: vec![],
    };
    match saved_job(db, generation_id) {
        Ok(Some(job)) => {
            if job.error_code.as_deref() == Some("provider_outcome_unknown") {
                recipe.warnings.push("provider_outcome_unknown".into());
            }
            let request: GenerationJobRequest = serde_json::from_value(job.request)
                .map_err(|_| invalid("Saved recipe is invalid"))?;
            recipe.provenance = "job";
            recipe.prompt = request.prompt;
            recipe.model = request.model;
            recipe.request_kind = request.kind.as_str().into();
            recipe.source_image_paths = request.source_image_paths;
            // Prefer the validated request. Its linked generation supplies the
            // effective defaults only for controls absent from older requests.
            recipe.options = GenerationJobOptions {
                size: request.options.size.or(recipe.options.size),
                quality: request.options.quality.or(recipe.options.quality),
                background: request.options.background.or(recipe.options.background),
                output_format: request
                    .options
                    .output_format
                    .or(recipe.options.output_format),
                output_compression: request.options.output_compression,
                moderation: request.options.moderation.or(recipe.options.moderation),
                input_fidelity: request
                    .options
                    .input_fidelity
                    .or(recipe.options.input_fidelity),
                image_count: request.options.image_count.or(recipe.options.image_count),
                advanced_options: request.options.advanced_options,
                aspect_ratio: request.options.aspect_ratio,
                resolution: request.options.resolution,
                stream: request.options.stream,
                partial_images: request.options.partial_images,
            };
            recipe.provider_profile_id = Some(job.provider_profile_id.clone());
            recipe.endpoint_snapshot = Some(job.endpoint_snapshot.clone());
            let profile = settings::read_model_provider_profiles_state(db, &recipe.model)
                .ok()
                .and_then(|s| {
                    s.profiles
                        .into_iter()
                        .find(|p| p.id == job.provider_profile_id)
                });
            match profile {
                None => recipe.warnings.push("provider_missing".into()),
                Some(profile) => {
                    let kind = if recipe.request_kind == "edit" {
                        crate::model_registry::ImageEndpointKind::Edit
                    } else {
                        crate::model_registry::ImageEndpointKind::Generate
                    };
                    if crate::model_registry::image_endpoint_url_for_model_settings(
                        &recipe.model,
                        &profile.endpoint_settings,
                        kind,
                    ) != job.endpoint_snapshot
                    {
                        recipe.warnings.push("endpoint_changed".into());
                    }
                    if profile.api_key.trim().is_empty() {
                        recipe.warnings.push("provider_unconfigured".into());
                    }
                }
            }
        }
        Ok(None) => {
            recipe.warnings.push("legacy_incomplete".into());
            if let Some(metadata) = generation
                .request_metadata
                .as_ref()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            {
                recipe.options.advanced_options = metadata
                    .get("advanced_options")
                    .and_then(|v| serde_json::from_value(v.clone()).ok());
                recipe.options.stream = metadata.get("stream").and_then(|v| v.as_bool());
                recipe.options.partial_images = metadata
                    .get("partial_images")
                    .and_then(|v| v.as_u64())
                    .and_then(|v| u8::try_from(v).ok());
                recipe.options.aspect_ratio = metadata
                    .get("aspect_ratio")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned);
                recipe.options.resolution = metadata
                    .get("resolution")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned);
            }
        }
        Err(_) => {
            recipe.warnings.push("invalid_job".into());
            recipe.warnings.push("legacy_incomplete".into());
        }
    }
    if crate::image_engines::provider_for_model(&recipe.model)
        == crate::image_engines::ImageProvider::Unknown
    {
        recipe.warnings.push("unknown_model".into());
    }
    let mut options = crate::generation_lifecycle::image_request_options(
        recipe.options.size.clone(),
        recipe.options.quality.clone(),
        recipe.options.background.clone(),
        recipe.options.output_format.clone(),
        recipe.options.output_compression,
        recipe.options.moderation.clone(),
        recipe.options.input_fidelity.clone(),
        recipe.options.image_count,
    );
    options.advanced_options = recipe.options.advanced_options.clone();
    options.stream = recipe
        .options
        .stream
        .or_else(|| {
            recipe
                .options
                .advanced_options
                .as_ref()
                .and_then(|a| a.stream)
        })
        .unwrap_or(false);
    options.partial_images = recipe
        .options
        .partial_images
        .or_else(|| {
            recipe
                .options
                .advanced_options
                .as_ref()
                .and_then(|a| a.partial_images)
        })
        .unwrap_or(0);
    options.aspect_ratio = recipe.options.aspect_ratio.clone();
    options.resolution = recipe.options.resolution.clone();
    if crate::model_registry::validate_request_options(
        &recipe.model,
        &options,
        recipe.source_image_paths.len(),
    )
    .is_err()
        || ((options.stream || options.partial_images > 0)
            && !matches!(
                recipe.model.as_str(),
                ENGINE_GPT_IMAGE_26_FLARE | ENGINE_GPT_IMAGE_26_SUNBURST
            ))
        || !(0..=100).contains(&generation.output_compression)
        || !(1..=10).contains(&generation.image_count)
        || !["auto", "low"].contains(&generation.moderation.as_str())
        || !["high", "low"].contains(&generation.input_fidelity.as_str())
    {
        recipe.warnings.push("invalid_parameters".into());
    }
    let paths = recipe.source_image_paths.iter().chain(
        recipe
            .options
            .advanced_options
            .iter()
            .flat_map(|a| a.markup_path.iter().chain(a.mask_path.iter())),
    );
    if paths.clone().any(|p| !std::path::Path::new(p).is_file()) {
        recipe.warnings.push("source_missing".into());
    }
    if recipe.provenance != "job" && !recipe.source_image_paths.is_empty() {
        recipe.warnings.push("sources_require_selection".into());
    }
    Ok(recipe)
}

#[tauri::command]
pub(crate) fn get_generation_recipe(
    db: tauri::State<'_, Database>,
    generation_id: String,
) -> Result<GenerationRecipe, AppError> {
    load_recipe(db.inner(), &generation_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_recipe_reports_missing_authority_and_preserves_basic_parameters() {
        let dir = std::env::temp_dir().join(format!("recipe-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::open(&dir.join("test.db")).unwrap();
        db.run_migrations().unwrap();
        db.conn.lock().unwrap().execute("INSERT INTO generations (id,prompt,engine,size,quality,status) VALUES ('legacy','draw observatory','gpt-image-2','1536x1024','high','completed')", []).unwrap();
        let recipe = load_recipe(&db, "legacy").unwrap();
        assert_eq!(recipe.version, 1);
        assert_eq!(recipe.options.size.as_deref(), Some("1536x1024"));
        assert_eq!(recipe.provenance, "legacy");
        assert!(recipe.warnings.contains(&"legacy_incomplete".into()));
        assert!(recipe.provider_profile_id.is_none());
        drop(db);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
