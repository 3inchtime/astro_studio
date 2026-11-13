import { getImageModelCatalogEntry, isGptImage26 } from "./modelCatalog";
import type {
  GenerationParams,
  ImageAdvancedOptions,
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

  // Saved requests are checked against the backend model registry. UI preset
  // lists are intentionally narrower and must not rewrite a recipe.
  const savedRecipe = Boolean(request.sourceGenerationId);
  const params: GenerationParams = {
    ...(request.providerOverrideConfirmed
      ? { providerOverrideConfirmed: true }
      : {}),
    ...(request.providerProfileId
      ? { providerProfileId: request.providerProfileId }
      : {}),
    ...(request.sourceGenerationId
      ? { sourceGenerationId: request.sourceGenerationId }
      : {}),
    ...(isGptImage26(request.model) && request.advancedOptions
      ? {
          advancedOptions: normalizeAdvancedOptions(
            request.advancedOptions,
            request.outputFormat,
            false,
          ),
        }
      : {}),
    ...((savedRecipe || isGptImage26(request.model)) &&
    ["jpeg", "webp"].includes(request.outputFormat) &&
    request.outputCompression !== undefined
      ? { outputCompression: request.outputCompression }
      : {}),
    prompt: request.prompt,
    model: request.model,
    size: request.size,
    ...((savedRecipe || capabilities.aspectRatios?.includes(request.aspectRatio ?? ""))
      ? { aspectRatio: request.aspectRatio }
      : {}),
    ...((savedRecipe || capabilities.resolutions?.includes(request.resolution ?? ""))
      ? { resolution: request.resolution }
      : {}),
    ...((savedRecipe || capabilities.qualities.length > 1)
      ? {
          quality: savedRecipe || capabilities.qualities.includes(request.quality)
            ? request.quality
            : "auto",
        }
      : {}),
    ...((savedRecipe || capabilities.backgrounds.length > 1)
      ? { background: request.background }
      : {}),
    ...((savedRecipe || capabilities.outputFormats.length > 1)
      ? {
          outputFormat: savedRecipe || capabilities.outputFormats.includes(
            request.outputFormat,
          )
            ? request.outputFormat
            : capabilities.outputFormats[0],
        }
      : {}),
    ...((savedRecipe || capabilities.moderationLevels.length > 1)
      ? { moderation: request.moderation }
      : {}),
    ...(capabilities.imageCounts.length > 0
      ? {
          imageCount: savedRecipe || capabilities.imageCounts.includes(request.imageCount)
            ? request.imageCount
            : 1,
        }
      : {}),
    conversationId: request.conversationId,
    projectId: request.projectId,
  };
  if (request.clientRequestId) {
    params.clientRequestId = request.clientRequestId;
  }
  return params;
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
    ...(isGptImage26(request.model) && request.advancedOptions
      ? {
          advancedOptions: normalizeAdvancedOptions(
            request.advancedOptions,
            request.outputFormat,
            true,
          ),
        }
      : {}),
    ...((request.sourceGenerationId || capabilities.inputFidelityOptions.length > 1)
      ? { inputFidelity: request.inputFidelity }
      : {}),
  };
}

export function normalizeAdvancedOptions(
  options: ImageAdvancedOptions,
  format: string,
  editing: boolean,
): ImageAdvancedOptions {
  return {
    ...(options.complexity ? { complexity: options.complexity } : {}),
    ...(format === "gif"
      ? {
          ...(options.frameCount !== undefined
            ? { frameCount: options.frameCount }
            : {}),
          ...(options.frameDurationMs !== undefined
            ? { frameDurationMs: options.frameDurationMs }
            : {}),
        }
      : {}),
    ...(format === "svg" && options.svg?.model.trim()
      ? { svg: options.svg }
      : {}),
    ...(!["gif", "svg"].includes(format) && options.stream !== undefined
      ? {
          stream: options.stream,
          ...(options.stream && options.partialImages !== undefined
            ? { partialImages: options.partialImages }
            : {}),
        }
      : {}),
    ...(editing && options.markupPath
      ? { markupPath: options.markupPath }
      : {}),
    ...(editing && format !== "gif" && options.maskPath
      ? { maskPath: options.maskPath }
      : {}),
  };
}
