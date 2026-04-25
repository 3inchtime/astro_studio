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

export type ImageSize = "1024x1024" | "1536x1024" | "1024x1536" | "auto";
export type ImageQuality = "low" | "medium" | "high";

export interface Task {
  id: string;
  prompt: string;
  size: string;
  quality: string;
  status: "processing" | "completed" | "failed";
  imagePath: string | null;
  error: string | null;
  createdAt: number;
}
