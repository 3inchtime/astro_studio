import type {
  ComposerMode,
  PromptAgentMessage,
  RetryGenerationRequest,
} from "../types";
import { findImageModel } from "./modelCatalog";
const KEY = "astro:provider-settings-draft:v1";
export interface ProviderSettingsDraft {
  request: RetryGenerationRequest;
  recipeWarnings: string[];
  composerMode: ComposerMode;
  editingPromptMessageId: string | null;
  promptAgentSessionId: string | null;
  promptAgentMessages: PromptAgentMessage[];
  selectedPromptAgentConfigId: string;
  agentTextOnly?: boolean;
  agentVisualSelection?: string[] | null;
}
export function saveProviderSettingsDraft(draft: ProviderSettingsDraft) {
  sessionStorage.setItem(KEY, JSON.stringify(draft));
}
function readDraft(key: string, consume: boolean): ProviderSettingsDraft | null {
  try {
    const text = sessionStorage.getItem(key);
    if (consume) sessionStorage.removeItem(key);
    if (!text) return null;
    const value = JSON.parse(text) as ProviderSettingsDraft;
    const model = findImageModel(value?.request?.model);
    if (
      !value?.request ||
      typeof value.request.prompt !== "string" ||
      !model ||
      !Array.isArray(value.request.editSources)
    )
      return null;
    value.request.model = model.id;
    return value;
  } catch {
    return null;
  }
}

export function takeProviderSettingsDraft() {
  return readDraft(KEY, true);
}

export function conversationDraftKey(project: string | null, conversation: string | null) {
  return "astro:composer:v1:" + JSON.stringify([project ?? "default", conversation]);
}

// Large browser-preview source images may exceed the session storage quota.
const memoryDrafts = new Map<string, ProviderSettingsDraft | null>();

export function readConversationDraft(key: string) {
  return memoryDrafts.has(key) ? memoryDrafts.get(key) ?? null : readDraft(key, false);
}

export function writeConversationDraft(key: string, draft: ProviderSettingsDraft | null) {
  try {
    if (draft) sessionStorage.setItem(key, JSON.stringify(draft));
    else sessionStorage.removeItem(key);
    memoryDrafts.delete(key);
  } catch {
    memoryDrafts.set(key, draft);
  }
}
