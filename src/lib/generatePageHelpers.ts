import type { EditSourceImage, MessageImage, PromptFavorite } from "../types";

export function mergeEditSources(
  current: EditSourceImage[],
  incoming: EditSourceImage[],
): EditSourceImage[] {
  const byPath = new Map(current.map((source) => [source.path, source]));

  for (const source of incoming) {
    byPath.set(source.path, source);
  }

  return Array.from(byPath.values());
}

export function createUploadedEditSource(path: string): EditSourceImage {
  const normalizedPath = path.replace(/\\/g, "/");
  const fileName = normalizedPath.split("/").pop() || "source-image";

  return {
    id: `${crypto.randomUUID()}:${normalizedPath}`,
    path,
    label: fileName,
  };
}

export function editSourcesToMessageImages(
  sources: EditSourceImage[],
  generationId: string,
): MessageImage[] {
  return sources.map((source, index) => ({
    imageId: source.imageId ?? `${generationId}-source-${index}`,
    generationId: source.generationId ?? generationId,
    path: source.path,
    thumbnailPath: source.path,
  }));
}

export function normalizePromptFavorite(prompt: string): string {
  return prompt.trim().toLocaleLowerCase();
}

export function upsertPromptFavorite(
  current: PromptFavorite[],
  favorite: PromptFavorite,
): PromptFavorite[] {
  const normalized = normalizePromptFavorite(favorite.prompt);
  return [
    favorite,
    ...current.filter(
      (item) =>
        item.id !== favorite.id &&
        normalizePromptFavorite(item.prompt) !== normalized,
    ),
  ];
}
