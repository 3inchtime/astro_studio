export interface Generation {
  id: string;
  prompt: string;
  engine: string;
  size: string;
  quality: string;
  status: string;
  created_at: string;
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
}

export interface Message {
  id: string;
  role: "user" | "assistant";
  content: string;
  generationId?: string;
  imagePath?: string;
  thumbnailPath?: string;
  status: "complete" | "processing" | "failed";
  error?: string;
  createdAt: string;
}

export type ImageSize = "1024x1024" | "1536x1024" | "1024x1536" | "auto";
export type ImageQuality = "low" | "medium" | "high";
