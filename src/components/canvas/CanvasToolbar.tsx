import { useState } from "react";
import {
  Brush,
  Circle,
  Eraser,
  Frame,
  Hand,
  ImagePlus,
  Maximize2,
  Minus,
  MoreHorizontal,
  MousePointer2,
  MoveUpRight,
  Plus,
  Redo2,
  Slash,
  Square,
  Type,
  Undo2,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import type { CanvasTool } from "../../types";
import type { CanvasOrderDirection } from "../../lib/canvas/ordering";
import CanvasActionMenu, { type CanvasActionProps } from "./CanvasActionMenu";
interface CanvasToolbarProps extends CanvasActionProps {
  activeTool: CanvasTool;
  strokeColor: string;
  strokeSize: number;
  canUndo: boolean;
  canRedo: boolean;
  selectedObjectCount: number;
  zoomPercent: number;
  canPaste: boolean;
  onToolChange: (tool: CanvasTool) => void;
  onColorChange: (color: string) => void;
  onSizeChange: (size: number) => void;
  onUndo: () => void;
  onRedo: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onImportImage: () => void;
  onDeleteSelection: () => void;
  onCopySelection: () => void;
  onPasteSelection: () => void;
  onReorderSelection: (direction: CanvasOrderDirection) => void;
  onFitFrame: () => void;
  onFitSelection: () => void;
  onFitAll?: () => void;
  onResetZoom?: () => void;
  onEditFrame?: () => void;
  frameEditing?: boolean;
  fillColor?: string;
  objectOpacity?: number;
  fontSize?: number;
  onFillColorChange?: (color: string) => void;
  onOpacityChange?: (opacity: number) => void;
  onFontSizeChange?: (fontSize: number) => void;
}

const TOOL_BUTTON_CLASS =
  "studio-control focus-ring inline-flex h-9 w-9 shrink-0 items-center justify-center rounded-lg text-foreground/75 transition-colors hover:bg-subtle disabled:opacity-30";
export default function CanvasToolbar(props: CanvasToolbarProps) {
  const { t } = useTranslation();
  const [actions, setActions] = useState(false);
  const [properties, setProperties] = useState(false);
  const [zoomMenu, setZoomMenu] = useState(false);
  const tools: Array<[CanvasTool, typeof MousePointer2, string]> = [
    ["select", MousePointer2, "V"],
    ["pan", Hand, "H / Space"],
    ["rectangle", Square, "R"],
    ["ellipse", Circle, "O"],
    ["line", Slash, "L"],
    ["arrow", MoveUpRight, "A"],
    ["brush", Brush, "P"],
    ["eraser", Eraser, "E"],
    ["text", Type, "T"],
  ];
  const button = (
    label: string,
    action: (() => void) | undefined,
    Icon: typeof MousePointer2,
    disabled = false,
  ) => (
    <button
      type="button"
      key={label}
      aria-label={t(label)}
      title={t(label)}
      onClick={action}
      disabled={disabled || !action}
      className={TOOL_BUTTON_CLASS}
    >
      <Icon size={16} strokeWidth={1.7} />
    </button>
  );
  return (
    <div className="pointer-events-auto relative flex max-w-full flex-col items-center gap-2">
      {properties && (
        <div className="flex max-w-full flex-wrap items-center justify-center gap-3 rounded-xl border border-border-subtle bg-surface/95 px-3 py-2 shadow-float">
          <label className="flex items-center gap-2 text-xs text-muted">
            {t("canvas.strokeWidth")}
            <input
              aria-label={t("canvas.strokeWidth")}
              type="range"
              min="1"
              max="32"
              value={props.strokeSize}
              onChange={(e) => props.onSizeChange(Number(e.target.value))}
              className="w-20 accent-primary"
            />
            <span className="w-6 tabular-nums">{props.strokeSize}</span>
          </label>
          {props.onFillColorChange && (
            <label className="flex items-center gap-2 text-xs text-muted">
              {t("canvas.fill")}
              <input
                aria-label={t("canvas.fill")}
                type="color"
                value={
                  props.fillColor === "transparent"
                    ? "#ffffff"
                    : (props.fillColor ?? "#ffffff")
                }
                onChange={(e) => props.onFillColorChange?.(e.target.value)}
                className="h-6 w-6 border-0 bg-transparent"
              />
              <button
                type="button"
                onClick={() => props.onFillColorChange?.("transparent")}
                className="text-xs hover:text-primary"
              >
                {t("canvas.noFill")}
              </button>
            </label>
          )}
          {props.onOpacityChange && (
            <label className="flex items-center gap-2 text-xs text-muted">
              {t("canvas.opacity")}
              <input
                aria-label={t("canvas.opacity")}
                type="range"
                min="0"
                max="1"
                step="0.05"
                value={props.objectOpacity ?? 1}
                onChange={(e) =>
                  props.onOpacityChange?.(Number(e.target.value))
                }
                className="w-20 accent-primary"
              />
            </label>
          )}
          {props.onFontSizeChange && (
            <label className="flex items-center gap-2 text-xs text-muted">
              {t("canvas.fontSize")}
              <input
                aria-label={t("canvas.fontSize")}
                type="number"
                min="8"
                max="300"
                value={props.fontSize ?? 24}
                onChange={(e) =>
                  props.onFontSizeChange?.(Number(e.target.value))
                }
                className="w-14 rounded border border-border-subtle bg-surface px-1 py-1"
              />
            </label>
          )}
        </div>
      )}
      <div className="flex max-w-full items-center gap-1 rounded-2xl border border-border-subtle bg-surface/95 p-1.5 shadow-float backdrop-blur-xl">
        <div className="flex min-w-0 items-center gap-0.5 overflow-x-auto">
          {tools.map(([tool, Icon, shortcut]) => (
            <button
              type="button"
              key={tool}
              aria-label={t(`canvas.tool.${tool}`)}
              title={`${t(`canvas.tool.${tool}`)} · ${shortcut}`}
              aria-pressed={!props.frameEditing && props.activeTool === tool}
              onClick={() => props.onToolChange(tool)}
              className={`${TOOL_BUTTON_CLASS} ${!props.frameEditing && props.activeTool === tool ? "bg-primary/12 text-primary" : ""}`}
            >
              <Icon size={17} strokeWidth={1.7} />
            </button>
          ))}
          {props.onEditFrame && (
            <button
              type="button"
              aria-label={t("canvas.editFrame")}
              title={t("canvas.editFrame")}
              aria-pressed={props.frameEditing}
              onClick={props.onEditFrame}
              className={`${TOOL_BUTTON_CLASS} ${props.frameEditing ? "bg-primary/12 text-primary" : ""}`}
            >
              <Frame size={17} />
            </button>
          )}
          {button("canvas.importImage", props.onImportImage, ImagePlus)}
        </div>
        <span className="mx-1 h-5 w-px shrink-0 bg-border-subtle" />
        <input
          aria-label={t("canvas.strokeColor")}
          title={t("canvas.strokeColor")}
          type="color"
          value={props.strokeColor}
          onChange={(e) => props.onColorChange(e.target.value)}
          className="h-7 w-7 shrink-0 cursor-pointer border-0 bg-transparent p-0.5"
        />
        <button
          type="button"
          aria-label={t("canvas.properties")}
          title={t("canvas.properties")}
          aria-expanded={properties}
          onClick={() => setProperties(!properties)}
          className={`${TOOL_BUTTON_CLASS} text-[11px] tabular-nums`}
        >
          {props.strokeSize}px
        </button>
        <div className="relative">
          {button("canvas.actions", () => setActions(!actions), MoreHorizontal)}
          {actions && (
            <CanvasActionMenu
              {...props}
              hasSelection={props.selectedObjectCount > 0}
              selectionCount={props.selectedObjectCount}
              onClose={() => setActions(false)}
              style={{ bottom: 48, right: 0 }}
            />
          )}
        </div>
      </div>
      <div className="flex items-center gap-0.5 rounded-xl border border-border-subtle bg-surface/90 px-1 py-0.5 shadow-card backdrop-blur-xl">
        {button("canvas.tool.undo", props.onUndo, Undo2, !props.canUndo)}
        {button("canvas.tool.redo", props.onRedo, Redo2, !props.canRedo)}
        <span className="mx-1 h-4 w-px bg-border-subtle" />
        {button("canvas.tool.zoomOut", props.onZoomOut, Minus)}
        <div className="relative">
          <button
            type="button"
            aria-label={t("canvas.zoomOptions")}
            aria-expanded={zoomMenu}
            onClick={() => setZoomMenu(!zoomMenu)}
            className="focus-ring min-w-14 rounded-lg px-1 py-2 text-xs tabular-nums text-muted"
          >
            {props.zoomPercent}%
          </button>
          {zoomMenu && (
            <div
              role="menu"
              className="absolute bottom-11 left-1/2 z-40 min-w-36 -translate-x-1/2 rounded-xl border border-border-subtle bg-surface p-1.5 shadow-float"
            >
              {[
                ["canvas.fitFrame", props.onFitFrame],
                [
                  "canvas.fitSelection",
                  props.selectedObjectCount ? props.onFitSelection : undefined,
                ],
                ["canvas.fitAll", props.onFitAll],
                ["canvas.resetZoom", props.onResetZoom],
              ].map(
                ([label, action]) =>
                  action && (
                    <button
                      key={label as string}
                      type="button"
                      role="menuitem"
                      onClick={() => {
                        (action as () => void)();
                        setZoomMenu(false);
                      }}
                      className="block w-full rounded-lg px-3 py-2 text-left text-xs hover:bg-subtle"
                    >
                      {t(label as string)}
                    </button>
                  ),
              )}
            </div>
          )}
        </div>
        {button("canvas.tool.zoomIn", props.onZoomIn, Plus)}
        {button("canvas.fitFrame", props.onFitFrame, Maximize2)}
        <span className="mx-2 hidden text-[10px] text-muted sm:inline">
          {t("canvas.selectionCount", { count: props.selectedObjectCount })}
        </span>
      </div>
    </div>
  );
}
