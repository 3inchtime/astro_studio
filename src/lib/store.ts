import { create } from "zustand";
import type { MessageImage } from "../types";

interface LightboxState {
  images: MessageImage[];
  index: number;
}

interface UIStore {
  lightbox: LightboxState | null;
  openLightbox: (images: MessageImage[], index: number) => void;
  closeLightbox: () => void;
  folderSelectorImageId: string | null;
  openFolderSelector: (imageId: string) => void;
  closeFolderSelector: () => void;
}

export const useUIStore = create<UIStore>((set) => ({
  lightbox: null,
  openLightbox: (images, index) => set({ lightbox: { images, index } }),
  closeLightbox: () => set({ lightbox: null }),

  folderSelectorImageId: null,
  openFolderSelector: (imageId) => set({ folderSelectorImageId: imageId }),
  closeFolderSelector: () => set({ folderSelectorImageId: null }),
}));
