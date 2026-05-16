import { Eye, EyeOff, Layers3, Lock, LockOpen, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { CanvasLayer } from "../../types";

interface CanvasLayersPanelProps {
  layers: CanvasLayer[];
  activeLayerId: string | null;
  onSelectLayer: (layerId: string) => void;
  onAddLayer: () => void;
  onToggleLayerVisibility: (layerId: string) => void;
  onToggleLayerLock: (layerId: string) => void;
}

export default function CanvasLayersPanel({
  layers,
  activeLayerId,
  onSelectLayer,
  onAddLayer,
  onToggleLayerVisibility,
  onToggleLayerLock,
}: CanvasLayersPanelProps) {
  const { t } = useTranslation();

  return (
    <section className="rounded-[16px] border border-border-subtle bg-surface/92 shadow-card">
      <div className="flex items-center justify-between border-b border-border-subtle px-4 py-3">
        <div className="flex items-center gap-2 text-[12px] font-semibold text-foreground">
          <Layers3 size={14} strokeWidth={1.8} />
          <span>{t("canvas.layersTitle")}</span>
        </div>
        <button
          type="button"
          aria-label={t("canvas.newLayer")}
          title={t("canvas.newLayer")}
          onClick={onAddLayer}
          className="studio-control focus-ring inline-flex h-8 w-8 items-center justify-center rounded-[8px] text-foreground/80 hover:studio-control-hover"
        >
          <Plus size={15} strokeWidth={1.8} />
        </button>
      </div>

      <div className="space-y-2 p-3">
        {layers.map((layer) => {
          const selected = layer.id === activeLayerId;
          return (
            <div
              key={layer.id}
              className={`rounded-[10px] border px-3 py-2 transition-colors ${
                selected
                  ? "border-primary/25 bg-primary/8"
                  : "border-transparent bg-surface-muted/85"
              }`}
            >
              <button
                type="button"
                onClick={() => onSelectLayer(layer.id)}
                className="flex w-full cursor-pointer items-center justify-between gap-2 text-left"
              >
                <div>
                  <div className="text-[13px] font-medium text-foreground">
                    {layer.name}
                  </div>
                  <div className="mt-1 text-[11px] text-muted">
                    {t("canvas.objectCount", { count: layer.objects.length })}
                  </div>
                </div>
              </button>

              <div className="mt-2 flex items-center gap-2">
                <button
                  type="button"
                  aria-label={layer.visible ? t("canvas.hideLayer") : t("canvas.showLayer")}
                  title={layer.visible ? t("canvas.hideLayer") : t("canvas.showLayer")}
                  onClick={() => onToggleLayerVisibility(layer.id)}
                  className="studio-control focus-ring inline-flex h-8 w-8 items-center justify-center rounded-[8px] text-foreground/80 hover:studio-control-hover"
                >
                  {layer.visible ? (
                    <Eye size={14} strokeWidth={1.8} />
                  ) : (
                    <EyeOff size={14} strokeWidth={1.8} />
                  )}
                </button>
                <button
                  type="button"
                  aria-label={layer.locked ? t("canvas.unlockLayer") : t("canvas.lockLayer")}
                  title={layer.locked ? t("canvas.unlockLayer") : t("canvas.lockLayer")}
                  onClick={() => onToggleLayerLock(layer.id)}
                  className="studio-control focus-ring inline-flex h-8 w-8 items-center justify-center rounded-[8px] text-foreground/80 hover:studio-control-hover"
                >
                  {layer.locked ? (
                    <Lock size={14} strokeWidth={1.8} />
                  ) : (
                    <LockOpen size={14} strokeWidth={1.8} />
                  )}
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </section>
  );
}
