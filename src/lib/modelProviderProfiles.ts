import type {
  ImageModel,
  ModelProviderProfile,
  ModelProviderProfilesState,
} from "../types";
import { defaultEndpointSettingsForModel } from "./settingsEndpoints";

export const DEFAULT_PROVIDER_ID = "default";
export const DEFAULT_PROVIDER_NAME = "Default";
export const NEW_PROVIDER_NAME = "New Provider";

export function defaultProviderProfileForModel(
  model: ImageModel,
): ModelProviderProfile {
  return {
    id: DEFAULT_PROVIDER_ID,
    name: DEFAULT_PROVIDER_NAME,
    api_key: "",
    endpoint_settings: defaultEndpointSettingsForModel(model),
  };
}

export function defaultProviderProfilesStateForModel(
  model: ImageModel,
): ModelProviderProfilesState {
  return {
    active_provider_id: DEFAULT_PROVIDER_ID,
    profiles: [defaultProviderProfileForModel(model)],
  };
}

export function activeProviderForState(
  state: ModelProviderProfilesState,
): ModelProviderProfile | undefined {
  return (
    state.profiles.find((profile) => profile.id === state.active_provider_id) ??
    state.profiles[0]
  );
}

export function providerForState(
  state: ModelProviderProfilesState,
  providerId: string,
): ModelProviderProfile | undefined {
  return state.profiles.find((profile) => profile.id === providerId);
}

export function updateProviderInState(
  state: ModelProviderProfilesState,
  providerId: string,
  update: (profile: ModelProviderProfile) => ModelProviderProfile,
): ModelProviderProfilesState {
  return {
    ...state,
    profiles: state.profiles.map((profile) =>
      profile.id === providerId ? update(profile) : profile,
    ),
  };
}

export function removeProviderFromState(
  state: ModelProviderProfilesState,
  providerId: string,
): ModelProviderProfilesState {
  if (state.profiles.length <= 1) {
    return state;
  }

  const profiles = state.profiles.filter((profile) => profile.id !== providerId);
  const activeProviderStillExists = profiles.some(
    (profile) => profile.id === state.active_provider_id,
  );

  return {
    active_provider_id: activeProviderStillExists
      ? state.active_provider_id
      : profiles[0].id,
    profiles,
  };
}
