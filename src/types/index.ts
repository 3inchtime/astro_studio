export interface Generation {
  id: string;
  prompt: string;
  engine: string;
  size: string;
  quality: string;
  status: string;
  error_message?: string | null;
  created_at: string;
  deleted_at: string | null;
}

export interface GeneratedImage {
  id: string;
  generation_id: string;
  file_path: string;
  thumbnail_path: string;
  width: number;
  height: number;
  file_size: number;
}

export interface GenerationResult {
  generation: Generation;
  images: GeneratedImage[];
}

export interface Conversation {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  generation_count: number;
  latest_generation_at: string | null;
  latest_thumbnail: string | null;
}

export interface SearchResult {
  generations: GenerationResult[];
  total: number;
  page: number;
  page_size: number;
}

export interface GenerationParams {
  prompt: string;
  size?: string;
  quality?: string;
  outputFormat?: string;
  imageCount?: number;
  conversationId?: string | null;
}

export interface MessageImage {
  imageId: string;
  generationId: string;
  path: string;
  thumbnailPath?: string;
}

export interface EditSourceImage {
  id: string;
  path: string;
  label: string;
  imageId?: string;
  generationId?: string;
}

export interface RetryGenerationRequest {
  prompt: string;
  size: ImageSize;
  quality: ImageQuality;
  outputFormat: ImageOutputFormat;
  imageCount: number;
  conversationId?: string | null;
  editSources: EditSourceImage[];
}

export interface Message {
  id: string;
  role: "user" | "assistant";
  content: string;
  generationId?: string;
  images?: MessageImage[];
  sourceImages?: MessageImage[];
  status: "complete" | "processing" | "failed";
  error?: string;
  retryRequest?: RetryGenerationRequest;
  createdAt: string;
}

export type ImageSize = "1024x1024" | "1536x1024" | "1024x1536" | "auto";
export type ImageQuality = "low" | "medium" | "high" | "auto";
export type ImageOutputFormat = "png" | "jpeg" | "webp";

export interface GenerateResponse {
  generation_id: string;
  conversation_id: string;
  images: GeneratedImage[];
}

export interface Folder {
  id: string;
  name: string;
  created_at: string;
}

export interface LogEntry {
  id: string;
  timestamp: string;
  log_type: "api_request" | "api_response" | "generation" | "system";
  level: "debug" | "info" | "warn" | "error";
  message: string;
  generation_id: string | null;
  metadata: string | null;
  response_file: string | null;
}

export interface LogSearchResult {
  logs: LogEntry[];
  total: number;
  page: number;
  page_size: number;
}

export interface LogSettings {
  enabled: boolean;
  retention_days: number;
}

export interface TrashSettings {
  retention_days: number;
}

export type AppFontSize = "small" | "medium" | "large";
export type ImageModel = "gpt-image-2";
