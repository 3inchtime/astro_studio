import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import * as api from "../api";
import type { GenerationSearchFilters } from "../../types";

export function useGenerationsSearchQuery(
  query?: string,
  page?: number,
  filters?: GenerationSearchFilters,
  projectId?: string | null,
) {
  return useQuery({
    queryKey: ["generations", { query, page, filters, projectId }],
    queryFn: () => api.searchGenerations(query, page, false, filters, projectId),
    placeholderData: (prev) => prev,
  });
}

export function useDeleteGenerationMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.deleteGeneration(id),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["generations"] }),
  });
}

export function useRestoreGenerationMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.restoreGeneration(id),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["generations"] }),
  });
}

export function usePermanentlyDeleteGenerationMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.permanentlyDeleteGeneration(id),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["generations"] }),
  });
}
