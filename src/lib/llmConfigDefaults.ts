import type { LlmConfig } from "../types";

export function createDefaultLlmConfig(): LlmConfig {
  return {
    id: crypto.randomUUID(),
    name: "",
    protocol: "openai",
    model: "",
    apiKey: "",
    baseUrl: "https://api.openai.com/v1",
    capability: "text",
    enabled: true,
  };
}

export function defaultBaseUrlForProtocol(protocol: string): string {
  return protocol === "anthropic"
    ? "https://api.anthropic.com"
    : "https://api.openai.com/v1";
}
