import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as api from "../api";

export function usePromptAgentSessionQuery(sessionId?: string | null) {
  return useQuery({
    queryKey: ["prompt-agent-session", sessionId],
    queryFn: () => api.getPromptAgentSession(sessionId as string),
    enabled: Boolean(sessionId),
  });
}

export function useStartPromptAgentSessionMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: api.startPromptAgentSession,
    onSuccess: (result) => {
      queryClient.setQueryData(["prompt-agent-session", result.session.id], result);
    },
  });
}

export function useSendPromptAgentMessageMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: api.sendPromptAgentMessage,
    onSuccess: (result) => {
      queryClient.setQueryData(["prompt-agent-session", result.session.id], result);
    },
  });
}

export function useAcceptPromptAgentDraftMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      sessionId,
      acceptedPrompt,
    }: {
      sessionId: string;
      acceptedPrompt: string;
    }) => api.acceptPromptAgentDraft(sessionId, acceptedPrompt),
    onSuccess: (session) => {
      queryClient.invalidateQueries({ queryKey: ["prompt-agent-session", session.id] });
    },
  });
}

export function useCancelPromptAgentSessionMutation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: api.cancelPromptAgentSession,
    onSuccess: (session) => {
      queryClient.invalidateQueries({ queryKey: ["prompt-agent-session", session.id] });
    },
  });
}
