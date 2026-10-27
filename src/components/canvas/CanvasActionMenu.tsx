import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { CanvasOrderDirection } from "../../lib/canvas/ordering";
import type { CanvasAlignment } from "../../lib/canvas/editor";
export interface CanvasActionProps {
  onCopySelection?: () => void;
  onPasteSelection?: () => void;
  onDuplicateSelection?: () => void;
  onDeleteSelection?: () => void;
  onReorderSelection?: (direction: CanvasOrderDirection) => void;
  onFrameFromSelection?: () => void;
  onSelectAll?: () => void;
  onAlignSelection?: (alignment: CanvasAlignment) => void;
  onDistributeSelection?: (axis: "horizontal" | "vertical") => void;
  canPaste?: boolean;
}
export default function CanvasActionMenu(
  props: CanvasActionProps & {
    hasSelection: boolean;
    selectionCount?: number;
    onClose: () => void;
    style?: React.CSSProperties;
  },
) {
  const { t } = useTranslation();
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current
      ?.querySelector<HTMLButtonElement>("button:not(:disabled)")
      ?.focus();
    function outside(event: MouseEvent) {
      if (!ref.current?.contains(event.target as Node)) props.onClose();
    }
    document.addEventListener("mousedown", outside);
    return () => document.removeEventListener("mousedown", outside);
  }, [props.onClose]);
  const entries: Array<[string, (() => void) | undefined, boolean]> = [
    ["selectAll", props.onSelectAll, true],
    ["copySelection", props.onCopySelection, props.hasSelection],
    ["pasteSelection", props.onPasteSelection, !!props.canPaste],
    ["duplicateSelection", props.onDuplicateSelection, props.hasSelection],
    [
      "bringToFront",
      props.onReorderSelection && (() => props.onReorderSelection?.("front")),
      props.hasSelection,
    ],
    [
      "sendToBack",
      props.onReorderSelection && (() => props.onReorderSelection?.("back")),
      props.hasSelection,
    ],
    ["frameFromSelection", props.onFrameFromSelection, props.hasSelection],
    ...(
      [
        "left",
        "center",
        "right",
        "top",
        "middle",
        "bottom",
      ] as CanvasAlignment[]
    ).map(
      (a) =>
        [
          `align.${a}`,
          props.onAlignSelection && (() => props.onAlignSelection?.(a)),
          (props.selectionCount ?? 0) > 1,
        ] as [string, (() => void) | undefined, boolean],
    ),
    ...(["horizontal", "vertical"] as const).map(
      (a) =>
        [
          `distribute.${a}`,
          props.onDistributeSelection &&
            (() => props.onDistributeSelection?.(a)),
          (props.selectionCount ?? 0) > 2,
        ] as [string, (() => void) | undefined, boolean],
    ),
    ["deleteSelection", props.onDeleteSelection, props.hasSelection],
  ];
  return (
    <div
      ref={ref}
      role="menu"
      aria-label={t("canvas.actions")}
      style={props.style}
      className="absolute z-50 min-w-48 max-h-[min(70vh,480px)] overflow-y-auto rounded-xl border border-border-subtle bg-surface p-1.5 shadow-float"
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.stopPropagation();
          props.onClose();
        }
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          event.preventDefault();
          const items = [
            ...(ref.current?.querySelectorAll<HTMLButtonElement>(
              "button:not(:disabled)",
            ) ?? []),
          ];
          const i = items.indexOf(document.activeElement as HTMLButtonElement);
          items[
            (i + (event.key === "ArrowDown" ? 1 : -1) + items.length) %
              items.length
          ]?.focus();
        }
      }}
    >
      {entries
        .filter(([, action]) => action)
        .map(([label, action, enabled]) => (
          <button
            type="button"
            key={label}
            role="menuitem"
            disabled={!enabled}
            onClick={() => {
              action?.();
              props.onClose();
            }}
            className={`focus-ring flex w-full items-center rounded-lg px-3 py-2 text-left text-xs hover:bg-subtle disabled:opacity-35 ${label === "deleteSelection" ? "text-red-500" : "text-foreground"}`}
          >
            {t(`canvas.${label}`)}
          </button>
        ))}
    </div>
  );
}
