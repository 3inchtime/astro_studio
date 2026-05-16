import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as api from "../api";
import type { CanvasDocumentContent } from "../../types";

export function useCanvasDocumentsQuery(projectId?: string | null) {
  return useQuery({
    queryKey: ["canvas-documents", projectId],
    queryFn: () => api.listCanvasDocuments(projectId),
  });
}

export function useCanvasDocumentQuery(id?: string | null) {
  return useQuery({
    queryKey: ["canvas-document", id],
    queryFn: () => (id ? api.getCanvasDocument(id) : Promise.resolve(null)),
    enabled: Boolean(id),
  });
}

export function useCreateCanvasDocumentMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      projectId,
      name,
    }: {
      projectId?: string | null;
      name?: string | null;
    }) => api.createCanvasDocument(projectId, name),
    onSuccess: (_, variables) => {
      queryClient.invalidateQueries({
        queryKey: ["canvas-documents", variables.projectId ?? null],
      });
    },
  });
}

export function useSaveCanvasDocumentMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      id,
      content,
      previewPngBase64,
    }: {
      id: string;
      content: CanvasDocumentContent;
      previewPngBase64?: string | null;
    }) => api.saveCanvasDocument(id, content, previewPngBase64),
    onSuccess: (document) => {
      queryClient.invalidateQueries({
        queryKey: ["canvas-documents", document.project_id],
      });
      queryClient.invalidateQueries({
        queryKey: ["canvas-document", document.id],
      });
    },
  });
}

export function useRenameCanvasDocumentMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) =>
      api.renameCanvasDocument(id, name),
    onSuccess: (document) => {
      queryClient.invalidateQueries({
        queryKey: ["canvas-documents", document.project_id],
      });
      queryClient.invalidateQueries({
        queryKey: ["canvas-document", document.id],
      });
    },
  });
}

export function useDeleteCanvasDocumentMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => api.deleteCanvasDocument(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["canvas-documents"] });
    },
  });
}
