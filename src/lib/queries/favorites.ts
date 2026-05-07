import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import * as api from "../api";
import type { PromptFavorite } from "../../types";

export function usePromptFavoritesQuery(query?: string, folderId?: string) {
  return useQuery({
    queryKey: ["prompt-favorites", { query, folderId }],
    queryFn: () => api.getPromptFavorites(query, folderId),
  });
}

export function useCreatePromptFavoriteMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (prompt: string) => api.createPromptFavorite(prompt),
    onSuccess: (favorite) => {
      queryClient.setQueriesData<PromptFavorite[]>(
        { queryKey: ["prompt-favorites"] },
        (old) => (old ? [...old, favorite] : [favorite]),
      );
    },
  });
}

export function useDeletePromptFavoriteMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.deletePromptFavorite(id),
    onSuccess: (_data, id) => {
      queryClient.setQueriesData<PromptFavorite[]>(
        { queryKey: ["prompt-favorites"] },
        (old) => (old ? old.filter((f) => f.id !== id) : []),
      );
    },
  });
}

export function usePromptFoldersQuery() {
  return useQuery({
    queryKey: ["prompt-folders"],
    queryFn: () => api.getPromptFolders(),
  });
}

export function useCreatePromptFolderMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (name: string) => api.createPromptFolder(name),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: ["prompt-folders"] }),
  });
}

export function useDeletePromptFolderMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.deletePromptFolder(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["prompt-folders"] });
      queryClient.invalidateQueries({ queryKey: ["prompt-favorites"] });
    },
  });
}
