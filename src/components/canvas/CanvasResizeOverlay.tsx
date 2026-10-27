import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { CanvasRect } from "../../lib/canvas/bounds";
import type { CanvasViewport } from "../../types";

const HANDLES = [
  "top-left",
  "top-center",
  "top-right",
  "middle-left",
  "middle-right",
  "bottom-left",
  "bottom-center",
  "bottom-right",
] as const;

/** DOM handles keep editing controls crisp at any canvas zoom. */
export default function CanvasResizeOverlay({
  bounds,
  viewport,
  kind,
  onPreview,
  onCommit,
  identity,
  keepRatio = false,
}: {
  bounds: CanvasRect;
  viewport: CanvasViewport;
  kind: "selection" | "frame";
  onPreview: (bounds: CanvasRect | null) => void;
  onCommit: (bounds: CanvasRect) => void;
  identity: unknown;
  keepRatio?: boolean;
}) {
  const { t } = useTranslation();
  const gesture = useRef<{
    x: number;
    y: number;
    bounds: CanvasRect;
    handle: string;
    next: CanvasRect;
    scale: number;
    keepRatio: boolean;
  } | null>(null);
  const callbacks = useRef({ onPreview, onCommit });
  callbacks.current = { onPreview, onCommit };
  useEffect(() => {
    function cancel() {
      gesture.current = null;
      callbacks.current.onPreview(null);
    }
    function move(event: MouseEvent | TouchEvent) {
      const g = gesture.current;
      if (!g) return;
      const point = "touches" in event ? event.touches[0] : event;
      if (!point) return;
      const dx = (point.clientX - g.x) / g.scale,
        dy = (point.clientY - g.y) / g.scale;
      let { x, y, width, height } = g.bounds;
      if (g.handle === "move") {
        x += dx;
        y += dy;
      } else {
        if (g.handle.includes("left")) {
          x += dx;
          width -= dx;
        }
        if (g.handle.includes("right")) width += dx;
        if (g.handle.includes("top")) {
          y += dy;
          height -= dy;
        }
        if (g.handle.includes("bottom")) height += dy;
        if (g.keepRatio || ("shiftKey" in event && event.shiftKey)) {
          const ratio = g.bounds.width / Math.max(1, g.bounds.height);
          if (g.handle.includes("center")) width = height * ratio;
          else height = width / ratio;
          if (g.handle.includes("left"))
            x = g.bounds.x + g.bounds.width - width;
          if (g.handle.includes("top"))
            y = g.bounds.y + g.bounds.height - height;
        }
        if (width < 8 || height < 8) return;
      }
      g.next = { x, y, width, height };
      callbacks.current.onPreview(g.next);
    }
    function end() {
      const g = gesture.current;
      gesture.current = null;
      if (!g) return;
      if (
        g &&
        Object.keys(g.bounds).some(
          (k) =>
            g.bounds[k as keyof CanvasRect] !== g.next[k as keyof CanvasRect],
        )
      )
        callbacks.current.onCommit(g.next);
      callbacks.current.onPreview(null);
    }
    function key(event: KeyboardEvent) {
      if (event.key === "Escape") cancel();
    }
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", end);
    window.addEventListener("touchmove", move);
    window.addEventListener("touchend", end);
    window.addEventListener("touchcancel", cancel);
    window.addEventListener("blur", cancel);
    window.addEventListener("keydown", key);
    return () => {
      cancel();
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", end);
      window.removeEventListener("touchmove", move);
      window.removeEventListener("touchend", end);
      window.removeEventListener("touchcancel", cancel);
      window.removeEventListener("blur", cancel);
      window.removeEventListener("keydown", key);
    };
  }, []);
  useEffect(() => {
    gesture.current = null;
    callbacks.current.onPreview(null);
  }, [identity]);
  function start(event: React.MouseEvent | React.TouchEvent, handle: string) {
    if ("button" in event && event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    const p = "touches" in event ? event.touches[0] : event;
    gesture.current = {
      x: p.clientX,
      y: p.clientY,
      bounds,
      next: bounds,
      handle,
      scale: viewport.scale,
      keepRatio,
    };
  }
  return (
    <div
      className="pointer-events-none absolute z-20 border border-primary"
      style={{
        left: bounds.x * viewport.scale + viewport.x,
        top: bounds.y * viewport.scale + viewport.y,
        width: bounds.width * viewport.scale,
        height: bounds.height * viewport.scale,
      }}
    >
      {kind === "frame" && (
        <button
          type="button"
          className="pointer-events-auto absolute -top-7 left-0 cursor-move rounded bg-primary px-2 py-1 text-[10px] text-white"
          onMouseDown={(e) => start(e, "move")}
          onTouchStart={(e) => start(e, "move")}
        >
          {t("canvas.moveFrame")}
        </button>
      )}
      {HANDLES.map((handle) => (
        <button
          key={handle}
          type="button"
          aria-label={`${t(kind === "frame" ? "canvas.resizeFrame" : "canvas.resizeSelection")} ${handle}`}
          className="pointer-events-auto absolute h-3 w-3 rounded-[2px] border border-primary bg-white shadow-sm"
          style={{
            left: handle.includes("left")
              ? 0
              : handle.includes("right")
                ? "100%"
                : "50%",
            top: handle.includes("top")
              ? 0
              : handle.includes("bottom")
                ? "100%"
                : "50%",
            transform: "translate(-50%, -50%)",
            cursor:
              handle.includes("left") || handle.includes("right")
                ? handle.includes("top") || handle.includes("bottom")
                  ? handle === "top-left" || handle === "bottom-right"
                    ? "nwse-resize"
                    : "nesw-resize"
                  : "ew-resize"
                : "ns-resize",
          }}
          onMouseDown={(e) => start(e, handle)}
          onTouchStart={(e) => start(e, handle)}
        />
      ))}
    </div>
  );
}
