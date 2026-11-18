import { invoke } from "@tauri-apps/api/core";
import { findImageModel } from "./modelCatalog";
import type { RetryGenerationRequest } from "../types";
import type { GenerationRecipe } from "../types/recipe";
export const getGenerationRecipe = (generationId: string) =>
  invoke<GenerationRecipe>("get_generation_recipe", { generationId });
export const getGenerationRecipeForRequest = (clientRequestId: string) =>
  invoke<GenerationRecipe | null>("get_generation_recipe", { clientRequestId });
export function recipeToRequest(
  recipe: GenerationRecipe,
): RetryGenerationRequest {
  const entry = findImageModel(recipe.model);
  if (recipe.version !== 1 || !entry) throw new Error("Unsupported generation recipe");
  const { id: model, parameterDefaults: defaults } = entry;
  const o = recipe.options;
  return {
    prompt: recipe.prompt,
    model,
    sourceGenerationId: recipe.generation_id,
    providerProfileId: recipe.provider_profile_id ?? undefined,
    size: o.size ?? defaults.size,
    quality: o.quality ?? defaults.quality,
    background: o.background ?? defaults.background,
    outputFormat: o.output_format ?? defaults.outputFormat,
    outputCompression: o.output_compression,
    moderation: o.moderation ?? defaults.moderation,
    inputFidelity: o.input_fidelity ?? defaults.inputFidelity,
    imageCount: o.image_count ?? defaults.imageCount,
    aspectRatio: o.aspect_ratio,
    resolution: o.resolution,
    advancedOptions: {
      ...(o.stream !== undefined ? { stream: o.stream } : {}),
      ...(o.partial_images !== undefined
        ? { partialImages: o.partial_images }
        : {}),
      ...structuredClone(o.advanced_options ?? {}),
    },
    editSources: recipe.source_image_paths.map((path, i) => ({
      id: `${recipe.generation_id}-source-${i}`,
      path,
      label: path.split(/[\\/]/).pop() ?? path,
    })),
  };
}

export async function listRecipeProviders(
  model: string,
): Promise<{ id: string; name: string }[]> {
  const state = await invoke<import("../types").ModelProviderProfilesState>(
    "get_model_provider_profiles",
    { model },
  );
  return state.profiles
    .filter((profile) => profile.api_key_configured)
    .map(({ id, name }) => ({ id, name }));
}
