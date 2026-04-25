import { useState, useCallback, useRef } from "react";

const STORAGE_KEY = "astro-layout-widths";

interface PanelConfig {
  min: number;
  default: number;
  max: number | null;
}

export function useResizable(panels: PanelConfig[]) {
  const defaults = panels.map((p) => p.default);
  const [widths, setWidths] = useState<number[]>(() => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        const parsed = JSON.parse(stored) as number[];
        if (parsed.length === panels.length) return parsed;
      }
    } catch {}
    return defaults;
  });

  const draggingIndex = useRef<number | null>(null);
  const startX = useRef(0);
  const startWidths = useRef<number[]>([]);

  const onMouseMove = useCallback(
    (e: MouseEvent) => {
      if (draggingIndex.current === null) return;
      const idx = draggingIndex.current;
      const dx = e.clientX - startX.current;
      const panel = panels[idx];
      const nextPanel = panels[idx + 1];

      setWidths((prev) => {
        const next = [...prev];
        const newWidth = Math.max(panel.min, Math.min(panel.max ?? Infinity, startWidths.current[idx] + dx));
        const remaining = prev[idx + 1] - (newWidth - startWidths.current[idx]);

        if (nextPanel && remaining < nextPanel.min) return prev;

        next[idx] = newWidth;
        if (nextPanel) {
          next[idx + 1] = Math.max(nextPanel.min, startWidths.current[idx + 1] - (newWidth - startWidths.current[idx]));
        }
        return next;
      });
    },
    [panels],
  );

  const onMouseUp = useCallback(() => {
    draggingIndex.current = null;
    document.removeEventListener("mousemove", onMouseMove);
    document.removeEventListener("mouseup", onMouseUp);
    setWidths((prev) => {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(prev));
      return prev;
    });
  }, [onMouseMove]);

  const onHandleDown = useCallback(
    (index: number) => (e: React.MouseEvent) => {
      e.preventDefault();
      draggingIndex.current = index;
      startX.current = e.clientX;
      startWidths.current = [...widths];
      document.addEventListener("mousemove", onMouseMove);
      document.addEventListener("mouseup", onMouseUp);
    },
    [widths, onMouseMove, onMouseUp],
  );

  return { widths, onHandleDown };
}
