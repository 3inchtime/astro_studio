import { getImageModelCatalogEntry } from "./modelCatalog";
import type {
  GenerationParams,
  ImageInputFidelity,
  ImageModel,
  RetryGenerationRequest,
} from "../types";

export function modelSupportsEdit(model: ImageModel): boolean {
  return getImageModelCatalogEntry(model).supportsEdit;
}

export function buildGenerationParams(
  request: RetryGenerationRequest,
): GenerationParams {
  const capabilities = getImageModelCatalogEntry(
    request.model,
  ).parameterCapabilities;

  return {
    prompt: request.prompt,
    model: request.model,
    ...(capabilities.sizes.length > 0 ? { size: request.size } : {}),
    ...(capabilities.qualities.length > 1 ? { quality: request.quality } : {}),
    ...(capabilities.backgrounds.length > 1
      ? { background: request.background }
      : {}),
    ...(capabilities.outputFormats.length > 1
      ? { outputFormat: request.outputFormat }
      : {}),
    ...(capabilities.moderationLevels.length > 1
      ? { moderation: request.moderation }
      : {}),
    ...(capabilities.imageCounts.length > 0
      ? { imageCount: request.imageCount }
      : {}),
    conversationId: request.conversationId,
    projectId: request.projectId,
  };
}

export function buildEditParams(
  request: RetryGenerationRequest,
): GenerationParams & {
  sourceImagePaths: string[];
  inputFidelity?: ImageInputFidelity;
} {
  const capabilities = getImageModelCatalogEntry(
    request.model,
  ).parameterCapabilities;

  return {
    ...buildGenerationParams(request),
    sourceImagePaths: request.editSources.map((source) => source.path),
    ...(capabilities.inputFidelityOptions.length > 1
      ? { inputFidelity: request.inputFidelity }
      : {}),
  };
}
