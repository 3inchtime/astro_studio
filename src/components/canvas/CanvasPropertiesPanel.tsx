import { useTranslation } from "react-i18next";
import CanvasTextControls from "./CanvasTextControls";
import type { CanvasObject } from "../../types";
import type { CanvasObjectStyle } from "../../lib/canvas/editor";
export default function CanvasPropertiesPanel({
  objects,
  onChange,
}: {
  objects: CanvasObject[];
  onChange: (style: CanvasObjectStyle) => void;
}) {
  const { t } = useTranslation();
  if (!objects.length) return null;
  const first = objects[0];
  const all = (key: string) => objects.every((o) => key in o);
  return (
    <section className="bg-surface p-4">
      <h3 className="mb-3 text-sm font-semibold">
        {t("canvas.propertiesTitle")}
      </h3>
      <div className="flex flex-col gap-3 text-xs">
        {objects.every((object) => object.type === "text") ? (
          <CanvasTextControls objects={objects} onChange={onChange} />
        ) : null}
        {all("color") && "color" in first ? (
          <label className="flex min-h-9 items-center justify-between gap-3 text-muted">
            {t("canvas.strokeColor")}
            <input
              aria-label={t("canvas.strokeColor")}
              className="h-9 w-12 rounded-lg border border-border-subtle bg-surface p-1"
              type="color"
              value={first.color}
              onChange={(e) => onChange({ color: e.target.value })}
            />
          </label>
        ) : null}
        {all("fill") && "fill" in first ? (
          <label className="flex min-h-9 items-center justify-between gap-3 text-muted">
            {t("canvas.fillColor")}
            <input
              aria-label={t("canvas.fillColor")}
              className="h-9 w-12 rounded-lg border border-border-subtle bg-surface p-1"
              type="color"
              value={first.fill === "transparent" ? "#ffffff" : first.fill}
              onChange={(e) => onChange({ fill: e.target.value })}
            />
          </label>
        ) : null}
        {all("size") && "size" in first ? (
          <label className="flex min-h-9 items-center justify-between gap-3 text-muted">
            {t("canvas.strokeWidth")}
            <input
              aria-label={t("canvas.strokeWidth")}
              className="studio-input focus-ring h-9 w-20 rounded-lg px-2 text-sm text-foreground"
              type="number"
              min="1"
              value={first.size}
              onChange={(e) => onChange({ size: Number(e.target.value) })}
            />
          </label>
        ) : null}
        {all("fontSize") && "fontSize" in first ? (
          <label className="flex min-h-9 items-center justify-between gap-3 text-muted">
            {t("canvas.fontSize")}
            <input
              aria-label={t("canvas.fontSize")}
              className="studio-input focus-ring h-9 w-20 rounded-lg px-2 text-sm text-foreground"
              type="number"
              min="1"
              value={first.fontSize}
              onChange={(e) => onChange({ fontSize: Number(e.target.value) })}
            />
          </label>
        ) : null}
        <label className="flex min-h-9 items-center justify-between gap-3 text-muted">
          {t("canvas.opacity")}
          <input
            aria-label={t("canvas.opacity")}
            type="range"
            min="0"
            max="1"
            step="0.05"
            value={first.opacity}
            onChange={(e) => onChange({ opacity: Number(e.target.value) })}
          />
        </label>
      </div>
    </section>
  );
}
