import type { LlmConfig } from "../types";

import manifest from "../../shared/model-catalog.v1.json";

export const llmProtocols = manifest.llm.protocols;
export const llmProtocolIds = Object.keys(llmProtocols) as LlmConfig["protocol"][];
const defaultProtocol = manifest.llm.defaultProtocol as LlmConfig["protocol"];

export function createDefaultLlmConfig(): LlmConfig {
  return {
    id: crypto.randomUUID(),
    name: "",
    protocol: defaultProtocol,
    model: "",
    api_key: "",
    base_url: defaultBaseUrlForProtocol(defaultProtocol),
    capability: "text",
    enabled: false,
  };
}

export function defaultBaseUrlForProtocol(protocol: string): string {
  return (llmProtocols[protocol as LlmConfig["protocol"]] ?? llmProtocols[defaultProtocol]).baseUrl;
}
