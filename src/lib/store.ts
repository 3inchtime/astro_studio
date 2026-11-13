import { create } from "zustand";
import type { MessageImage } from "../types";

export interface PreviewSource {
  imageId: string;
  rect?: { left: number; top: number; width: number; height: number };
}

export function capturePreviewSource(element: HTMLElement, imageId: string): PreviewSource {
  const rect = element.getBoundingClientRect();
  return { imageId, rect: rect.width > 0 && rect.height > 0 ? { left: rect.left, top: rect.top, width: rect.width, height: rect.height } : undefined };
}

interface LightboxState {
  source?: PreviewSource;
  images: MessageImage[];
  index: number;
}

interface UIStore {
  lightbox: LightboxState | null;
  openLightbox: (images: MessageImage[], index: number, source?: PreviewSource) => void;
  closeLightbox: () => void;
  folderSelectorImageId: string | null;
  openFolderSelector: (imageId: string) => void;
  closeFolderSelector: () => void;
}

export const useUIStore = create<UIStore>((set) => ({
  lightbox: null,
  openLightbox: (images, index, source) => set({ lightbox: { images, index, source } }),
  closeLightbox: () => set({ lightbox: null }),

  folderSelectorImageId: null,
  openFolderSelector: (imageId) => set({ folderSelectorImageId: imageId }),
  closeFolderSelector: () => set({ folderSelectorImageId: null }),
}));
