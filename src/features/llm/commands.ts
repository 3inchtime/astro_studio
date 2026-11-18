import { invoke } from "@tauri-apps/api/core";
import { credentialWrite, credentialDraft } from "../../lib/credentialDraft";
import type { LlmConfig } from "../../types";

export async function getLlmConfigs(): Promise<LlmConfig[]> {
  return (await invoke<LlmConfig[]>("get_llm_configs")).map(credentialDraft);
}

export async function saveLlmConfigs(configs: LlmConfig[]): Promise<void> {
  return invoke("save_llm_configs", { configs: configs.map(credentialWrite) });
}

export async function optimizePrompt(
  prompt: string,
  configId: string,
  imagePaths?: string[],
): Promise<string> {
  return invoke("optimize_prompt", {
    prompt,
    configId,
    imagePaths: imagePaths ?? null,
  });
}
