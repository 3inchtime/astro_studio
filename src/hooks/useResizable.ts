import { useState, useCallback, useRef, useEffect } from "react";

const STORAGE_KEY = "astro-layout-widths-v2";
interface PanelConfig { min: number; default: number; max: number | null }

export function useResizable(panels: PanelConfig[]) {
  const clamp = useCallback((index: number, value: number) => {
    const panel = panels[index];
    return Math.max(panel.min, Math.min(panel.max ?? Infinity, Number.isFinite(value) ? value : panel.default));
  }, [panels]);
  const [widths, setWidths] = useState<number[]>(() => {
    try {
      const parsed: unknown = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? localStorage.getItem("astro-layout-widths") ?? "null");
      if (Array.isArray(parsed) && parsed.length === panels.length) {
        return parsed.map((value, index) => clamp(index, typeof value === "number" ? value : NaN));
      }
    } catch { /* Unavailable storage should never prevent opening the workspace. */ }
    return panels.map((panel) => panel.default);
  });
  const stopDrag = useRef<(() => void) | null>(null);
  useEffect(() => () => stopDrag.current?.(), []);
  useEffect(() => {
    try { localStorage.setItem(STORAGE_KEY, JSON.stringify(widths)); } catch { /* Session-only sizing. */ }
  }, [widths]);
  const setPanelWidth = useCallback((index: number, width: number) => {
    setWidths((previous) => previous.map((value, i) => i === index ? clamp(index, width) : value));
  }, [clamp]);
  const onHandleDown = useCallback((index: number) => (event: React.MouseEvent) => {
    event.preventDefault();
    stopDrag.current?.();
    const startX = event.clientX;
    const startWidth = widths[index];
    const oldSelection = document.body.style.userSelect;
    document.body.style.userSelect = "none";
    const onMove = (next: MouseEvent) => setPanelWidth(index, startWidth + next.clientX - startX);
    const onUp = () => {
      document.body.style.userSelect = oldSelection;
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      stopDrag.current = null;
    };
    stopDrag.current = onUp;
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  }, [widths, setPanelWidth]);
  return { widths, onHandleDown, setPanelWidth };
}
