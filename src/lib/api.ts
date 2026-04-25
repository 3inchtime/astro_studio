import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { GenerationParams, SearchResult, Conversation, GenerationResult, Folder } from "../types";

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

export async function generateImage(params: GenerationParams): Promise<{ generation_id: string; conversation_id: string; image_paths: string[] }> {
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

export async function getConversations(query?: string): Promise<Conversation[]> {
  return invoke("get_conversations", { query: query || null });
}

export async function getConversationGenerations(
  conversationId: string,
): Promise<GenerationResult[]> {
  return invoke("get_conversation_generations", { conversationId });
}

export async function copyImageToClipboard(imagePath: string): Promise<void> {
  await invoke("copy_image_to_clipboard", { imagePath });
}

export async function saveImageToFile(imagePath: string): Promise<void> {
  await invoke("save_image_to_file", { imagePath });
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

export async function createFolder(name: string): Promise<Folder> {
  return invoke("create_folder", { name });
}

export async function renameFolder(id: string, name: string): Promise<void> {
  return invoke("rename_folder", { id, name });
}

export async function deleteFolder(id: string): Promise<void> {
  return invoke("delete_folder", { id });
}

export async function getFolders(): Promise<Folder[]> {
  return invoke("get_folders");
}

export async function addImageToFolders(imageId: string, folderIds: string[]): Promise<void> {
  return invoke("add_image_to_folders", { imageId, folderIds });
}

export async function removeImageFromFolders(imageId: string, folderIds: string[]): Promise<void> {
  return invoke("remove_image_from_folders", { imageId, folderIds });
}

export async function getImageFolders(imageId: string): Promise<string[]> {
  return invoke("get_image_folders", { imageId });
}

export async function getFavoriteImages(
  folderId?: string,
  query?: string,
  page?: number,
): Promise<SearchResult> {
  return invoke("get_favorite_images", { folderId: folderId || null, query: query || null, page });
}
