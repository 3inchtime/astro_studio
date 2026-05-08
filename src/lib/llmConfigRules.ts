import type { LlmConfig } from "../types";

export type LlmConfigCapability = LlmConfig["capability"];

export const MAX_ENABLED_TEXT_CONFIGS = 1;
export const MAX_ENABLED_MULTIMODAL_CONFIGS = 2;
export const MAX_ENABLED_LLM_CONFIGS = 2;

export function enabledLlmConfigs(configs: LlmConfig[]): LlmConfig[] {
  return configs.filter((config) => config.enabled);
}

export function countEnabledByCapability(
  configs: LlmConfig[],
): Record<LlmConfigCapability, number> {
  return configs.reduce(
    (counts, config) => ({
      ...counts,
      [config.capability]: counts[config.capability] + (config.enabled ? 1 : 0),
    }),
    { text: 0, multimodal: 0 },
  );
}

export function normalizeLlmEnabledState(
  configs: LlmConfig[],
): LlmConfig[] {
  let textEnabled = 0;
  let multimodalEnabled = 0;

  return configs.map((config) => {
    if (!config.enabled) return config;

    if (config.capability === "text") {
      if (textEnabled >= MAX_ENABLED_TEXT_CONFIGS || textEnabled + multimodalEnabled >= MAX_ENABLED_LLM_CONFIGS) {
        return { ...config, enabled: false };
      }
      textEnabled += 1;
      return config;
    }

    if (multimodalEnabled >= MAX_ENABLED_MULTIMODAL_CONFIGS || textEnabled + multimodalEnabled >= MAX_ENABLED_LLM_CONFIGS) {
      return { ...config, enabled: false };
    }
    multimodalEnabled += 1;
    return config;
  });
}

export function canEnableLlmConfig(
  configs: LlmConfig[],
  targetId: string,
): { ok: true } | { ok: false; reason: "text_limit" | "multimodal_limit" | "total_limit" } {
  const target = configs.find((config) => config.id === targetId);
  if (!target) return { ok: true };
  if (target.enabled) return { ok: true };

  const counts = countEnabledByCapability(configs);
  const totalEnabled = counts.text + counts.multimodal;

  if (totalEnabled >= MAX_ENABLED_LLM_CONFIGS) {
    return { ok: false, reason: "total_limit" };
  }

  if (target.capability === "text" && counts.text >= MAX_ENABLED_TEXT_CONFIGS) {
    return { ok: false, reason: "text_limit" };
  }

  if (target.capability === "multimodal" && counts.multimodal >= MAX_ENABLED_MULTIMODAL_CONFIGS) {
    return { ok: false, reason: "multimodal_limit" };
  }

  return { ok: true };
}
