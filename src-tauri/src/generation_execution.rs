use crate::api_gateway::GptImageEngine;
use crate::db::Database;
use crate::error::AppError;
use crate::generation_lifecycle::{
    run_generation_lifecycle, GenerationLifecycleKind, GenerationLifecycleRequest,
};
use crate::models::GenerateResult;

/// Compatibility-mode execution mode selector.
/// Phase 1A uses this seam as the one supported path.
/// It is intentionally narrow so the later runtime adapter can replace behavior
/// without changing command call sites.
#[derive(Debug, Clone, Copy)]
pub(crate) enum GenerationExecutionMode {
    /// Current behavior: command adapter submits to the compatibility lifecycle.
    Compatibility,
}

#[derive(Debug, Clone)]
pub(crate) struct GenerationExecutionRequest {
    pub(crate) aspect_ratio: Option<String>,
    pub(crate) resolution: Option<String>,
    pub(crate) kind: GenerationLifecycleKind,
    pub(crate) prompt: String,
    pub(crate) model: Option<String>,
    pub(crate) client_request_id: Option<String>,
    pub(crate) source_image_paths: Vec<String>,
    pub(crate) size: Option<String>,
    pub(crate) quality: Option<String>,
    pub(crate) background: Option<String>,
    pub(crate) output_format: Option<String>,
    pub(crate) output_compression: Option<u8>,
    pub(crate) moderation: Option<String>,
    pub(crate) input_fidelity: Option<String>,
    pub(crate) image_count: Option<u8>,
    pub(crate) conversation_id: Option<String>,
    pub(crate) project_id: Option<String>,
    pub(crate) execution_mode: GenerationExecutionMode,
}

impl GenerationExecutionRequest {
    pub(crate) fn into_lifecycle_request(self) -> GenerationLifecycleRequest {
        GenerationLifecycleRequest {
            aspect_ratio: self.aspect_ratio,
            resolution: self.resolution,
            kind: self.kind,
            prompt: self.prompt,
            model: self.model,
            client_request_id: self.client_request_id,
            source_image_paths: self.source_image_paths,
            size: self.size,
            quality: self.quality,
            background: self.background,
            output_format: self.output_format,
            output_compression: self.output_compression,
            moderation: self.moderation,
            input_fidelity: self.input_fidelity,
            image_count: self.image_count,
            conversation_id: self.conversation_id,
            project_id: self.project_id,
        }
    }
}

impl Default for GenerationExecutionRequest {
    fn default() -> Self {
        Self {
            aspect_ratio: None,
            resolution: None,
            kind: GenerationLifecycleKind::Generate,
            prompt: String::new(),
            client_request_id: None,
            model: None,
            source_image_paths: Vec::new(),
            size: None,
            quality: None,
            background: None,
            output_format: None,
            output_compression: None,
            moderation: None,
            input_fidelity: None,
            image_count: None,
            conversation_id: None,
            project_id: None,
            execution_mode: GenerationExecutionMode::Compatibility,
        }
    }
}

pub(crate) async fn submit_generation_request(
    app: &tauri::AppHandle,
    db: &Database,
    engine: &GptImageEngine,
    request: GenerationExecutionRequest,
) -> Result<GenerateResult, AppError> {
    match request.execution_mode {
        GenerationExecutionMode::Compatibility => {
            run_generation_lifecycle(app, db, engine, request.into_lifecycle_request()).await
        }
    }
}
