import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as api from "../../lib/api";
import type { PromptExtraction } from "../../types";

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
