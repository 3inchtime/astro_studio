import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import * as api from "../api";
import type { LlmConfig } from "../../types";

// ── LLM configs ───────────────────────────────────────────────────────────────

export function useLlmConfigsQuery() {
  return useQuery<LlmConfig[]>({
    queryKey: ["llm-configs"],
    queryFn: api.getLlmConfigs,
    staleTime: Infinity,
  });
}

export function useSaveLlmConfigsMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (configs: LlmConfig[]) => api.saveLlmConfigs(configs),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["llm-configs"] });
    },
  });
}

// ── Prompt optimization ───────────────────────────────────────────────────────

export function useOptimizePromptMutation() {
  return useMutation({
    mutationFn: ({
      prompt,
      configId,
      imagePaths,
    }: {
      prompt: string;
      configId: string;
      imagePaths?: string[];
    }) => api.optimizePrompt(prompt, configId, imagePaths),
  });
}
