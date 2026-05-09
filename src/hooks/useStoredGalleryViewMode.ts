import { useCallback, useState } from "react";
import {
  readStoredGalleryViewMode,
  writeStoredGalleryViewMode,
  type GalleryViewMode,
} from "../lib/galleryViewMode";

export function useStoredGalleryViewMode(storageKey: string) {
  const [viewMode, setViewModeState] = useState<GalleryViewMode>(() =>
    readStoredGalleryViewMode(storageKey),
  );

  const setViewMode = useCallback(
    (nextViewMode: GalleryViewMode) => {
      setViewModeState(nextViewMode);
      writeStoredGalleryViewMode(storageKey, nextViewMode);
    },
    [storageKey],
  );

  return [viewMode, setViewMode] as const;
}
