import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import * as api from "../api";
import type { LlmConfig, PromptExtraction } from "../../types";

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

export function usePromptExtractionsQuery(limit = 20) {
  return useQuery<PromptExtraction[]>({
    queryKey: ["prompt-extractions", { limit }],
    queryFn: () => api.getPromptExtractions(limit),
  });
}

export function useExtractPromptFromImageMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      imagePath,
      configId,
      language,
    }: {
      imagePath: string;
      configId: string;
      language: string;
    }) => api.extractPromptFromImage(imagePath, configId, language),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["prompt-extractions"] });
    },
  });
}
