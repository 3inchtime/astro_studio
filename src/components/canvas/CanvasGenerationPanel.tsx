import { Loader2, PanelRight, Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toAssetUrl } from "../../lib/api";
import ImageAdvancedControls from "../generate/ImageAdvancedControls";
import {
  getImageModelCatalogEntry,
  isGptImage26,
} from "../../lib/modelCatalog";
import type { GenerationParams } from "../../types";
import type { CanvasFrame, ImageModel } from "../../types";

interface CanvasGenerationPanelProps {
  partialPreview?: string;
  parameters?: GenerationParams;
  onParametersChange?: (value: GenerationParams) => void;
  prompt: string;
  imageModel: ImageModel;
  frame: CanvasFrame | null;
  disabled: boolean;
  isGenerating: boolean;
  environmentHint?: string | null;
  error?: string | null;
  results?: string[];
  onRetry?: () => void;
  onFrameAspectChange?: (aspect: string) => void;
  onFrameFromSelection?: () => void;
  onPromptChange: (value: string) => void;
  onGenerate: () => void;
}

export default function CanvasGenerationPanel({
  parameters,
  onParametersChange,
  partialPreview,
  prompt,
  imageModel,
  frame,
  disabled,
  isGenerating,
  environmentHint,
  onPromptChange,
  onGenerate,
  error,
  results,
  onRetry,
  onFrameAspectChange,
  onFrameFromSelection,
}: CanvasGenerationPanelProps) {
  const { t } = useTranslation();

  return (
    <section className="bg-surface">
      <div className="border-b border-border-subtle px-3 py-3">
        <div className="flex items-center gap-2 text-[14px] font-semibold text-foreground">
          <PanelRight size={14} strokeWidth={1.8} />
          <span>{t("canvas.generationTitle")}</span>
        </div>
      </div>

      <div className="flex flex-col gap-3 p-3">
        <div className="space-y-2">
          <label className="block text-[12px] font-medium text-foreground/80">
            {t("generate.modelLabel")}
          </label>
          <div className="rounded-[12px] border border-border-subtle bg-surface px-3 py-2 text-[14px] text-foreground">
            {imageModel}
          </div>
        </div>

        <div className="grid grid-cols-2 gap-2">
          <div className="rounded-[12px] border border-border-subtle bg-surface/80 px-3 py-2">
            <div className="text-[12px] uppercase tracking-[0.04em] text-muted">
              {t("canvas.frameAspect")}
            </div>
            <div className="mt-1 text-[14px] font-medium text-foreground">
              {onFrameAspectChange ? (
                <select
                  aria-label={t("canvas.framePreset")}
                  value={
                    frame?.aspect === "free"
                      ? "custom"
                      : (frame?.aspect ?? "1:1")
                  }
                  onChange={(e) => onFrameAspectChange(e.target.value)}
                >
                  {[
                    "1:1",
                    "4:3",
                    "3:4",
                    "16:9",
                    "9:16",
                    "3:2",
                    "2:3",
                    "custom",
                  ].map((aspect) => (
                    <option key={aspect} value={aspect}>
                      {aspect === "custom" ? t("canvas.customAspect") : aspect}
                    </option>
                  ))}
                </select>
              ) : (
                (frame?.aspect ?? "--")
              )}
            </div>
          </div>
          <div className="rounded-[12px] border border-border-subtle bg-surface/80 px-3 py-2">
            <div className="text-[12px] uppercase tracking-[0.04em] text-muted">
              {t("canvas.layersTitle")}
            </div>
            <div className="mt-1 text-[14px] font-medium text-foreground">
              {frame
                ? `${Math.round(frame.width)} x ${Math.round(frame.height)}`
                : "--"}
            </div>
          </div>
        </div>

        {onFrameFromSelection ? (
          <button
            type="button"
            className="studio-control-secondary rounded-lg p-2 text-xs"
            onClick={onFrameFromSelection}
          >
            {t("canvas.frameFromSelection")}
          </button>
        ) : null}
        {isGptImage26(imageModel) && parameters && onParametersChange && (
          <div className="space-y-2">
            <div className="grid grid-cols-2 gap-2">
              <label className="text-xs text-muted">
                {t("generate.formatLabel")}
                <select
                  className="studio-input mt-1 w-full rounded-lg p-2"
                  value={parameters.outputFormat ?? "png"}
                  onChange={(e) =>
                    onParametersChange({
                      ...parameters,
                      outputFormat: e.target
                        .value as GenerationParams["outputFormat"],
                      ...(e.target.value === "gif"
                        ? {
                            size: "auto",
                            background: "auto",
                            advancedOptions: {
                              ...parameters.advancedOptions,
                              maskPath: undefined,
                            },
                          }
                        : {}),
                    })
                  }
                >
                  {["png", "jpeg", "webp", "gif"].map((format) => (
                    <option key={format} value={format}>
                      {format.toUpperCase()}
                    </option>
                  ))}
                </select>
              </label>
              <label className="text-xs text-muted">
                {t("generate.qualityLabel")}
                <select
                  className="studio-input mt-1 w-full rounded-lg p-2"
                  value={parameters.quality ?? "auto"}
                  onChange={(e) =>
                    onParametersChange({
                      ...parameters,
                      quality: e.target.value as GenerationParams["quality"],
                    })
                  }
                >
                  {getImageModelCatalogEntry(
                    imageModel,
                  ).parameterCapabilities.qualities.map((quality) => (
                    <option key={quality} value={quality}>
                      {t(`generate.quality.${quality}`)}
                    </option>
                  ))}
                </select>
              </label>
              <label className="text-xs text-muted">
                {t("generate.countLabel")}
                <select
                  className="studio-input mt-1 w-full rounded-lg p-2"
                  value={parameters.imageCount ?? 1}
                  onChange={(e) =>
                    onParametersChange({
                      ...parameters,
                      imageCount: Number(e.target.value),
                    })
                  }
                >
                  {Array.from({ length: 10 }, (_, index) => index + 1).map(
                    (count) => (
                      <option key={count} value={count}>
                        {count}
                      </option>
                    ),
                  )}
                </select>
              </label>
            </div>
            <ImageAdvancedControls
              value={parameters.advancedOptions ?? {}}
              onChange={(advancedOptions) =>
                onParametersChange({ ...parameters, advancedOptions })
              }
              format={parameters.outputFormat ?? "png"}
              editing
              size={parameters.size ?? "auto"}
              onSizeChange={(size) =>
                onParametersChange({ ...parameters, size })
              }
              compression={parameters.outputCompression}
              onCompressionChange={(outputCompression) =>
                onParametersChange({ ...parameters, outputCompression })
              }
            />
            {parameters.outputFormat === "gif" && (
              <p className="text-xs text-muted">
                {t("generate.advanced.canvasStillHint")}
              </p>
            )}
          </div>
        )}
        <label
          htmlFor="canvas-generation-prompt"
          className="text-[12px] font-medium text-foreground/80"
        >
          {t("canvas.promptLabel")}
        </label>
        <textarea
          id="canvas-generation-prompt"
          value={prompt}
          onChange={(event) => onPromptChange(event.target.value)}
          placeholder={t("canvas.promptPlaceholder")}
          className="studio-input -mt-2 min-h-[144px] w-full resize-y rounded-xl px-4 py-3 text-[14px] leading-[1.65]"
        />

        <button
          type="button"
          onClick={onGenerate}
          disabled={disabled}
          aria-busy={isGenerating}
          className="studio-control-primary focus-ring inline-flex items-center justify-center gap-2 h-9 rounded-lg px-4 text-[14px] font-semibold disabled:cursor-not-allowed disabled:opacity-50"
        >
          {isGenerating ? (
            <Loader2
              aria-hidden="true"
              className="animate-spin"
              size={15}
              strokeWidth={1.8}
            />
          ) : (
            <Sparkles size={15} strokeWidth={1.8} />
          )}
          <span
            aria-label={isGenerating ? t("canvas.generating") : undefined}
            role={isGenerating ? "status" : undefined}
          >
            {isGenerating ? t("canvas.generating") : t("canvas.generate")}
          </span>
        </button>

        {error ? (
          <div role="alert" className="text-[12px] text-red-600">
            <p>
              {t("canvas.generationError")}: {error}
            </p>
            {onRetry ? (
              <button
                type="button"
                onClick={onRetry}
                disabled={isGenerating}
                className="mt-2 rounded-lg border border-border-subtle px-3 py-2"
              >
                {t("canvas.generationRetry")}
              </button>
            ) : null}
          </div>
        ) : null}
        {partialPreview && (
          <img
            src={partialPreview}
            alt={t("generate.advanced.partialImages")}
            className="w-full rounded-lg object-contain"
          />
        )}
        {results?.length ? (
          <div
            aria-label={t("canvas.generationResults")}
            className="grid grid-cols-2 gap-2"
          >
            {results.map((path, index) => (
              <img
                key={`${path}-${index}`}
                src={toAssetUrl(path)}
                alt={`${t("canvas.generationResults")} ${index + 1}`}
                className="aspect-square w-full rounded-lg object-contain"
              />
            ))}
          </div>
        ) : null}
        <div className="rounded-[12px] border border-border-subtle bg-surface/75 px-4 py-3 text-[12px] leading-6 text-muted">
          {environmentHint ?? t("canvas.generationEmpty")}
        </div>
      </div>
    </section>
  );
}
