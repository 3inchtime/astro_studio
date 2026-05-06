export interface Generation {
  id: string;
  prompt: string;
  engine: string;
  request_kind: string;
  size: string;
  quality: string;
  background: ImageBackground;
  output_format: ImageOutputFormat;
  output_compression: number;
  moderation: ImageModeration;
  input_fidelity: ImageInputFidelity;
  image_count: number;
  source_image_count: number;
  source_image_paths: string[];
  request_metadata?: string | null;
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

export interface GenerationSearchFilters {
  model?: ImageModel | "";
  request_kind?: "generate" | "edit" | "";
  status?: "processing" | "completed" | "failed" | "";
  size?: ImageSize | "";
  quality?: ImageQuality | "";
  background?: ImageBackground | "";
  output_format?: ImageOutputFormat | "";
  moderation?: ImageModeration | "";
  input_fidelity?: ImageInputFidelity | "";
  source_image_count?: "any" | "0" | "1" | "2" | "3" | "4+";
  created_from?: string;
  created_to?: string;
}

export interface GenerationParams {
  prompt: string;
  model?: ImageModel;
  size?: ImageSize;
  quality?: ImageQuality;
  background?: ImageBackground;
  outputFormat?: ImageOutputFormat;
  outputCompression?: number;
  moderation?: ImageModeration;
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
  model: ImageModel;
  size: ImageSize;
  quality: ImageQuality;
  background: ImageBackground;
  outputFormat: ImageOutputFormat;
  moderation: ImageModeration;
  inputFidelity: ImageInputFidelity;
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
export type ImageBackground = "auto" | "opaque" | "transparent";
export type ImageModeration = "auto" | "low";
export type ImageInputFidelity = "low" | "high";
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

export interface PromptFavorite {
  id: string;
  prompt: string;
  created_at: string;
  updated_at: string;
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

export interface RuntimeLogEntry {
  sequence: number;
  timestamp: string;
  level: "debug" | "info" | "warn" | "error" | string;
  target: string;
  message: string;
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
export type ImageModel =
  | "gpt-image-2"
  | "nano-banana"
  | "nano-banana-2"
  | "nano-banana-pro";
export type EndpointMode = "base_url" | "full_url";

export interface EndpointSettings {
  mode: EndpointMode;
  base_url: string;
  generation_url: string;
  edit_url: string;
}

export interface ModelConnectionDefaults {
  baseUrl: string;
  generationUrl: string;
  editUrl: string;
}

export interface ModelParameterDefaults {
  size: ImageSize;
  quality: ImageQuality;
  background: ImageBackground;
  outputFormat: ImageOutputFormat;
  moderation: ImageModeration;
  inputFidelity: ImageInputFidelity;
  imageCount: number;
}

export interface ModelParameterCapabilities {
  sizes: ImageSize[];
  qualities: ImageQuality[];
  backgrounds: ImageBackground[];
  outputFormats: ImageOutputFormat[];
  moderationLevels: ImageModeration[];
  inputFidelityOptions: ImageInputFidelity[];
  imageCounts: number[];
}

export interface ImageModelCatalogEntry {
  id: ImageModel;
  label: string;
  provider: string;
  providerModelId: string;
  supportsEdit: boolean;
  connectionDefaults: ModelConnectionDefaults;
  parameterDefaults: ModelParameterDefaults;
  parameterCapabilities: ModelParameterCapabilities;
}
