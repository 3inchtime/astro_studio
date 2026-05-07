import type { EditSourceImage } from "../types";

const PENDING_EDIT_SOURCES_KEY = "astro-studio.pending-edit-sources";

export function buildEditSource(
  imagePath: string,
  imageId: string,
  generationId: string,
): EditSourceImage {
  const normalizedPath = imagePath.replace(/\\/g, "/");
  const fileName = normalizedPath.split("/").pop() || "source-image";

  return {
    id: `${imageId}:${normalizedPath}`,
    path: imagePath,
    label: fileName,
    imageId,
    generationId,
  };
}

export function savePendingEditSources(sources: EditSourceImage[]): void {
  if (typeof window === "undefined") return;

  sessionStorage.setItem(PENDING_EDIT_SOURCES_KEY, JSON.stringify(sources));
}

export function consumePendingEditSources(): EditSourceImage[] {
  if (typeof window === "undefined") return [];

  const raw = sessionStorage.getItem(PENDING_EDIT_SOURCES_KEY);
  if (!raw) return [];

  sessionStorage.removeItem(PENDING_EDIT_SOURCES_KEY);

  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];

    return parsed.filter((item): item is EditSourceImage => {
      return (
        typeof item === "object" &&
        item !== null &&
        typeof item.id === "string" &&
        typeof item.path === "string" &&
        typeof item.label === "string"
      );
    });
  } catch {
    return [];
  }
}
