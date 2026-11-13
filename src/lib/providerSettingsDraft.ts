import type {
  ComposerMode,
  PromptAgentMessage,
  RetryGenerationRequest,
} from "../types";
import { IMAGE_MODEL_CATALOG } from "./modelCatalog";
const KEY = "astro:provider-settings-draft:v1";
export interface ProviderSettingsDraft {
  request: RetryGenerationRequest;
  recipeWarnings: string[];
  composerMode: ComposerMode;
  editingPromptMessageId: string | null;
  promptAgentSessionId: string | null;
  promptAgentMessages: PromptAgentMessage[];
  selectedPromptAgentConfigId: string;
}
export function saveProviderSettingsDraft(draft: ProviderSettingsDraft) {
  sessionStorage.setItem(KEY, JSON.stringify(draft));
}
export function takeProviderSettingsDraft(): ProviderSettingsDraft | null {
  try {
    const text = sessionStorage.getItem(KEY);
    sessionStorage.removeItem(KEY);
    if (!text) return null;
    const value = JSON.parse(text) as ProviderSettingsDraft;
    if (
      !value?.request ||
      typeof value.request.prompt !== "string" ||
      !IMAGE_MODEL_CATALOG.some((entry) => entry.id === value.request.model) ||
      !Array.isArray(value.request.editSources)
    )
      return null;
    return value;
  } catch {
    return null;
  }
}
