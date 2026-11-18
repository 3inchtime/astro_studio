import { useCallback, useState } from "react";

export type GenerationMode = "traditional" | "nodes";
const preferenceKey = "astro:generation-mode:v1";

export function useGenerationMode() {
  const [mode, setMode] = useState<GenerationMode>(() => {
    try {
      return sessionStorage.getItem(preferenceKey) === "nodes" ? "nodes" : "traditional";
    } catch {
      return "traditional";
    }
  });
  const selectMode = useCallback((next: GenerationMode) => {
    setMode(next);
    try {
      sessionStorage.setItem(preferenceKey, next);
    } catch {
      // This window can still switch modes when browser storage is unavailable.
    }
  }, []);
  return [mode, selectMode] as const;
}
