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

pub(crate) fn validate_saved_recipe_options(
    model: &str,
    saved: &GenerationJobOptions,
    source_count: usize,
) -> Result<(), AppError> {
    let unsupported = || invalid("Unsupported image parameters for the selected model.");
    if saved.output_compression.is_some_and(|value| value > 100)
        || saved
            .image_count
            .is_some_and(|value| !(1..=10).contains(&value))
        || saved
            .moderation
            .as_deref()
            .is_some_and(|value| !["auto", "low"].contains(&value))
        || saved
            .input_fidelity
            .as_deref()
            .is_some_and(|value| !["high", "low"].contains(&value))
    {
        return Err(unsupported());
    }
    let defaults = crate::model_registry::sanitize_request_options_for_model(
        model,
        crate::generation_lifecycle::image_request_options(
            None, None, None, None, None, None, None, None,
        ),
    );
    let mut options = crate::generation_lifecycle::image_request_options(
        saved.size.clone().or(Some(defaults.size)),
        saved.quality.clone().or(Some(defaults.quality)),
        saved.background.clone().or(Some(defaults.background)),
        saved.output_format.clone().or(Some(defaults.output_format)),
        saved
            .output_compression
            .or(Some(defaults.output_compression)),
        saved.moderation.clone().or(Some(defaults.moderation)),
        saved
            .input_fidelity
            .clone()
            .or(Some(defaults.input_fidelity)),
        saved.image_count.or(Some(defaults.image_count)),
    );
    options.advanced_options = saved.advanced_options.clone();
    options.stream = saved
        .advanced_options
        .as_ref()
        .and_then(|a| a.stream)
        .or(saved.stream)
        .unwrap_or(false);
    options.partial_images = saved
        .advanced_options
        .as_ref()
        .and_then(|a| a.partial_images)
        .or(saved.partial_images)
        .unwrap_or(0);
    options.aspect_ratio = saved.aspect_ratio.clone();
    options.resolution = saved.resolution.clone();
    crate::model_registry::validate_request_options(model, &options, source_count)?;
    let sanitized =
        crate::model_registry::sanitize_request_options_for_model(model, options.clone());
    if options.quality != sanitized.quality
        || options.background != sanitized.background
        || options.output_format != sanitized.output_format
        || options.output_compression != sanitized.output_compression
        || options.moderation != sanitized.moderation
        || options.input_fidelity != sanitized.input_fidelity
        || options.image_count != sanitized.image_count
        || options.stream != sanitized.stream
        || options.partial_images != sanitized.partial_images
        || ((options.stream || options.partial_images > 0)
            && !matches!(
                model,
                ENGINE_GPT_IMAGE_26_FLARE | ENGINE_GPT_IMAGE_26_SUNBURST
            ))
        || (options.output_compression != 100
            && (crate::image_engines::provider_for_model(model)
                != crate::image_engines::ImageProvider::OpenAi
                || !["jpeg", "webp"].contains(&options.output_format.as_str())))
    {
        return Err(unsupported());
    }
    Ok(())
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
            let unknown: bool = db.conn.lock().map_err(|_| invalid("Cannot read saved generation"))?
                .query_row("SELECT EXISTS(SELECT 1 FROM generations g LEFT JOIN generation_recoveries r ON r.generation_id=g.id WHERE g.id=?1 AND g.status='failed' AND COALESCE(r.request_state!='response_ready',1) AND (r.request_state='requesting' OR g.error_message=?2))", rusqlite::params![generation_id, crate::INTERRUPTED_GENERATION_MESSAGE], |row| row.get(0))?;
            if unknown {
                recipe.warnings.push("provider_outcome_unknown".into());
            }
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
    if validate_saved_recipe_options(
        &recipe.model,
        &recipe.options,
        recipe.source_image_paths.len(),
    )
    .is_err()
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

pub(crate) fn load_recipe_for_client_request(
    db: &Database,
    client_request_id: &str,
) -> Result<Option<GenerationRecipe>, AppError> {
    let result = {
        let conn = db
            .conn
            .lock()
            .map_err(|_| invalid("Cannot read saved generation"))?;
        generation_jobs::find_enqueue_result_by_client_request_id(&conn, client_request_id)?
    };
    result
        .map(|result| load_recipe(db, &result.generation_id))
        .transpose()
}

#[tauri::command]
pub(crate) fn get_generation_recipe(
    db: tauri::State<'_, Database>,
    generation_id: Option<String>,
    client_request_id: Option<String>,
) -> Result<Option<GenerationRecipe>, AppError> {
    match (generation_id, client_request_id) {
        (Some(id), None) => load_recipe(db.inner(), &id).map(Some),
        (None, Some(id)) => load_recipe_for_client_request(db.inner(), &id),
        _ => Err(invalid("Select exactly one generation or client request")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_registered_model_defaults_are_valid_but_explicit_unsupported_values_are_not() {
        for model in [
            ENGINE_GPT_IMAGE_26_FLARE,
            ENGINE_GPT_IMAGE_26_SUNBURST,
            ENGINE_GPT_IMAGE_2,
            ENGINE_GPT_IMAGE_FLARE,
            ENGINE_GPT_IMAGE_SUNBURST,
            ENGINE_NANO_BANANA_21,
            ENGINE_GROK,
            ENGINE_SEEDREAM_PRO,
            ENGINE_SEEDREAM_FLASH,
            ENGINE_NANO_BANANA,
            ENGINE_NANO_BANANA_2,
            ENGINE_NANO_BANANA_PRO,
        ] {
            assert!(
                validate_saved_recipe_options(model, &GenerationJobOptions::default(), 0).is_ok(),
                "{model}"
            );
        }
        let unsupported = GenerationJobOptions {
            output_format: Some("png".into()),
            ..Default::default()
        };
        assert!(validate_saved_recipe_options(ENGINE_GROK, &unsupported, 0).is_err());
        let advanced = GenerationJobOptions {
            advanced_options: Some(ImageAdvancedOptions {
                complexity: Some("high".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(validate_saved_recipe_options(ENGINE_GPT_IMAGE_2, &advanced, 0).is_err());
        let compression = GenerationJobOptions {
            output_format: Some("png".into()),
            output_compression: Some(75),
            ..Default::default()
        };
        assert!(validate_saved_recipe_options(ENGINE_GPT_IMAGE_2, &compression, 0).is_err());
    }

    #[test]
    fn legacy_parameters_that_would_be_silently_sanitized_require_review() {
        let dir = std::env::temp_dir().join(format!("recipe-unsupported-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::open(&dir.join("test.db")).unwrap();
        db.run_migrations().unwrap();
        db.conn.lock().unwrap().execute("INSERT INTO generations(id,prompt,engine,image_count,background,output_format) VALUES('legacy','fixture','nano-banana',3,'transparent','webp')", []).unwrap();
        let recipe = load_recipe(&db, "legacy").unwrap();
        assert!(recipe.warnings.contains(&"invalid_parameters".into()));
        assert_eq!(recipe.options.image_count, Some(3));
        assert_eq!(recipe.options.output_format.as_deref(), Some("webp"));
        let supported = GenerationJobOptions {
            output_format: Some("jpeg".into()),
            output_compression: Some(75),
            image_count: Some(7),
            moderation: Some("low".into()),
            ..Default::default()
        };
        assert!(validate_saved_recipe_options(ENGINE_GPT_IMAGE_2, &supported, 0).is_ok());
        let unsupported = GenerationJobOptions {
            image_count: Some(12),
            ..supported
        };
        assert!(validate_saved_recipe_options(ENGINE_GPT_IMAGE_2, &unsupported, 0).is_err());
        drop(db);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn client_request_lookup_distinguishes_missing_valid_and_corrupt_jobs() {
        use generation_jobs::{enqueue_job, PreparedGenerationJob};
        let dir = std::env::temp_dir().join(format!("recipe-client-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::open(&dir.join("test.db")).unwrap();
        db.run_migrations().unwrap();
        assert!(load_recipe_for_client_request(&db, "missing")
            .unwrap()
            .is_none());
        let prepared = PreparedGenerationJob {
            job_id: "job-current".into(),
            client_request_id: "client-current".into(),
            generation_id: "generation-current".into(),
            requested_conversation_id: None,
            requested_project_id: Some("default".into()),
            prompt: "Edited prompt".into(),
            model: "gpt-image-2".into(),
            request_kind: "generate".into(),
            size: "1024x1024".into(),
            quality: "high".into(),
            background: "auto".into(),
            output_format: "png".into(),
            output_compression: 100,
            moderation: "auto".into(),
            input_fidelity: "high".into(),
            image_count: 1,
            stream: false,
            partial_images: 0,
            source_image_paths: vec![],
            request_options: GenerationJobOptions {
                size: Some("1024x1024".into()),
                quality: Some("high".into()),
                ..Default::default()
            },
            parent_job_id: None,
            source_kind: "generate".into(),
            source_ref: serde_json::json!({"id":"client-current", "source_generation_id":"ancestor"}),
            provider_kind: "openai".into(),
            provider_profile_id: "saved-profile".into(),
            endpoint_snapshot: "https://example.test/v1/images/generations".into(),
            status: GenerationJobStatus::Queued,
            chain_attempt: 1,
            auto_attempt: 0,
            max_auto_attempts: 2,
            queued_at: "2026-11-13T00:00:00Z".into(),
            finished_at: None,
            error_code: None,
            error_message: None,
            retryable: false,
        };
        enqueue_job(&mut db.conn.lock().unwrap(), &prepared).unwrap();
        let recipe = load_recipe_for_client_request(&db, "client-current")
            .unwrap()
            .unwrap();
        assert_eq!(recipe.generation_id, "generation-current");
        assert_eq!(recipe.prompt, "Edited prompt");
        db.conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE generation_jobs SET request_json='{}' WHERE id='job-current'",
                [],
            )
            .unwrap();
        assert!(load_recipe_for_client_request(&db, "client-current").is_err());
        drop(db);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn legacy_unknown_recipe_requires_charge_acknowledgement() {
        let dir = std::env::temp_dir().join(format!("recipe-unknown-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = Database::open(&dir.join("test.db")).unwrap();
        db.run_migrations().unwrap();
        {
            let conn = db.conn.lock().unwrap();
            for (id, error) in [
                ("requesting", None),
                ("old", Some(crate::INTERRUPTED_GENERATION_MESSAGE)),
                ("ordinary", Some("Provider rejected request")),
                ("ready", None),
            ] {
                conn.execute("INSERT INTO generations(id,prompt,status,error_message) VALUES(?1,'fixture','failed',?2)", rusqlite::params![id,error]).unwrap();
            }
            for (id, state) in [("requesting", "requesting"), ("ready", "response_ready")] {
                conn.execute("INSERT INTO generation_recoveries(generation_id,request_kind,request_state,output_format,created_at,updated_at) VALUES(?1,'generate',?2,'png','2026-11-13','2026-11-13')",rusqlite::params![id,state]).unwrap();
            }
        }
        for (id, expected) in [
            ("requesting", true),
            ("old", true),
            ("ordinary", false),
            ("ready", false),
        ] {
            assert_eq!(
                load_recipe(&db, id)
                    .unwrap()
                    .warnings
                    .contains(&"provider_outcome_unknown".into()),
                expected,
                "{id}"
            );
        }
        drop(db);
        std::fs::remove_dir_all(dir).unwrap();
    }

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
