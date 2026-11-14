import { useEffect, useRef } from "react";
import { RotateCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { CanvasTextObject, CanvasViewport } from "../../types";
import { getCanvasObjectBounds } from "../../lib/canvas/bounds";

export default function CanvasTextRotationHandle({
  object,
  viewport,
  identity,
  onPreview,
  onCommit,
}: {
  object: CanvasTextObject;
  viewport: CanvasViewport;
  identity: unknown;
  onPreview: (angle: number | null) => void;
  onCommit: (angle: number) => void;
}) {
  const { t } = useTranslation();
  const root = useRef<HTMLDivElement>(null);
  const callbacks = useRef({ onPreview, onCommit });
  callbacks.current = { onPreview, onCommit };
  const gesture = useRef<{
    cx: number;
    cy: number;
    start: number;
    rotation: number;
    next: number;
  } | null>(null);
  const bounds = getCanvasObjectBounds(object)!;
  const center = {
    x: (bounds.x + bounds.width / 2) * viewport.scale + viewport.x,
    y: (bounds.y + bounds.height / 2) * viewport.scale + viewport.y,
  };
  useEffect(() => {
    const cancel = () => {
      gesture.current = null;
      callbacks.current.onPreview(null);
    };
    function move(event: MouseEvent | TouchEvent) {
      const g = gesture.current;
      if (!g) return;
      const point = "touches" in event ? event.touches[0] : event;
      if (!point) return;
      if (event.cancelable) event.preventDefault();
      const angle =
        (Math.atan2(point.clientY - g.cy, point.clientX - g.cx) * 180) /
        Math.PI;
      const rotation = g.rotation + angle - g.start;
      g.next = ((rotation % 360) + 360) % 360;
      if (event.shiftKey) g.next = (Math.round(g.next / 15) * 15) % 360;
      callbacks.current.onPreview(g.next);
    }
    function end() {
      const g = gesture.current;
      gesture.current = null;
      if (g && g.next !== g.rotation) callbacks.current.onCommit(g.next);
      callbacks.current.onPreview(null);
    }
    function key(event: KeyboardEvent) {
      if (event.key === "Escape") cancel();
    }
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", end);
    window.addEventListener("touchmove", move, { passive: false });
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
  function start(event: React.MouseEvent | React.TouchEvent) {
    if ("button" in event && event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    const p = "touches" in event ? event.touches[0] : event;
    if (!p) return;
    const rect = root.current!.getBoundingClientRect();
    const cx = center.x + rect.left,
      cy = center.y + rect.top;
    gesture.current = {
      cx,
      cy,
      start: (Math.atan2(p.clientY - cy, p.clientX - cx) * 180) / Math.PI,
      rotation: object.rotation,
      next: object.rotation,
    };
  }
  return (
    <div ref={root} className="pointer-events-none absolute inset-0 z-20">
      <button
        type="button"
        aria-label={t("canvas.rotateText")}
        title={t("canvas.rotateTextHint")}
        className="focus-ring pointer-events-auto absolute flex h-7 w-7 touch-none items-center justify-center rounded-full border border-primary bg-surface text-primary"
        style={{
          left: center.x,
          top: Math.max(16, bounds.y * viewport.scale + viewport.y - 28),
          transform: "translate(-50%, -50%)",
          cursor: "grab",
        }}
        onMouseDown={start}
        onTouchStart={start}
        onKeyDown={(event) => {
          if (!["ArrowLeft", "ArrowRight"].includes(event.key)) return;
          event.preventDefault();
          event.stopPropagation();
          onCommit(
            object.rotation +
              (event.key === "ArrowRight" ? 1 : -1) * (event.shiftKey ? 15 : 1),
          );
        }}
      >
        <RotateCw size={14} />
      </button>
    </div>
  );
}
