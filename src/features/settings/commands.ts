import { invoke } from "@tauri-apps/api/core";
import { credentialWrite, credentialDraft } from "../../lib/credentialDraft";
import { flushCanvasSaves } from "../../lib/canvas/persistence";
import type { LogSettings, TrashSettings, AppFontSize, ImageModel, EndpointSettings, ModelProviderProfile, ModelProviderProfilesState } from "../../types";

export async function saveApiKey(key: string): Promise<void> {
  await invoke("save_api_key", { key });
}

export async function getApiKey(): Promise<import("../../types").CredentialStatus> {
  return invoke("get_api_key");
}

export async function saveBaseUrl(url: string): Promise<void> {
  await invoke("save_base_url", { url });
}

export async function getBaseUrl(): Promise<string> {
  return invoke("get_base_url");
}

export async function getEndpointSettings(): Promise<EndpointSettings> {
  return invoke("get_endpoint_settings");
}

export async function getModelApiKey(
  model: ImageModel,
): Promise<import("../../types").CredentialStatus> {
  return invoke("get_model_api_key", { model });
}

export async function saveModelApiKey(
  model: ImageModel,
  key: string,
): Promise<void> {
  await invoke("save_model_api_key", { model, key });
}

export async function saveEndpointSettings(
  settings: EndpointSettings,
): Promise<void> {
  await invoke("save_endpoint_settings", {
    mode: settings.mode,
    baseUrl: settings.base_url,
    generationUrl: settings.generation_url,
    editUrl: settings.edit_url,
  });
}

export async function getModelEndpointSettings(
  model: ImageModel,
): Promise<EndpointSettings> {
  return invoke("get_model_endpoint_settings", { model });
}

export async function saveModelEndpointSettings(
  model: ImageModel,
  settings: EndpointSettings,
): Promise<void> {
  await invoke("save_model_endpoint_settings", {
    model,
    mode: settings.mode,
    baseUrl: settings.base_url,
    generationUrl: settings.generation_url,
    editUrl: settings.edit_url,
  });
}

export async function getModelProviderProfiles(
  model: ImageModel,
): Promise<ModelProviderProfilesState> {
  const state = await invoke<ModelProviderProfilesState>("get_model_provider_profiles", { model });
  return { ...state, profiles: state.profiles.map(credentialDraft) };
}

export async function saveModelProviderProfiles(
  model: ImageModel,
  state: ModelProviderProfilesState,
): Promise<ModelProviderProfilesState> {
  return invoke("save_model_provider_profiles", {
    model,
    activeProviderId: state.active_provider_id,
    profiles: state.profiles.map(credentialWrite),
  });
}

export async function createModelProviderProfile(
  model: ImageModel,
  name: string,
): Promise<ModelProviderProfilesState> {
  return invoke("create_model_provider_profile", { model, name });
}

export async function deleteModelProviderProfile(
  model: ImageModel,
  providerId: string,
): Promise<ModelProviderProfilesState> {
  return invoke("delete_model_provider_profile", { model, providerId });
}

export async function setActiveModelProvider(
  model: ImageModel,
  providerId: string,
): Promise<ModelProviderProfilesState> {
  return invoke("set_active_model_provider", { model, providerId });
}

export type { ModelProviderProfile, ModelProviderProfilesState };

export async function getImageModel(): Promise<ImageModel> {
  return invoke("get_image_model");
}

export async function saveImageModel(model: ImageModel): Promise<void> {
  await invoke("save_image_model", { model });
}

export async function getLogSettings(): Promise<LogSettings> {
  return invoke("get_log_settings");
}

export async function saveLogSettings(
  enabled: boolean,
  retentionDays: number,
): Promise<void> {
  await invoke("save_log_settings", { enabled, retentionDays });
}

export async function getTrashSettings(): Promise<TrashSettings> {
  return invoke("get_trash_settings");
}

export async function saveTrashSettings(retentionDays: number): Promise<void> {
  // Updating retention also purges expired images, including canvas sources.
  await flushCanvasSaves();
  await invoke("save_trash_settings", { retentionDays });
}

export async function getFontSize(): Promise<AppFontSize> {
  return invoke("get_font_size");
}

export async function saveFontSize(fontSize: AppFontSize): Promise<void> {
  await invoke("save_font_size", { fontSize });
}

export async function getCredentialStatus(): Promise<{ready: boolean; message: string | null}> {
  return invoke("get_credential_status");
}
