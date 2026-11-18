import {
  Background, BackgroundVariant, Panel, ReactFlow, ReactFlowProvider, useReactFlow, useViewport,
  type Connection, type Edge, type NodeChange, type XYPosition,
} from "@xyflow/react";
import { Maximize, Minus, Plus, Unplug, X } from "lucide-react";
import { useEffect, useRef, useState, type DragEvent } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { useDialogFocus } from "../../hooks/useDialogFocus";
import { connectNodes, GraphError, makeNode, type NodeKind, type WorkflowGraph, type WorkflowNode } from "./graph";
import WorkflowCard, { nodeIcons, WorkflowImage, type CardNode, type Port } from "./WorkflowCard";
import "@xyflow/react/dist/style.css";
import "./nodes.css";

type Props = {
  graph: WorkflowGraph;
  onChange: (graph: WorkflowGraph) => void;
  onRun: (id: string) => void;
  onPick: (id: string) => void;
  onDetach?: (id: string) => void;
  busyIds: ReadonlySet<string>;
};

type Layout = Pick<CardNode, "measured" | "selected"> & { position?: XYPosition };
const nodeTypes = { workflow: WorkflowCard };
const kinds: NodeKind[] = ["text", "reference", "generate"];
const dragFormat = "application/x-astro-workflow-node";
const fitOptions = { padding: 0.25, maxZoom: 1 };
const edgeId = (edge: Pick<Edge, "source" | "target">) => JSON.stringify([edge.source, edge.target]);

function ZoomControls() {
  const { t } = useTranslation();
  const { zoomIn, zoomOut, fitView } = useReactFlow();
  const { zoom } = useViewport();
  return <Panel position="bottom-left" className="workflow-toolbar workflow-zoom">
    <button aria-label={t("nodes.zoomOut")} title={t("nodes.zoomOut")} disabled={zoom <= 0.2} onClick={() => void zoomOut()}><Minus size={16} /></button>
    <span>{Math.round(zoom * 100)}%</span>
    <button aria-label={t("nodes.zoomIn")} title={t("nodes.zoomIn")} disabled={zoom >= 2} onClick={() => void zoomIn()}><Plus size={16} /></button>
    <i aria-hidden="true" />
    <button aria-label={t("nodes.fit")} title={t("nodes.fit")} onClick={() => void fitView(fitOptions)}><Maximize size={16} /></button>
  </Panel>;
}

function Canvas({ graph, onChange, onRun, onPick, onDetach, busyIds }: Props) {
  const { t } = useTranslation();
  const { screenToFlowPosition, getZoom, setCenter } = useReactFlow();
  const container = useRef<HTMLDivElement>(null);
  const current = useRef(graph);
  current.current = graph;
  const [layout, setLayout] = useState<Record<string, Layout>>({});
  const promptToFocus = useRef<string | null>(null);
  const [selectedEdge, setSelectedEdge] = useState<string | null>(null);
  const [pending, setPending] = useState<Port | null>(null);
  const [notice, setNotice] = useState("");
  const [preview, setPreview] = useState("");
  const dialog = useDialogFocus({ open: Boolean(preview), onClose: () => setPreview("") });

  function commit(next: WorkflowGraph) {
    current.current = next;
    onChange(next);
  }

  function update(id: string, edit: Partial<WorkflowNode>) {
    if (busyIds.has(id)) return;
    const next = current.current;
    commit({ ...next, nodes: next.nodes.map(node => node.id === id ? { ...node, ...edit } : node) });
  }

  function canDelete(id: string) {
    return !busyIds.has(id) && !current.current.edges.some(edge =>
      (edge.source === id && busyIds.has(edge.target)) || (edge.target === id && busyIds.has(edge.source)));
  }

  function remove(id: string) {
    if (!canDelete(id)) return;
    const next = current.current;
    commit({ ...next, nodes: next.nodes.filter(node => node.id !== id),
      edges: next.edges.filter(edge => edge.source !== id && edge.target !== id) });
    if (pending?.id === id) setPending(null);
    setLayout(previous => {
      const clean = { ...previous };
      delete clean[id];
      return clean;
    });
  }

  function disconnect(id: string) {
    const next = current.current;
    const edge = next.edges.find(edge => edgeId(edge) === id);
    if (!edge || busyIds.has(edge.source) || busyIds.has(edge.target)) return;
    commit({ ...next, edges: next.edges.filter(other => other !== edge) });
    setSelectedEdge(previous => previous === id ? null : previous);
  }

  function connect({ source, target }: Pick<Connection, "source" | "target">) {
    setPending(null);
    if (busyIds.has(source) || busyIds.has(target)) return;
    try {
      commit(connectNodes(current.current, source, target));
      setNotice("");
    } catch (error) {
      if (!(error instanceof GraphError)) throw error;
      setNotice(t(`nodes.errors.${error.code}`));
    }
  }

  function selectPort(port: Port) {
    setNotice("");
    if (!pending || pending.type === port.type) {
      setPending(pending?.id === port.id && pending.type === port.type ? null : port);
      return;
    }
    connect(port.type === "source" ? { source: port.id, target: pending.id } : { source: pending.id, target: port.id });
  }

  function openSpace(x: number, y: number, occupied: WorkflowNode[]) {
    // Sorting lets each collision move the candidate below a card in a single pass.
    for (const node of [...occupied].sort((a, b) => a.y - b.y)) {
      const width = layout[node.id]?.measured?.width ?? 294;
      const height = layout[node.id]?.measured?.height ?? (node.kind === "text" ? 210 : 360);
      if (x < node.x + width + 24 && x + 318 > node.x && y < node.y + height + 24 && y + 384 > node.y) {
        y = node.y + height + 32;
      }
    }
    return { x, y };
  }

  useEffect(() => {
    const id = promptToFocus.current;
    if (!id || !layout[id]?.measured) return;
    const card = Array.from(container.current?.querySelectorAll<HTMLElement>(".react-flow__node") ?? [])
      .find(element => element.dataset.id === id);
    const input = card?.querySelector("textarea");
    if (input) { input.focus({ preventScroll: true }); promptToFocus.current = null; }
  }, [layout]);

  function variation(id: string) {
    const next = current.current;
    const source = next.nodes.find(node => node.id === id);
    if (!source || busyIds.has(id) || !(source.kind === "reference" ? source.path : source.resultPath)) return;
    const promptPosition = openSpace(source.x, source.y + (layout[id]?.measured?.height ?? 360) + 32, next.nodes);
    const prompt = makeNode("text", promptPosition.x, promptPosition.y);
    const generatorPosition = openSpace(source.x + (layout[id]?.measured?.width ?? 294) + 85,
      (source.y + prompt.y) / 2, [...next.nodes, prompt]);
    const generator = makeNode("generate", generatorPosition.x, generatorPosition.y);
    if (source.kind === "generate") generator.model = source.model;
    const branch = { ...next, nodes: [...next.nodes, prompt, generator] };
    commit(connectNodes(connectNodes(branch, source.id, generator.id), prompt.id, generator.id));
    promptToFocus.current = prompt.id;
    setLayout(previous => ({
      ...Object.fromEntries(Object.entries(previous).map(([key, value]) => [key, { ...value, selected: false }])),
      [prompt.id]: { selected: true },
    }));
    setSelectedEdge(null);
    void setCenter(prompt.x + 136, prompt.y + 105, { zoom: Math.max(getZoom(), 0.75) });
  }

  function add(kind: NodeKind, point?: XYPosition) {
    const rect = container.current?.getBoundingClientRect();
    if (!rect) return;
    const next = current.current;
    const center = screenToFlowPosition({ x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 });
    const position = point ?? openSpace(center.x - 136, center.y - 96, next.nodes);
    const node = makeNode(kind, position.x, position.y);
    commit({ ...next, nodes: [...next.nodes, node] });
    if (!point) void setCenter(node.x + 136, node.y + 110, { zoom: Math.max(getZoom(), 0.75) });
  }

  function drop(event: DragEvent) {
    const kind = event.dataTransfer.getData(dragFormat) as NodeKind;
    if (kinds.includes(kind)) {
      event.preventDefault();
      add(kind, screenToFlowPosition({ x: event.clientX, y: event.clientY }));
    }
  }

  function changeNodes(changes: NodeChange<CardNode>[]) {
    setLayout(previous => {
      const next = { ...previous };
      for (const change of changes) {
        if (change.type === "dimensions") next[change.id] = { ...next[change.id], measured: change.dimensions };
        if (change.type === "select") next[change.id] = { ...next[change.id], selected: change.selected };
        if (change.type === "position") next[change.id] = {
          ...next[change.id], position: change.dragging ? change.position : undefined,
        };
      }
      return next;
    });
    // Only write finished moves to the durable graph. Keyboard moves are already final.
    const positions = new Map<string, XYPosition>();
    for (const change of changes) {
      if (change.type === "position" && !change.dragging && change.position && !busyIds.has(change.id)) {
        positions.set(change.id, change.position);
      }
    }
    if (positions.size) {
      const next = current.current;
      commit({ ...next, nodes: next.nodes.map(node => positions.has(node.id) ? { ...node, ...positions.get(node.id) } : node) });
    }
  }

  const nodes: CardNode[] = graph.nodes.map(node => ({
    id: node.id, type: "workflow", ...layout[node.id], position: layout[node.id]?.position ?? { x: node.x, y: node.y },
    ariaLabel: t(`nodes.types.${node.kind}`), dragHandle: ".workflow-card-header", draggable: !busyIds.has(node.id),
    data: { node, busy: busyIds.has(node.id), removable: canDelete(node.id), pending, update, remove,
      port: selectPort, run: onRun, pick: onPick, preview: setPreview, variation, detach: onDetach },
  }));
  const titles = new Map(nodes.map(node => [node.id, node.ariaLabel]));
  const edges = graph.edges.map(edge => ({
    ...edge, id: edgeId(edge), selected: selectedEdge === edgeId(edge), interactionWidth: 22,
    ariaLabel: t("nodes.edgeLabel", { source: titles.get(edge.source), target: titles.get(edge.target) }),
  }));
  const selected = edges.find(edge => edge.selected);
  const message = notice || (pending ? t(pending.type === "source" ? "nodes.pickInput" : "nodes.pickOutput") : "");

  return <div className="workflow-stage" ref={container}
    onKeyDown={event => {
      if (event.key === "Escape") { setPending(null); setNotice(""); }
      if ((event.key !== "Delete" && event.key !== "Backspace") || !(event.target instanceof Element)) return;
      if (event.target.matches(".react-flow__node")) {
        event.preventDefault();
        const id = event.target.getAttribute("data-id");
        if (id) remove(id);
      } else if (event.target.matches(".react-flow__edge")) {
        event.preventDefault();
        const id = event.target.getAttribute("data-id");
        if (id) disconnect(id);
      }
    }}>
    <ReactFlow<CardNode> nodes={nodes} edges={edges} nodeTypes={nodeTypes} onNodesChange={changeNodes}
      onEdgesChange={changes => {
        for (const change of changes) if (change.type === "select") {
          setSelectedEdge(previous => change.selected ? change.id : previous === change.id ? null : previous);
        }
      }}
      fitView fitViewOptions={fitOptions} minZoom={0.2} maxZoom={2} zoomOnDoubleClick={false}
      deleteKeyCode={null} multiSelectionKeyCode={null} selectionKeyCode={null}
      connectOnClick={false} onConnect={connect}
      onPaneClick={() => { setPending(null); setNotice(""); }}
      onDragOver={event => {
        if (event.dataTransfer.types.includes(dragFormat)) { event.preventDefault(); event.dataTransfer.dropEffect = "copy"; }
      }} onDrop={drop}
      aria-label={t("nodes.canvas")} ariaLabelConfig={{
        "node.a11yDescription.default": t("nodes.nodeKeyboard"), "edge.a11yDescription.default": t("nodes.edgeKeyboard"),
        "node.a11yDescription.ariaLiveMessage": ({ x, y }) => t("nodes.nodeMoved", { x, y }),
      }}>
      <Background variant={BackgroundVariant.Dots} gap={20} size={1.2} color="var(--color-control-border)" />
      <Panel position="top-left" className="workflow-toolbar workflow-palette">
        <div role="group" aria-label={t("nodes.add")}>
          <span className="workflow-add-label"><Plus size={16} />{t("nodes.add")}</span>
          {kinds.map(kind => {
            const Icon = nodeIcons[kind];
            const label = t(`nodes.types.${kind}`);
            return <button key={kind} type="button" draggable aria-label={label} title={label}
              onDragStart={event => { event.dataTransfer.setData(dragFormat, kind); event.dataTransfer.effectAllowed = "copy"; }}
              onClick={() => add(kind)}><Icon size={16} /><span>{label}</span></button>;
          })}
        </div>
      </Panel>
      <ZoomControls />
      {selected && <Panel position="bottom-center" className="workflow-toolbar workflow-edge-actions">
        <button type="button" disabled={busyIds.has(selected.source) || busyIds.has(selected.target)}
          onClick={() => disconnect(selected.id)}><Unplug size={15} />{t("nodes.disconnect")}</button>
      </Panel>}
      {graph.nodes.length === 0 && <p className="workflow-empty">{t("nodes.empty")}</p>}
      {message && <div className="workflow-notice" role={notice ? "alert" : "status"}>
        <span>{message}</span>
        <button className="workflow-icon-button" aria-label={t(pending ? "nodes.cancelConnection" : "nodes.close")}
          onClick={() => { setPending(null); setNotice(""); }}><X size={14} /></button>
      </div>}
    </ReactFlow>
    {preview && createPortal(<div className="workflow-preview-backdrop bg-black/65" onClick={() => setPreview("")}>
      <div ref={dialog} className="workflow-preview-dialog" role="dialog" aria-modal="true"
        aria-label={t("nodes.result")} onClick={event => event.stopPropagation()}>
        <button className="workflow-icon-button" aria-label={t("nodes.close")} onClick={() => setPreview("")}><X size={19} /></button>
        <WorkflowImage key={preview} path={preview} alt={t("nodes.result")} />
      </div>
    </div>, document.body)}
  </div>;
}

export default function NodeCanvas(props: Props) {
  return <ReactFlowProvider><Canvas {...props} /></ReactFlowProvider>;
}
