import type { GenerationResult, MessageImage } from "../types";

export function generationResultToLightboxImages(
  result: GenerationResult,
): MessageImage[] {
  return result.images.map((image) => ({
    imageId: image.id,
    generationId: image.generation_id,
    path: image.file_path,
    thumbnailPath: image.thumbnail_path,
  }));
}
