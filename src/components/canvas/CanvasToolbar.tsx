import {
  Brush,
  Eraser,
  Hand,
  ImagePlus,
  Minus,
  MousePointer2,
  Plus,
  Redo2,
  Undo2,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import type { CanvasTool } from "../../types";

interface CanvasToolbarProps {
  activeTool: CanvasTool;
  strokeColor: string;
  strokeSize: number;
  canUndo: boolean;
  canRedo: boolean;
  onToolChange: (tool: CanvasTool) => void;
  onColorChange: (color: string) => void;
  onSizeChange: (size: number) => void;
  onUndo: () => void;
  onRedo: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onImportImage: () => void;
}

const TOOL_BUTTON_CLASS =
  "studio-control focus-ring inline-flex h-10 w-10 items-center justify-center rounded-[10px] transition-colors hover:studio-control-hover";

export default function CanvasToolbar({
  activeTool,
  strokeColor,
  strokeSize,
  canUndo,
  canRedo,
  onToolChange,
  onColorChange,
  onSizeChange,
  onUndo,
  onRedo,
  onZoomIn,
  onZoomOut,
  onImportImage,
}: CanvasToolbarProps) {
  const { t } = useTranslation();

  const tools: Array<{
    key: CanvasTool;
    label: string;
    icon: typeof MousePointer2;
  }> = [
    { key: "select", label: t("canvas.tool.select"), icon: MousePointer2 },
    { key: "brush", label: t("canvas.tool.brush"), icon: Brush },
    { key: "eraser", label: t("canvas.tool.eraser"), icon: Eraser },
    { key: "pan", label: t("canvas.tool.pan"), icon: Hand },
  ];

  return (
    <div className="pointer-events-auto max-w-full rounded-[18px] border border-border-subtle bg-surface/90 px-3 py-2 shadow-float backdrop-blur-xl">
      <div className="flex flex-wrap items-center justify-center gap-3">
        <div className="flex flex-wrap items-center gap-1.5">
          {tools.map(({ key, label, icon: Icon }) => (
            <button
              key={key}
              type="button"
              aria-label={label}
              title={label}
              onClick={() => onToolChange(key)}
              className={`${TOOL_BUTTON_CLASS} ${
                activeTool === key ? "bg-primary/12 text-primary" : "text-foreground/80"
              }`}
            >
              <Icon size={16} strokeWidth={1.8} />
            </button>
          ))}

          <button
            type="button"
            aria-label={t("canvas.importImage")}
            title={t("canvas.importImage")}
            onClick={onImportImage}
            className={TOOL_BUTTON_CLASS}
          >
            <ImagePlus size={16} strokeWidth={1.8} />
          </button>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <label className="flex h-10 items-center gap-2 rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] text-foreground/80">
            <span>{t("canvas.tool.brush")}</span>
            <input
              type="color"
              value={strokeColor}
              onChange={(event) => onColorChange(event.target.value)}
              className="h-6 w-6 rounded border-0 bg-transparent p-0"
            />
          </label>

          <label className="flex h-10 items-center gap-3 rounded-[10px] border border-border-subtle bg-surface px-3 text-[12px] text-foreground/80">
            <span>{strokeSize}px</span>
            <input
              type="range"
              min={2}
              max={32}
              step={1}
              value={strokeSize}
              onChange={(event) => onSizeChange(Number(event.target.value))}
              className="w-28 accent-primary"
            />
          </label>

          <div className="flex items-center gap-2">
            <button
              type="button"
              aria-label={t("canvas.tool.undo")}
              title={t("canvas.tool.undo")}
              onClick={onUndo}
              disabled={!canUndo}
              className={TOOL_BUTTON_CLASS}
            >
              <Undo2 size={16} strokeWidth={1.8} />
            </button>
            <button
              type="button"
              aria-label={t("canvas.tool.redo")}
              title={t("canvas.tool.redo")}
              onClick={onRedo}
              disabled={!canRedo}
              className={TOOL_BUTTON_CLASS}
            >
              <Redo2 size={16} strokeWidth={1.8} />
            </button>
            <button
              type="button"
              aria-label={t("canvas.tool.zoomOut")}
              title={t("canvas.tool.zoomOut")}
              onClick={onZoomOut}
              className={TOOL_BUTTON_CLASS}
            >
              <Minus size={16} strokeWidth={1.8} />
            </button>
            <button
              type="button"
              aria-label={t("canvas.tool.zoomIn")}
              title={t("canvas.tool.zoomIn")}
              onClick={onZoomIn}
              className={TOOL_BUTTON_CLASS}
            >
              <Plus size={16} strokeWidth={1.8} />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
