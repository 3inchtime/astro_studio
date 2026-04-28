use serde::{Deserialize, Serialize};

pub const ENGINE_GPT_IMAGE_2: &str = "gpt-image-2";
pub const SETTING_IMAGE_MODEL: &str = "image_model";
pub const SETTING_API_KEY: &str = "api_key";
pub const SETTING_BASE_URL: &str = "base_url";
pub const SETTING_ENDPOINT_MODE: &str = "endpoint_mode";
pub const SETTING_GENERATION_URL: &str = "generation_url";
pub const SETTING_EDIT_URL: &str = "edit_url";
pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
pub const ENDPOINT_MODE_BASE_URL: &str = "base_url";
pub const ENDPOINT_MODE_FULL_URL: &str = "full_url";
pub const DEFAULT_GENERATION_URL: &str = "https://api.openai.com/v1/images/generations";
pub const DEFAULT_EDIT_URL: &str = "https://api.openai.com/v1/images/edits";
pub const DEFAULT_IMAGE_SIZE: &str = "auto";
pub const DEFAULT_IMAGE_QUALITY: &str = "auto";
pub const DEFAULT_IMAGE_BACKGROUND: &str = "auto";
pub const DEFAULT_OUTPUT_FORMAT: &str = "png";
pub const DEFAULT_OUTPUT_COMPRESSION: u8 = 100;
pub const DEFAULT_IMAGE_MODERATION: &str = "auto";
pub const DEFAULT_INPUT_FIDELITY: &str = "high";
pub const DEFAULT_IMAGE_STREAM: bool = false;
pub const DEFAULT_PARTIAL_IMAGES: u8 = 0;
pub const DEFAULT_IMAGE_COUNT: u8 = 1;
pub const DEFAULT_PAGE_SIZE: i32 = 20;
pub const SETTING_LOG_ENABLED: &str = "log_enabled";
pub const SETTING_LOG_RETENTION_DAYS: &str = "log_retention_days";
pub const DEFAULT_LOG_RETENTION_DAYS: u32 = 7;
pub const SETTING_TRASH_RETENTION_DAYS: &str = "trash_retention_days";
pub const DEFAULT_TRASH_RETENTION_DAYS: u32 = 30;
pub const SETTING_FONT_SIZE: &str = "font_size";
pub const DEFAULT_FONT_SIZE: &str = "medium";
pub const DEFAULT_IMAGE_MODEL: &str = ENGINE_GPT_IMAGE_2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub timestamp: String,
    pub log_type: String,
    pub level: String,
    pub message: String,
    pub generation_id: Option<String>,
    pub metadata: Option<String>,
    pub response_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeLogEntry {
    pub sequence: u64,
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogSearchResult {
    pub logs: Vec<LogEntry>,
    pub total: i32,
    pub page: i32,
    pub page_size: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSettings {
    pub enabled: bool,
    pub retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashSettings {
    pub retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointSettings {
    pub mode: String,
    pub base_url: String,
    pub generation_url: String,
    pub edit_url: String,
}

#[derive(Debug, Clone)]
pub struct GptImageRequestOptions {
    pub size: String,
    pub quality: String,
    pub background: String,
    pub output_format: String,
    pub output_compression: u8,
    pub moderation: String,
    pub input_fidelity: String,
    pub stream: bool,
    pub partial_images: u8,
    pub image_count: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Generation {
    pub id: String,
    pub prompt: String,
    pub engine: String,
    pub size: String,
    pub quality: String,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedImage {
    pub id: String,
    pub generation_id: String,
    pub file_path: String,
    pub thumbnail_path: String,
    pub width: i32,
    pub height: i32,
    pub file_size: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerationResult {
    pub generation: Generation,
    pub images: Vec<GeneratedImage>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub generations: Vec<GenerationResult>,
    pub total: i32,
    pub page: i32,
    pub page_size: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateResult {
    pub generation_id: String,
    pub conversation_id: String,
    pub images: Vec<GeneratedImage>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAiImageResponse {
    pub data: Vec<OpenAiImageData>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct OpenAiImageData {
    pub b64_json: Option<String>,
    pub url: Option<String>,
    pub revised_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub generation_count: i32,
    pub latest_generation_at: Option<String>,
    pub latest_thumbnail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub created_at: String,
}
