import { invoke } from "@tauri-apps/api/core";
import { flushCanvasSaves } from "../../lib/canvas/persistence";
import type { GenerationSearchFilters, SearchResult, Folder, PromptFavorite } from "../../types";

export async function createPromptFavorite(
  prompt: string,
): Promise<PromptFavorite> {
  return invoke("create_prompt_favorite", { prompt });
}

export async function getPromptFavorites(
  query?: string,
  folderId?: string,
): Promise<PromptFavorite[]> {
  return invoke("get_prompt_favorites", {
    query: query || null,
    folderId: folderId || null,
  });
}

export async function deletePromptFavorite(id: string): Promise<void> {
  await invoke("delete_prompt_favorite", { id });
}

export async function createPromptFolder(name: string): Promise<Folder> {
  return invoke("create_prompt_folder", { name });
}

export async function renamePromptFolder(
  id: string,
  name: string,
): Promise<void> {
  return invoke("rename_prompt_folder", { id, name });
}

export async function deletePromptFolder(id: string): Promise<void> {
  await invoke("delete_prompt_folder", { id });
}

export async function getPromptFolders(): Promise<Folder[]> {
  return invoke("get_prompt_folders");
}

export async function addPromptFavoriteToFolders(
  favoriteId: string,
  folderIds: string[],
): Promise<void> {
  return invoke("add_prompt_favorite_to_folders", { favoriteId, folderIds });
}

export async function removePromptFavoriteFromFolders(
  favoriteId: string,
  folderIds: string[],
): Promise<void> {
  return invoke("remove_prompt_favorite_from_folders", {
    favoriteId,
    folderIds,
  });
}

export async function getPromptFavoriteFolders(
  favoriteId: string,
): Promise<string[]> {
  return invoke("get_prompt_favorite_folders", { favoriteId });
}

export async function searchGenerations(
  query?: string,
  page?: number,
  onlyDeleted?: boolean,
  filters?: GenerationSearchFilters,
  projectId?: string | null,
): Promise<SearchResult> {
  return invoke("search_generations", {
    query: query || null,
    page,
    onlyDeleted: onlyDeleted || null,
    filters: filters || null,
    projectId: projectId || null,
  });
}

export async function deleteGeneration(id: string): Promise<void> {
  await invoke("delete_generation", { id });
}

export async function restoreGeneration(id: string): Promise<void> {
  await invoke("restore_generation", { id });
}

export async function permanentlyDeleteGeneration(id: string): Promise<void> {
  await flushCanvasSaves();
  await invoke("permanently_delete_generation", { id });
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

export async function addImageToFolders(
  imageId: string,
  folderIds: string[],
): Promise<void> {
  return invoke("add_image_to_folders", { imageId, folderIds });
}

export async function removeImageFromFolders(
  imageId: string,
  folderIds: string[],
): Promise<void> {
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
  return invoke("get_favorite_images", {
    folderId: folderId || null,
    query: query || null,
    page,
  });
}
