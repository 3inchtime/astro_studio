export type GalleryViewMode = "masonry" | "list";

export const DEFAULT_GALLERY_VIEW_MODE: GalleryViewMode = "masonry";

export function isGalleryViewMode(value: unknown): value is GalleryViewMode {
  return value === "masonry" || value === "list";
}

export function readStoredGalleryViewMode(
  storageKey: string,
): GalleryViewMode {
  if (typeof window === "undefined") return DEFAULT_GALLERY_VIEW_MODE;

  try {
    const stored = window.localStorage.getItem(storageKey);
    return isGalleryViewMode(stored) ? stored : DEFAULT_GALLERY_VIEW_MODE;
  } catch {
    return DEFAULT_GALLERY_VIEW_MODE;
  }
}

export function writeStoredGalleryViewMode(
  storageKey: string,
  viewMode: GalleryViewMode,
) {
  try {
    window.localStorage.setItem(storageKey, viewMode);
  } catch {
    // Storage can be unavailable in restricted browser contexts.
  }
}
