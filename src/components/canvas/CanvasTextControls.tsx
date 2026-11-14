import { AlignCenter, AlignLeft, AlignRight, Bold, Italic } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { CanvasTextObject } from "../../types";
import type { CanvasObjectStyle } from "../../lib/canvas/editor";
import { CANVAS_FONTS, normalizeTextStyle } from "../../lib/canvas/text";

export default function CanvasTextControls({
  objects,
  onChange,
}: {
  objects: CanvasTextObject[];
  onChange: (style: CanvasObjectStyle) => void;
}) {
  const { t } = useTranslation();
  const styles = objects.map((object) => ({
    ...object,
    ...normalizeTextStyle(object),
  }));
  const first = styles[0];
  if (!first) return null;
  const inputClass =
    "studio-input focus-ring h-9 min-w-0 rounded-lg px-2 text-sm text-foreground";
  const buttonClass =
    "focus-ring flex h-9 flex-1 items-center justify-center rounded-lg text-muted hover:bg-subtle hover:text-foreground aria-pressed:bg-primary/10 aria-pressed:text-primary";
  return (
    <div className="flex flex-col gap-3">
      <label className="flex flex-col gap-2 text-xs text-muted">
        {t("canvas.fontFamily")}
        <select
          aria-label={t("canvas.fontFamily")}
          className={inputClass}
          value={
            styles.every((s) => s.fontFamily === first.fontFamily)
              ? first.fontFamily
              : ""
          }
          onChange={(event) => onChange({ fontFamily: event.target.value })}
        >
          {!styles.every((s) => s.fontFamily === first.fontFamily) && (
            <option value="" disabled>
              {t("canvas.mixedValues")}
            </option>
          )}
          {CANVAS_FONTS.map((font) => (
            <option
              key={font.value}
              value={font.value}
              style={{ fontFamily: font.value }}
            >
              {t(`canvas.${font.label}`)}
            </option>
          ))}
        </select>
      </label>
      <div className="flex gap-2">
        <div
          role="group"
          aria-label={t("canvas.fontStyle")}
          className="flex flex-1 gap-1 rounded-lg bg-subtle p-1"
        >
          <button
            type="button"
            className={buttonClass}
            aria-label={t("canvas.bold")}
            title={t("canvas.bold")}
            aria-pressed={styles.every((s) => s.fontWeight === "bold")}
            onClick={() =>
              onChange({
                fontWeight: styles.every((s) => s.fontWeight === "bold")
                  ? "normal"
                  : "bold",
              })
            }
          >
            <Bold size={16} />
          </button>
          <button
            type="button"
            className={buttonClass}
            aria-label={t("canvas.italic")}
            title={t("canvas.italic")}
            aria-pressed={styles.every((s) => s.fontStyle === "italic")}
            onClick={() =>
              onChange({
                fontStyle: styles.every((s) => s.fontStyle === "italic")
                  ? "normal"
                  : "italic",
              })
            }
          >
            <Italic size={16} />
          </button>
        </div>
        <div
          role="group"
          aria-label={t("canvas.textAlign")}
          className="flex flex-[1.5] gap-1 rounded-lg bg-subtle p-1"
        >
          {(
            [
              ["left", AlignLeft, "textAlignLeft"],
              ["center", AlignCenter, "textAlignCenter"],
              ["right", AlignRight, "textAlignRight"],
            ] as const
          ).map(([align, Icon, label]) => (
            <button
              key={align}
              type="button"
              className={buttonClass}
              aria-label={t(`canvas.${label}`)}
              title={t(`canvas.${label}`)}
              aria-pressed={styles.every((s) => s.textAlign === align)}
              onClick={() => onChange({ textAlign: align })}
            >
              <Icon size={16} />
            </button>
          ))}
        </div>
      </div>
      <div className="grid grid-cols-2 gap-3">
        {(
          [
            ["letterSpacing", -20, 200, 0.5, "px"],
            ["lineHeight", 0.5, 5, 0.1, "×"],
            ["rotation", -360, 360, 1, "°"],
          ] as const
        ).map(([key, min, max, step, unit]) => (
          <label
            key={key}
            className="flex min-w-0 flex-col gap-2 text-xs text-muted"
          >
            {t(`canvas.${key}`)}
            <span className="relative">
              <TextNumberInput
                label={t(`canvas.${key}`)}
                min={min}
                max={max}
                step={step}
                value={
                  styles.every((s) => s[key] === first[key])
                    ? Math.round(first[key] * 100) / 100
                    : ""
                }
                placeholder={t("canvas.mixedValues")}
                className={`${inputClass} w-full pr-6 tabular-nums`}
                onCommit={(value) => onChange({ [key]: value })}
              />
              <span
                aria-hidden="true"
                className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2"
              >
                {unit}
              </span>
            </span>
          </label>
        ))}
      </div>
    </div>
  );
}

function TextNumberInput({
  value,
  min,
  max,
  step,
  label,
  placeholder,
  className,
  onCommit,
}: {
  value: number | "";
  min: number;
  max: number;
  step: number;
  label: string;
  placeholder: string;
  className: string;
  onCommit: (value: number) => void;
}) {
  const [draft, setDraft] = useState(String(value));
  const focused = useRef(false);
  const cancelled = useRef(false);
  useEffect(() => {
    if (!focused.current) setDraft(String(value));
  }, [value]);
  return (
    <input
      aria-label={label}
      type="number"
      min={min}
      max={max}
      step={step}
      value={draft}
      placeholder={placeholder}
      className={className}
      onFocus={() => {
        focused.current = true;
        cancelled.current = false;
      }}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={() => {
        focused.current = false;
        const parsed = Number(draft);
        if (parsed === value && draft.trim()) return;
        if (!cancelled.current && draft.trim() && Number.isFinite(parsed)) {
          const next = Math.max(min, Math.min(max, parsed));
          setDraft(String(next));
          if (next !== value) onCommit(next);
        } else setDraft(String(value));
        cancelled.current = false;
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter") {
          event.preventDefault();
          event.currentTarget.blur();
        }
        if (event.key === "Escape") {
          event.stopPropagation();
          cancelled.current = true;
          event.currentTarget.blur();
        }
      }}
    />
  );
}
