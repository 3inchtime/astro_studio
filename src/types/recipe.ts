import type {
  ImageAdvancedOptions,
  ImageBackground,
  ImageInputFidelity,
  ImageModeration,
  ImageOutputFormat,
  ImageQuality,
  ImageSize,
} from "./index";
export interface GenerationRecipe {
  version: number;
  generation_id: string;
  provenance: "job" | "legacy";
  prompt: string;
  model: string;
  request_kind: string;
  source_image_paths: string[];
  provider_profile_id?: string | null;
  endpoint_snapshot?: string | null;
  warnings: string[];
  options: {
    size?: ImageSize;
    quality?: ImageQuality;
    background?: ImageBackground;
    output_format?: ImageOutputFormat;
    output_compression?: number;
    moderation?: ImageModeration;
    input_fidelity?: ImageInputFidelity;
    image_count?: number;
    aspect_ratio?: string;
    resolution?: string;
    advanced_options?: ImageAdvancedOptions;
    stream?: boolean;
    partial_images?: number;
  };
}
