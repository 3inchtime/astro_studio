import { Loader2, PanelRight, Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { CanvasFrame, ImageModel } from "../../types";

interface CanvasGenerationPanelProps {
  prompt: string;
  imageModel: ImageModel;
  frame: CanvasFrame | null;
  disabled: boolean;
  isGenerating: boolean;
  environmentHint?: string | null;
  onPromptChange: (value: string) => void;
  onGenerate: () => void;
}

export default function CanvasGenerationPanel({
  prompt,
  imageModel,
  frame,
  disabled,
  isGenerating,
  environmentHint,
  onPromptChange,
  onGenerate,
}: CanvasGenerationPanelProps) {
  const { t } = useTranslation();

  return (
    <section className="rounded-[16px] border border-border-subtle bg-surface/92 shadow-card">
      <div className="border-b border-border-subtle px-4 py-4">
        <div className="flex items-center gap-2 text-[13px] font-semibold text-foreground">
          <PanelRight size={14} strokeWidth={1.8} />
          <span>{t("canvas.generationTitle")}</span>
        </div>
      </div>

      <div className="flex flex-col gap-4 p-4">
        <div className="space-y-2">
          <label className="block text-[12px] font-medium text-foreground/80">
            {t("generate.modelLabel")}
          </label>
          <div className="rounded-[12px] border border-border-subtle bg-surface px-3 py-2 text-[13px] text-foreground">
            {imageModel}
          </div>
        </div>

        <div className="grid grid-cols-2 gap-2">
          <div className="rounded-[12px] border border-border-subtle bg-surface/80 px-3 py-2">
            <div className="text-[10px] uppercase tracking-[0.04em] text-muted">
              {t("canvas.frameAspect")}
            </div>
            <div className="mt-1 text-[13px] font-medium text-foreground">
              {frame?.aspect ?? "--"}
            </div>
          </div>
          <div className="rounded-[12px] border border-border-subtle bg-surface/80 px-3 py-2">
            <div className="text-[10px] uppercase tracking-[0.04em] text-muted">
              {t("canvas.layersTitle")}
            </div>
            <div className="mt-1 text-[13px] font-medium text-foreground">
              {frame ? `${Math.round(frame.width)} x ${Math.round(frame.height)}` : "--"}
            </div>
          </div>
        </div>

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
          className="studio-input -mt-2 min-h-[320px] w-full resize-y rounded-[14px] px-4 py-3 text-[14px] leading-[1.65]"
        />

        <button
          type="button"
          onClick={onGenerate}
          disabled={disabled}
          aria-busy={isGenerating}
          className="studio-control-primary focus-ring inline-flex items-center justify-center gap-2 rounded-[12px] px-4 py-3 text-[13px] font-semibold disabled:cursor-not-allowed disabled:opacity-50"
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

        <div className="rounded-[12px] border border-border-subtle bg-surface/75 px-4 py-3 text-[12px] leading-6 text-muted">
          {environmentHint ?? t("canvas.generationEmpty")}
        </div>
      </div>
    </section>
  );
}
