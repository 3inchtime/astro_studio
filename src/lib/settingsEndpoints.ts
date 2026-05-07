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

export function defaultEndpointSettingsForModel(model: ImageModel): EndpointSettings {
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
  const { generationUrl, editUrl } = getImageModelCatalogEntry(model).connectionDefaults;

  return generationUrl === editUrl;
}

export function normalizeEndpointSettings(
  model: ImageModel,
  settings: EndpointSettings,
): EndpointSettings {
  const defaults = defaultEndpointSettingsForModel(model);
  const generationUrl = settings.generation_url.trim() || defaults.generation_url;
  const editUrl = !modelSupportsEdit(model) || usesSharedEditEndpoint(model)
    ? generationUrl
    : settings.edit_url.trim() || defaults.edit_url;

  return {
    mode: settings.mode,
    base_url: settings.base_url.trim() || defaults.base_url,
    generation_url: generationUrl,
    edit_url: editUrl,
  };
}
