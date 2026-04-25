import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { GenerationParams, SearchResult } from "../types";

export function toAssetUrl(filePath: string): string {
  return convertFileSrc(filePath.replace(/\\/g, "/"));
}

export async function saveApiKey(key: string): Promise<void> {
  await invoke("save_api_key", { key });
}

export async function getApiKey(): Promise<string | null> {
  return invoke("get_api_key");
}

export async function saveBaseUrl(url: string): Promise<void> {
  await invoke("save_base_url", { url });
}

export async function getBaseUrl(): Promise<string> {
  return invoke("get_base_url");
}

export async function generateImage(params: GenerationParams): Promise<{ generation_id: string; image_paths: string[] }> {
  return invoke("generate_image", {
    prompt: params.prompt,
    size: params.size,
    quality: params.quality,
  });
}

export async function searchGenerations(
  query?: string,
  page?: number,
): Promise<SearchResult> {
  return invoke("search_generations", { query: query || null, page });
}

export async function deleteGeneration(id: string): Promise<void> {
  await invoke("delete_generation", { id });
}

function onGenerationEvent<T>(event: string, handler: (data: T) => void) {
  return listen(event, (e) => handler(e.payload as T));
}

export function onGenerationProgress(
  handler: (data: { generation_id: string; status: string }) => void,
) {
  return onGenerationEvent("generation:progress", handler);
}

export function onGenerationComplete(
  handler: (data: { generation_id: string; status: string }) => void,
) {
  return onGenerationEvent("generation:complete", handler);
}

export function onGenerationFailed(
  handler: (data: { generation_id: string; error: string }) => void,
) {
  return onGenerationEvent("generation:failed", handler);
}
