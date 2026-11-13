import { invoke } from "@tauri-apps/api/core";
import { IMAGE_MODEL_CATALOG, getImageModelCatalogEntry } from "./modelCatalog";
import type { ImageModel, RetryGenerationRequest } from "../types";
import type { GenerationRecipe } from "../types/recipe";
export const getGenerationRecipe = (generationId: string) =>
  invoke<GenerationRecipe>("get_generation_recipe", { generationId });
export function recipeToRequest(
  recipe: GenerationRecipe,
): RetryGenerationRequest {
  if (
    recipe.version !== 1 ||
    !IMAGE_MODEL_CATALOG.some((m) => m.id === recipe.model)
  )
    throw new Error("Unsupported generation recipe");
  const model = recipe.model as ImageModel;
  const defaults = getImageModelCatalogEntry(model).parameterDefaults;
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
    .filter((profile) => profile.api_key.trim())
    .map(({ id, name }) => ({ id, name }));
}
