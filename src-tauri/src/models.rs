use serde::{Deserialize, Serialize};

pub const ENGINE_GPT_IMAGE_2: &str = "gpt-image-2";
pub const SETTING_API_KEY: &str = "api_key";
pub const SETTING_BASE_URL: &str = "base_url";
pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Generation {
    pub id: String,
    pub prompt: String,
    pub engine: String,
    pub size: String,
    pub quality: String,
    pub status: String,
    pub created_at: String,
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
    pub image_paths: Vec<String>,
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
    pub latest_thumbnail: Option<String>,
}
