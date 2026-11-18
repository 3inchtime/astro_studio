import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import { Aperture, Image as ImageIcon, ImagePlus, GitBranchPlus, LoaderCircle, Play, TextCursorInput, Trash2 } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toAssetUrl } from "../../lib/desktop";
import { IMAGE_MODEL_CATALOG } from "../../lib/modelCatalog";
import type { WorkflowNode } from "./graph";

export const nodeIcons = { text: TextCursorInput, reference: ImagePlus, generate: Aperture };
export type Port = { id: string; type: "source" | "target" };
export type CardNode = Node<{
  node: WorkflowNode;
  inputError?: string;
  busy: boolean;
  removable: boolean;
  pending: Port | null;
  update: (id: string, edit: Partial<WorkflowNode>) => void;
  remove: (id: string) => void;
  port: (port: Port) => void;
  run: (id: string) => void;
  pick: (id: string) => void;
  preview: (path: string) => void;
  variation: (id: string) => void;
  detach?: (id: string) => void;
}, "workflow">;

export function WorkflowImage({ path, alt }: { path: string; alt: string }) {
  const { t } = useTranslation();
  const [failed, setFailed] = useState(false);
  return failed ? <span className="workflow-image-fallback">{t("nodes.unableToLoad")}</span> :
    <img src={toAssetUrl(path)} alt={alt} draggable={false} onError={() => setFailed(true)} />;
}

export default function WorkflowCard({ data }: NodeProps<CardNode>) {
  const { t } = useTranslation();
  const { node, busy } = data;
  const error = data.inputError || node.error;
  const title = t(`nodes.types.${node.kind}`);
  const Icon = nodeIcons[node.kind];
  const running = busy && Boolean(node.clientRequestId || node.jobId);
  const path = node.kind === "reference" ? node.path : node.resultPath;

  function port(type: Port["type"]) {
    const input = type === "target";
    const label = t(input ? "nodes.connectInput" : "nodes.connectOutput", { name: title });
    const pending = data.pending?.id === node.id && data.pending.type === type;
    function choose() {
      if (!busy) data.port({ id: node.id, type });
    }
    return <Handle type={type} position={input ? Position.Left : Position.Right} isConnectable={!busy}
      isConnectableStart={!busy} isConnectableEnd={!busy}
      className={`workflow-port ${pending ? "is-pending" : ""}`} role="button"
      tabIndex={busy ? -1 : 0} aria-disabled={busy} aria-pressed={pending} aria-label={label} title={label}
      onClick={event => { event.stopPropagation(); choose(); }}
      onKeyDown={event => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          event.stopPropagation();
          choose();
        }
      }} />;
  }

  return <article className={`workflow-card workflow-card-${node.kind}`}>
    <header className="workflow-card-header">
      <span className="workflow-card-icon"><Icon size={18} aria-hidden="true" /></span>
      <strong>{title}</strong>
      <button type="button" className="nodrag workflow-icon-button workflow-remove"
        aria-label={t("nodes.remove")} title={t("nodes.remove")} disabled={!data.removable}
        onClick={() => data.remove(node.id)}><Trash2 size={15} /></button>
    </header>
    <div className="workflow-ports">
      {node.kind === "generate" && <span>{t("nodes.inputs")}{port("target")}</span>}
      <span className="workflow-output">{t(node.kind === "text" ? "nodes.textOutput" : "nodes.imageOutput")}{port("source")}</span>
    </div>
    <div className="workflow-card-body nodrag nopan">
      {node.kind === "text" && <textarea className="nowheel" value={node.text}
        aria-label={title} placeholder={t("nodes.promptPlaceholder")} disabled={busy}
        onChange={event => data.update(node.id, { text: event.target.value })} />}
      {node.kind === "reference" && <>
        <button type="button" className={`workflow-image-box workflow-pick ${path ? "has-image" : ""}`}
          disabled={busy} onClick={() => data.pick(node.id)}
          aria-label={t(path ? "nodes.changeImage" : "nodes.pickImage")}>
          {path ? <WorkflowImage key={path} path={path} alt={title} /> : <ImagePlus size={26} strokeWidth={1.5} />}
          <span className="workflow-pick-label">{t(path ? "nodes.changeImage" : "nodes.pickImage")}</span>
        </button>
      </>}
      {node.kind === "generate" && <>
        {path ? <button className="workflow-image-box workflow-result" type="button"
          onClick={() => data.preview(path)} aria-label={t("nodes.result")}>
          <WorkflowImage key={path} path={path} alt={t("nodes.result")} />
        </button> : <div className="workflow-image-box workflow-empty-preview">
          <ImageIcon size={28} strokeWidth={1.4} aria-hidden="true" />
          <span>{t("nodes.noResult")}</span>
        </div>}
        <label className="workflow-model">
          <span>{t("nodes.model")}</span>
          <select className="nowheel" value={node.model} disabled={busy}
            onChange={event => data.update(node.id, { model: event.target.value as WorkflowNode["model"] })}>
            {IMAGE_MODEL_CATALOG.map(entry => <option key={entry.id} value={entry.id}>{t(entry.i18nKey)}</option>)}
          </select>
        </label>
        {node.clientRequestId && node.error && data.detach
          ? <button type="button" className="workflow-secondary" onClick={() => data.detach?.(node.id)}>
            {t("nodes.detachRequest")}
          </button>
          : <button type="button" className="workflow-run" disabled={busy} onClick={() => data.run(node.id)}>
          {running ? <LoaderCircle size={15} className="animate-spin motion-reduce:animate-none" /> : <Play size={15} />}
          {t(running ? "nodes.running" : "nodes.run")}
        </button>}
      </>}
      {path && <button type="button" className="workflow-secondary" disabled={busy} onClick={() => data.variation(node.id)}>
        <GitBranchPlus size={15} />{t("nodes.variation")}
      </button>}
      {error && <p className="workflow-error nowheel" role="alert">{error}</p>}
    </div>
  </article>;
}
