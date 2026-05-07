import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import * as api from "../api";
import type {
  AppFontSize,
  EndpointSettings,
  ImageModel,
  ModelProviderProfilesState,
} from "../../types";

// ── Image model ──────────────────────────────────────────────────────────────

export function useImageModelQuery() {
  return useQuery({
    queryKey: ["settings", "image-model"],
    queryFn: () => api.getImageModel(),
  });
}

export function useSaveImageModelMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (model: ImageModel) => api.saveImageModel(model),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["settings", "image-model"] }),
  });
}

// ── Font size ────────────────────────────────────────────────────────────────

export function useFontSizeQuery() {
  return useQuery({
    queryKey: ["settings", "font-size"],
    queryFn: () => api.getFontSize(),
  });
}

export function useSaveFontSizeMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (fontSize: AppFontSize) => api.saveFontSize(fontSize),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["settings", "font-size"] }),
  });
}

// ── API key ──────────────────────────────────────────────────────────────────

export function useModelApiKeyQuery(model: ImageModel) {
  return useQuery({
    queryKey: ["settings", "api-key", model],
    queryFn: () => api.getModelApiKey(model),
  });
}

// ── Endpoint settings ────────────────────────────────────────────────────────

export function useModelEndpointSettingsQuery(model: ImageModel) {
  return useQuery({
    queryKey: ["settings", "endpoint", model],
    queryFn: () => api.getModelEndpointSettings(model),
  });
}

export function useSaveModelEndpointSettingsMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      model,
      settings,
    }: {
      model: ImageModel;
      settings: EndpointSettings;
    }) => api.saveModelEndpointSettings(model, settings),
    onSuccess: (_data, variables) =>
      queryClient.invalidateQueries({
        queryKey: ["settings", "endpoint", variables.model],
      }),
  });
}

// ── Provider profiles ────────────────────────────────────────────────────────

export function useModelProviderProfilesQuery(model: ImageModel) {
  return useQuery({
    queryKey: ["settings", "provider-profiles", model],
    queryFn: () => api.getModelProviderProfiles(model),
  });
}

export function useSaveModelProviderProfilesMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      model,
      state,
    }: {
      model: ImageModel;
      state: ModelProviderProfilesState;
    }) => api.saveModelProviderProfiles(model, state),
    onSuccess: (_data, variables) =>
      queryClient.invalidateQueries({
        queryKey: ["settings", "provider-profiles", variables.model],
      }),
  });
}

export function useCreateModelProviderProfileMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      model,
      name,
    }: {
      model: ImageModel;
      name: string;
    }) => api.createModelProviderProfile(model, name),
    onSuccess: (_data, variables) =>
      queryClient.invalidateQueries({
        queryKey: ["settings", "provider-profiles", variables.model],
      }),
  });
}

export function useDeleteModelProviderProfileMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      model,
      providerId,
    }: {
      model: ImageModel;
      providerId: string;
    }) => api.deleteModelProviderProfile(model, providerId),
    onSuccess: (_data, variables) =>
      queryClient.invalidateQueries({
        queryKey: ["settings", "provider-profiles", variables.model],
      }),
  });
}

export function useSetActiveModelProviderMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      model,
      providerId,
    }: {
      model: ImageModel;
      providerId: string;
    }) => api.setActiveModelProvider(model, providerId),
    onSuccess: (_data, variables) =>
      queryClient.invalidateQueries({
        queryKey: ["settings", "provider-profiles", variables.model],
      }),
  });
}

// ── Logs ─────────────────────────────────────────────────────────────────────

export function useLogsQuery(
  logType?: string,
  level?: string,
  page?: number,
  pageSize?: number,
) {
  return useQuery({
    queryKey: ["logs", { logType, level, page, pageSize }],
    queryFn: () => api.getLogs(logType, level, page, pageSize),
  });
}

export function useLogSettingsQuery() {
  return useQuery({
    queryKey: ["settings", "logs"],
    queryFn: () => api.getLogSettings(),
  });
}

export function useSaveLogSettingsMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      enabled,
      retentionDays,
    }: {
      enabled: boolean;
      retentionDays: number;
    }) => api.saveLogSettings(enabled, retentionDays),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["settings", "logs"] }),
  });
}

export function useClearLogsMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (beforeDays?: number) => api.clearLogs(beforeDays),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["logs"] }),
  });
}

// ── Trash ────────────────────────────────────────────────────────────────────

export function useTrashSettingsQuery() {
  return useQuery({
    queryKey: ["settings", "trash"],
    queryFn: () => api.getTrashSettings(),
  });
}

export function useSaveTrashSettingsMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (retentionDays: number) => api.saveTrashSettings(retentionDays),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["settings", "trash"] }),
  });
}
