import { useState } from "react";
import { useTranslation } from "react-i18next";
import { pickSourceImages } from "../../lib/api";
import type {
  ImageAdvancedOptions,
  ImageOutputFormat,
  ImageSize,
} from "../../types";

interface Props {
  embedded?: boolean;
  value: ImageAdvancedOptions;
  onChange: (value: ImageAdvancedOptions) => void;
  format: ImageOutputFormat;
  editing: boolean;
  size?: ImageSize;
  onSizeChange?: (value: ImageSize) => void;
  compression?: number;
  onCompressionChange?: (value: number) => void;
}

export default function ImageAdvancedControls({
  embedded = false,
  value,
  onChange,
  format,
  editing,
  size,
  onSizeChange,
  compression = 100,
  onCompressionChange,
}: Props) {
  const { t } = useTranslation();
  const [error, setError] = useState<string | null>(null);
  const update = (patch: Partial<ImageAdvancedOptions>) =>
    onChange({ ...value, ...patch });
  const raster = format !== "gif" && format !== "svg";
  const field =
    "studio-input focus-ring mt-1 w-full rounded-lg px-3 py-2 text-sm";
  const label = "text-xs text-muted";
  const chooseOverlay = async (kind: "markupPath" | "maskPath") => {
    try {
      const paths = await pickSourceImages();
      if (paths[0]) {
        update({
          markupPath: undefined,
          maskPath: undefined,
          [kind]: paths[0],
        });
        setError(null);
      }
    } catch (cause) {
      setError(String(cause));
    }
  };
  const Container = embedded ? "div" : "details";
  return (
    <Container className="@container mt-3 border-t border-border-subtle py-3">
      {!embedded && (
        <summary className="focus-ring cursor-pointer rounded-lg text-xs font-medium text-foreground">
          {t("generate.advanced.title")}
        </summary>
      )}
      <div className="grid grid-cols-1 gap-4 @min-[400px]:grid-cols-2 @min-[640px]:grid-cols-3">
        <label className={label}>
          {t("generate.advanced.complexity")}
          <select
            className={field}
            value={value.complexity ?? ""}
            onChange={(e) =>
              update({
                complexity: (e.target.value ||
                  undefined) as ImageAdvancedOptions["complexity"],
              })
            }
          >
            <option value="">{t("generate.advanced.default")}</option>
            {["low", "medium", "high"].map((level) => (
              <option key={level} value={level}>
                {t(`generate.quality.${level}`)}
              </option>
            ))}
          </select>
        </label>
        {onSizeChange && (
          <label className={label}>
            {t("generate.advanced.customSize")}
            <input
              className={field}
              value={size ?? "auto"}
              placeholder="1536x864"
              onChange={(e) => onSizeChange(e.target.value as ImageSize)}
            />
          </label>
        )}
        {["jpeg", "webp"].includes(format) && onCompressionChange && (
          <label className={label}>
            {t("generate.advanced.compression")}
            <input
              type="number"
              className={field}
              min={0}
              max={100}
              value={compression}
              onChange={(e) => onCompressionChange(Number(e.target.value))}
            />
          </label>
        )}
        {format === "gif" && (
          <>
            <label className={label}>
              {t("generate.advanced.frameCount")}
              <input
                type="number"
                className={field}
                min={2}
                max={16}
                value={value.frameCount ?? 9}
                onChange={(e) => update({ frameCount: Number(e.target.value) })}
              />
            </label>
            <label className={label}>
              {t("generate.advanced.frameDuration")}
              <input
                type="number"
                className={field}
                min={20}
                max={4294967295}
                step={10}
                value={value.frameDurationMs ?? 200}
                onChange={(e) =>
                  update({ frameDurationMs: Number(e.target.value) })
                }
              />
            </label>
          </>
        )}
        {format === "svg" && (
          <>
            <label className={label}>
              {t("generate.advanced.svgModel")}
              <input
                className={field}
                value={value.svg?.model ?? ""}
                placeholder={t("generate.advanced.default")}
                onChange={(e) =>
                  update({
                    svg: e.target.value
                      ? { ...value.svg, model: e.target.value }
                      : undefined,
                  })
                }
              />
            </label>
            <label className={label}>
              {t("generate.advanced.svgReasoning")}
              <select
                className={field}
                disabled={!value.svg?.model}
                value={value.svg?.reasoning?.effort ?? ""}
                onChange={(e) =>
                  value.svg &&
                  update({
                    svg: {
                      ...value.svg,
                      reasoning: e.target.value
                        ? {
                            effort: e.target.value as NonNullable<
                              NonNullable<
                                ImageAdvancedOptions["svg"]
                              >["reasoning"]
                            >["effort"],
                          }
                        : undefined,
                    },
                  })
                }
              >
                <option value="">{t("generate.advanced.default")}</option>
                {[
                  "none",
                  "minimal",
                  "low",
                  "medium",
                  "high",
                  "xhigh",
                  "max",
                ].map((effort) => (
                  <option key={effort}>{effort}</option>
                ))}
              </select>
            </label>
          </>
        )}
        {raster && (
          <>
            <label className={`${label} flex items-center gap-2`}>
              <input
                type="checkbox"
                checked={value.stream ?? false}
                onChange={(e) => update({ stream: e.target.checked })}
              />
              {t("generate.advanced.stream")}
            </label>
            {value.stream && (
              <label className={label}>
                {t("generate.advanced.partialImages")}
                <select
                  className={field}
                  value={value.partialImages ?? 0}
                  onChange={(e) =>
                    update({ partialImages: Number(e.target.value) })
                  }
                >
                  {[0, 1, 2, 3].map((n) => (
                    <option key={n}>{n}</option>
                  ))}
                </select>
              </label>
            )}
          </>
        )}
      </div>
      {editing && (
        <div className="mt-3 space-y-2">
          <p className="text-xs leading-relaxed text-muted">
            {t("generate.advanced.overlayHint")}
          </p>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              className="studio-control rounded-lg px-3 py-1.5 text-xs"
              onClick={() => void chooseOverlay("markupPath")}
            >
              {t("generate.advanced.markup")}
            </button>
            {format !== "gif" && (
              <button
                type="button"
                className="studio-control rounded-lg px-3 py-1.5 text-xs"
                onClick={() => void chooseOverlay("maskPath")}
              >
                {t("generate.advanced.mask")}
              </button>
            )}
            {(value.markupPath || value.maskPath) && (
              <button
                type="button"
                className="studio-control rounded-lg px-3 py-1.5 text-xs"
                onClick={() =>
                  update({ markupPath: undefined, maskPath: undefined })
                }
              >
                {t("generate.advanced.clearOverlay")}
              </button>
            )}
          </div>
          {(value.markupPath || value.maskPath) && (
            <p className="break-all text-xs text-muted">
              {value.markupPath || value.maskPath}
            </p>
          )}
        </div>
      )}
      {format === "gif" && (
        <p className="mt-2 text-xs leading-relaxed text-muted">
          {t("generate.advanced.gifHint")}
        </p>
      )}
      {format === "svg" && (
        <p className="mt-2 text-xs leading-relaxed text-muted">
          {t("generate.advanced.svgHint")}
        </p>
      )}
      {error && (
        <p role="alert" className="mt-2 text-xs text-error">
          {error}
        </p>
      )}
    </Container>
  );
}
