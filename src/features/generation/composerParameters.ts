import { getImageModelCatalogEntry } from "../../lib/modelCatalog";
import type { ImageAdvancedOptions, ImageModel, ModelParameterDefaults, PromptAgentMessage, RetryGenerationRequest } from "../../types";

export interface ComposerParameters extends ModelParameterDefaults {
  advancedOptions: ImageAdvancedOptions;
  outputCompression: number;
}

export function defaultParameters(model: ImageModel): ComposerParameters {
  return { ...getImageModelCatalogEntry(model).parameterDefaults, advancedOptions: {}, outputCompression: 100 };
}

export function parametersFromRequest(request: RetryGenerationRequest): ComposerParameters {
  const { size, quality, background, outputFormat, moderation, inputFidelity, imageCount, aspectRatio, resolution } = request;
  return {
    size, quality, background, outputFormat, moderation, inputFidelity, imageCount, aspectRatio, resolution,
    advancedOptions: request.advancedOptions ?? {},
    outputCompression: request.outputCompression ?? 100,
  };
}

function supported<T>(value: T, choices: readonly T[] | undefined, fallback: T): T {
  return choices?.includes(value) ? value : fallback;
}

export function reconcileParameters(draft: ComposerParameters, model: ImageModel): ComposerParameters {
  const { parameterDefaults: defaults, parameterCapabilities: options, supportsEdit } = getImageModelCatalogEntry(model);
  return {
    ...draft,
    aspectRatio: supported(draft.aspectRatio, options.aspectRatios, defaults.aspectRatio),
    resolution: supported(draft.resolution, options.resolutions, defaults.resolution),
    size: supported(draft.size, options.sizes, defaults.size),
    quality: supported(draft.quality, options.qualities, defaults.quality),
    background: supported(draft.background, options.backgrounds, defaults.background),
    outputFormat: supported(draft.outputFormat, options.outputFormats, defaults.outputFormat),
    moderation: supported(draft.moderation, options.moderationLevels, defaults.moderation),
    imageCount: supported(draft.imageCount, options.imageCounts, defaults.imageCount),
    inputFidelity: supported(draft.inputFidelity, supportsEdit ? options.inputFidelityOptions : undefined, defaults.inputFidelity),
  };
}

export function applyAgentSuggestions(
  request: RetryGenerationRequest,
  prompt: string,
  suggested: PromptAgentMessage["suggested_params"],
): RetryGenerationRequest {
  const model = suggested.model ?? request.model;
  const defaults = model === request.model ? request : getImageModelCatalogEntry(model).parameterDefaults;
  return {
    ...request, model, prompt, aspectRatio: defaults.aspectRatio, resolution: defaults.resolution,
    size: suggested.size ?? request.size, quality: suggested.quality ?? request.quality,
    background: suggested.background ?? request.background, outputFormat: suggested.output_format ?? request.outputFormat,
    moderation: suggested.moderation ?? request.moderation, imageCount: suggested.image_count ?? request.imageCount,
  };
}
