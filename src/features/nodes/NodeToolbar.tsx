import { useReactFlow, useViewport } from "@xyflow/react";
import { Maximize, Minus, Plus, Undo2 } from "lucide-react";
import type { RefObject } from "react";
import { useTranslation } from "react-i18next";
import type { NodeKind } from "./graph";
import { nodeIcons } from "./WorkflowCard";

export const kinds: NodeKind[] = ["text", "reference", "generate"];
export const dragFormat = "application/x-astro-workflow-node";
export const fitOptions = { padding: 0.25, maxZoom: 1 };

function ZoomControls() {
  const { t } = useTranslation();
  const { zoomIn, zoomOut, fitView } = useReactFlow();
  const { zoom } = useViewport();
  return <div className="workflow-toolbar workflow-zoom">
    <button aria-label={t("nodes.zoomOut")} title={t("nodes.zoomOut")} disabled={zoom <= 0.2} onClick={() => void zoomOut()}><Minus size={16} /></button>
    <span>{Math.round(zoom * 100)}%</span>
    <button aria-label={t("nodes.zoomIn")} title={t("nodes.zoomIn")} disabled={zoom >= 2} onClick={() => void zoomIn()}><Plus size={16} /></button>
    <i aria-hidden="true" />
    <button aria-label={t("nodes.fit")} title={t("nodes.fit")} onClick={() => void fitView(fitOptions)}><Maximize size={16} /></button>
  </div>;
}

export default function NodeToolbar({ onAdd, onUndo, undoAvailable, undoButton }: {
  onAdd: (kind: NodeKind) => void;
  onUndo: () => void;
  undoAvailable: boolean;
  undoButton: RefObject<HTMLButtonElement | null>;
}) {
  const { t } = useTranslation();
  return <div className="workflow-command-bar">
    <div className="workflow-toolbar workflow-palette">
      <div role="group" aria-label={t("nodes.add")}>
        <span className="workflow-add-label"><Plus size={16} />{t("nodes.add")}</span>
        {kinds.map(kind => {
          const Icon = nodeIcons[kind];
          const label = t(`nodes.types.${kind}`);
          return <button key={kind} type="button" draggable aria-label={label} title={label}
            onDragStart={event => { event.dataTransfer.setData(dragFormat, kind); event.dataTransfer.effectAllowed = "copy"; }}
            onClick={() => onAdd(kind)}><Icon size={16} /><span>{label}</span></button>;
        })}
      </div>
    </div>
    <div className="workflow-view-tools">
      <button type="button" ref={undoButton} className="workflow-icon-button"
        aria-label={t("nodes.undoRemove")} title={t("nodes.undoRemove")}
        disabled={!undoAvailable} onClick={onUndo}><Undo2 size={16} /></button>
      <ZoomControls />
    </div>
  </div>;
}
