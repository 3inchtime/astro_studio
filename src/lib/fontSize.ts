import type { AppFontSize } from "../types";

export const DEFAULT_APP_FONT_SIZE: AppFontSize = "medium";
export const APP_FONT_SIZE_STORAGE_KEY = "astro-studio:font-size";
export const APP_FONT_SIZE_OPTIONS: AppFontSize[] = ["small", "medium", "large"];

export function isAppFontSize(value: string | null | undefined): value is AppFontSize {
  return value === "small" || value === "medium" || value === "large";
}

export function getStoredAppFontSize(): AppFontSize {
  if (typeof window === "undefined") return DEFAULT_APP_FONT_SIZE;

  try {
    const value = window.localStorage.getItem(APP_FONT_SIZE_STORAGE_KEY);
    return isAppFontSize(value) ? value : DEFAULT_APP_FONT_SIZE;
  } catch {
    return DEFAULT_APP_FONT_SIZE;
  }
}

export function applyAppFontSize(fontSize: AppFontSize) {
  if (typeof document !== "undefined") {
    document.documentElement.dataset.fontSize = fontSize;
  }

  if (typeof window !== "undefined") {
    try {
      window.localStorage.setItem(APP_FONT_SIZE_STORAGE_KEY, fontSize);
    } catch {
      // Ignore storage failures so UI scaling still works in restricted contexts.
    }
  }
}
