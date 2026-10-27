import { useTranslation } from "react-i18next";
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
    <section className="rounded-[16px] border border-border-subtle bg-surface/92 p-4 shadow-card">
      <h3 className="mb-3 text-[13px] font-semibold">
        {t("canvas.propertiesTitle")}
      </h3>
      <div className="flex flex-col gap-3 text-[12px]">
        {all("color") && "color" in first ? (
          <label>
            {t("canvas.strokeColor")}
            <input
              aria-label={t("canvas.strokeColor")}
              type="color"
              value={first.color}
              onChange={(e) => onChange({ color: e.target.value })}
            />
          </label>
        ) : null}
        {all("fill") && "fill" in first ? (
          <label>
            {t("canvas.fillColor")}
            <input
              aria-label={t("canvas.fillColor")}
              type="color"
              value={first.fill === "transparent" ? "#ffffff" : first.fill}
              onChange={(e) => onChange({ fill: e.target.value })}
            />
          </label>
        ) : null}
        {all("size") && "size" in first ? (
          <label>
            {t("canvas.strokeWidth")}
            <input
              aria-label={t("canvas.strokeWidth")}
              type="number"
              min="1"
              value={first.size}
              onChange={(e) => onChange({ size: Number(e.target.value) })}
            />
          </label>
        ) : null}
        {all("fontSize") && "fontSize" in first ? (
          <label>
            {t("canvas.fontSize")}
            <input
              aria-label={t("canvas.fontSize")}
              type="number"
              min="1"
              value={first.fontSize}
              onChange={(e) => onChange({ fontSize: Number(e.target.value) })}
            />
          </label>
        ) : null}
        <label>
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
