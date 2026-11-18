import { invoke } from "@tauri-apps/api/core";
import type { PromptAgentSession, PromptAgentTurnResponse } from "../../types";

export async function startPromptAgentSession(params: {
  prompt: string;
  configId: string;
  conversationId?: string | null;
  projectId?: string | null;
  sourceImagePaths?: string[];
  visualSourceImagePaths?: string[];
  textOnly?: boolean;
}): Promise<PromptAgentTurnResponse> {
  return invoke("start_prompt_agent_session", {
    request: {
      prompt: params.prompt,
      config_id: params.configId,
      conversation_id: params.conversationId ?? null,
      project_id: params.projectId ?? null,
      source_image_paths: params.sourceImagePaths ?? [],
      ...(params.visualSourceImagePaths !== undefined ? { visual_source_image_paths: params.visualSourceImagePaths } : {}),
      text_only: params.textOnly ?? false,
    },
  });
}

export async function sendPromptAgentMessage(params: {
  sessionId: string;
  message: string;
  configId: string;
  sourceImagePaths?: string[];
  visualSourceImagePaths?: string[];
  textOnly?: boolean;
}): Promise<PromptAgentTurnResponse> {
  return invoke("send_prompt_agent_message", {
    request: {
      session_id: params.sessionId,
      message: params.message,
      config_id: params.configId,
      source_image_paths: params.sourceImagePaths ?? [],
      ...(params.visualSourceImagePaths !== undefined ? { visual_source_image_paths: params.visualSourceImagePaths } : {}),
      text_only: params.textOnly ?? false,
    },
  });
}

export async function acceptPromptAgentDraft(
  sessionId: string,
  acceptedPrompt: string,
): Promise<PromptAgentSession> {
  return invoke("accept_prompt_agent_draft", {
    sessionId,
    acceptedPrompt,
  });
}

export async function cancelPromptAgentSession(
  sessionId: string,
): Promise<PromptAgentSession> {
  return invoke("cancel_prompt_agent_session", { sessionId });
}

export async function getPromptAgentSession(
  sessionId: string,
): Promise<PromptAgentTurnResponse> {
  return invoke("get_prompt_agent_session", { sessionId });
}
