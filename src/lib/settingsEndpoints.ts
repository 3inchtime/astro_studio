import type { EndpointSettings, ImageModel } from "../types";
import { getImageModelCatalogEntry } from "./modelCatalog";

export function defaultBaseUrlForModel(model: ImageModel): string {
  return getImageModelCatalogEntry(model).connectionDefaults.baseUrl;
}

export function defaultGenerationUrlForModel(model: ImageModel): string {
  return getImageModelCatalogEntry(model).connectionDefaults.generationUrl;
}

export function defaultEditUrlForModel(model: ImageModel): string {
  return getImageModelCatalogEntry(model).connectionDefaults.editUrl;
}

export function defaultEndpointSettingsForModel(
  model: ImageModel,
): EndpointSettings {
  return {
    mode: "base_url",
    base_url: defaultBaseUrlForModel(model),
    generation_url: defaultGenerationUrlForModel(model),
    edit_url: defaultEditUrlForModel(model),
  };
}

export function modelSupportsEdit(model: ImageModel): boolean {
  return getImageModelCatalogEntry(model).supportsEdit;
}

export function usesSharedEditEndpoint(model: ImageModel): boolean {
  const { generationUrl, editUrl } =
    getImageModelCatalogEntry(model).connectionDefaults;

  return generationUrl === editUrl;
}

export function normalizeEndpointSettings(
  model: ImageModel,
  settings: EndpointSettings,
): EndpointSettings {
  const defaults = defaultEndpointSettingsForModel(model);
  let generationUrl = settings.generation_url.trim() || defaults.generation_url;
  for (const [oldId, newId] of [
    ["gemini-3.1-flash-image-preview", "gemini-3.1-flash-image"],
    ["gemini-3-pro-image-preview", "gemini-3-pro-image"],
  ]) {
    if (
      generationUrl ===
      `https://generativelanguage.googleapis.com/v1beta/models/${oldId}:generateContent`
    ) {
      generationUrl = `https://generativelanguage.googleapis.com/v1beta/models/${newId}:generateContent`;
    }
  }
  const editUrl =
    !modelSupportsEdit(model) || usesSharedEditEndpoint(model)
      ? generationUrl
      : settings.edit_url.trim() || defaults.edit_url;

  return {
    mode: settings.mode,
    base_url: settings.base_url.trim() || defaults.base_url,
    generation_url: generationUrl,
    edit_url: editUrl,
  };
}
