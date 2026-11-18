import {
  Background, BackgroundVariant, Panel, ReactFlow, ReactFlowProvider, useReactFlow,
  type Connection, type Edge, type NodeChange, type Viewport, type XYPosition,
} from "@xyflow/react";
import { Unplug, X } from "lucide-react";
import { useEffect, useRef, useState, type DragEvent, type RefObject } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import { useDialogFocus } from "../../hooks/useDialogFocus";
import { connectNodes, GraphError, makeNode, type NodeKind, type WorkflowGraph, type WorkflowNode } from "./graph";
import WorkflowCard, { WorkflowImage, type CardNode, type Port } from "./WorkflowCard";
import NodeToolbar, { dragFormat, fitOptions, kinds } from "./NodeToolbar";
import "@xyflow/react/dist/style.css";
import "./nodes.css";

type Props = {
  viewportRef?: RefObject<Viewport | null>;
  inputErrors?: ReadonlyMap<string, string>;
  graph: WorkflowGraph;
  onChange: (graph: WorkflowGraph) => boolean | void;
  onRun: (id: string) => void;
  onPick: (id: string) => void;
  onDetach?: (id: string) => void;
  busyIds: ReadonlySet<string>;
};

type Layout = Pick<CardNode, "measured" | "selected"> & { position?: XYPosition };
const nodeTypes = { workflow: WorkflowCard };
const edgeId = (edge: Pick<Edge, "source" | "target">) => JSON.stringify([edge.source, edge.target]);

function Canvas({ viewportRef, inputErrors, graph, onChange, onRun, onPick, onDetach, busyIds }: Props) {
  const { t } = useTranslation();
  const { screenToFlowPosition, getZoom, setCenter } = useReactFlow();
  const [initialViewport] = useState(() => viewportRef?.current ?? undefined);
  const container = useRef<HTMLDivElement>(null);
  const current = useRef(graph);
  current.current = graph;
  const [layout, setLayout] = useState<Record<string, Layout>>({});
  const undoButton = useRef<HTMLButtonElement>(null);
  const [removed, setRemoved] = useState<{
    node: WorkflowNode;
    index: number;
    links: { edge: WorkflowGraph["edges"][number]; index: number }[];
  } | null>(null);
  const undoAvailable = removed !== null && removed.links.every(({ edge }) =>
    !busyIds.has(edge.source) && !busyIds.has(edge.target));
  const nodeToFocus = useRef<string | null>(null);
  const [selectedEdge, setSelectedEdge] = useState<string | null>(null);
  const [pending, setPending] = useState<Port | null>(null);
  const [notice, setNotice] = useState("");
  const [preview, setPreview] = useState("");
  const dialog = useDialogFocus({ open: Boolean(preview), onClose: () => setPreview("") });

  function commit(next: WorkflowGraph) {
    if (onChange(next) === false) return false;
    current.current = next;
    return true;
  }

  function update(id: string, edit: Partial<WorkflowNode>) {
    if (busyIds.has(id)) return;
    const next = current.current;
    commit({ ...next, nodes: next.nodes.map(node => node.id === id ? { ...node, ...edit } : node) });
  }

  const protectedNodes = new Set(busyIds);
  if (busyIds.size) {
    for (const { source, target } of graph.edges) {
      if (busyIds.has(target)) protectedNodes.add(source);
      if (busyIds.has(source)) protectedNodes.add(target);
    }
  }

  function remove(id: string) {
    if (protectedNodes.has(id)) return;
    const next = current.current;
    const index = next.nodes.findIndex(node => node.id === id);
    if (index === -1) return;
    const links = next.edges.flatMap((edge, index) => edge.source === id || edge.target === id ? [{ edge, index }] : []);
    if (!commit({ ...next, nodes: next.nodes.filter(node => node.id !== id),
      edges: next.edges.filter(edge => edge.source !== id && edge.target !== id) })) return;
    setRemoved({ node: next.nodes[index], index, links });
    if (pending?.id === id) setPending(null);
    setLayout(previous => {
      const clean = { ...previous };
      delete clean[id];
      return clean;
    });
  }

  useEffect(() => {
    if (removed) undoButton.current?.focus({ preventScroll: true });
  }, [removed]);

  function undoRemoval() {
    if (!removed || !undoAvailable) return;
    const next = current.current;
    if (next.nodes.some(node => node.id === removed.node.id)) return;
    const nodes = [...next.nodes];
    nodes.splice(removed.index, 0, removed.node);
    // Topology edits retire this undo. Keep newer text, positions and completed jobs.
    const edges = [...next.edges];
    for (const { edge, index } of removed.links) edges.splice(index, 0, edge);
    if (!commit({ ...next, nodes, edges })) return;
    setRemoved(null);
    selectNewNode(removed.node);
  }

  function disconnect(id: string) {
    const next = current.current;
    const edge = next.edges.find(edge => edgeId(edge) === id);
    if (!edge || busyIds.has(edge.source) || busyIds.has(edge.target)) return;
    if (!commit({ ...next, edges: next.edges.filter(other => other !== edge) })) return;
    setRemoved(null);
    setSelectedEdge(previous => previous === id ? null : previous);
  }

  function connect({ source, target }: Pick<Connection, "source" | "target">) {
    setPending(null);
    if (busyIds.has(source) || busyIds.has(target)) return;
    try {
      if (!commit(connectNodes(current.current, source, target))) return;
      setRemoved(null);
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
    const id = nodeToFocus.current;
    if (!id || !layout[id]?.measured) return;
    const card = Array.from(container.current?.querySelectorAll<HTMLElement>(".react-flow__node") ?? [])
      .find(element => element.dataset.id === id);
    const control = card?.querySelector("textarea") ?? card;
    if (control) { control.focus({ preventScroll: true }); nodeToFocus.current = null; }
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
    if (!commit(connectNodes(connectNodes(branch, source.id, generator.id), prompt.id, generator.id))) return;
    setRemoved(null);
    selectNewNode(prompt);
    void setCenter(prompt.x + 136, prompt.y + 105, { zoom: Math.max(getZoom(), 0.75) });
  }

  function selectNewNode(node: WorkflowNode) {
    nodeToFocus.current = node.id;
    setLayout(previous => ({
      ...Object.fromEntries(Object.entries(previous).map(([key, value]) => [key, { ...value, selected: false }])),
      [node.id]: { selected: true },
    }));
    setSelectedEdge(null);
  }

  function add(kind: NodeKind, point?: XYPosition) {
    const rect = container.current?.getBoundingClientRect();
    if (!rect) return;
    const next = current.current;
    const center = screenToFlowPosition({ x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 });
    const position = point ?? openSpace(center.x - 136, center.y - 96, next.nodes);
    const node = makeNode(kind, position.x, position.y);
    if (!commit({ ...next, nodes: [...next.nodes, node] })) return;
    setRemoved(null);
    selectNewNode(node);
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
    data: { node, inputError: inputErrors?.get(node.id), busy: busyIds.has(node.id), removable: !protectedNodes.has(node.id), pending, update, remove,
      port: selectPort, run: onRun, pick: onPick, preview: setPreview, variation, detach: onDetach },
  }));
  const titles = new Map(nodes.map(node => [node.id, node.ariaLabel]));
  const edges = graph.edges.map(edge => ({
    ...edge, id: edgeId(edge), selected: selectedEdge === edgeId(edge), interactionWidth: 22,
    ariaLabel: t("nodes.edgeLabel", { source: titles.get(edge.source), target: titles.get(edge.target) }),
  }));
  const selected = edges.find(edge => edge.selected);
  const message = notice || (pending ? t(pending.type === "source" ? "nodes.pickInput" : "nodes.pickOutput") : "");

  return <div className="workflow-stage"
    onKeyDown={event => {
      if (undoAvailable && (event.ctrlKey || event.metaKey) && !event.shiftKey && !event.altKey
        && event.key.toLowerCase() === "z" && event.target instanceof Element
        && !event.target.closest("input, textarea, select, [contenteditable=true]")) {
        event.preventDefault();
        undoRemoval();
        return;
      }
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
    <NodeToolbar onAdd={add} onUndo={undoRemoval} undoAvailable={undoAvailable} undoButton={undoButton} />
    <div className="workflow-viewport" ref={container}>
      <ReactFlow<CardNode> nodes={nodes} edges={edges} nodeTypes={nodeTypes} onNodesChange={changeNodes}
        onEdgesChange={changes => {
          for (const change of changes) if (change.type === "select") {
            setSelectedEdge(previous => change.selected ? change.id : previous === change.id ? null : previous);
          }
        }}
        defaultViewport={initialViewport} fitView={!initialViewport}
        onViewportChange={viewport => { if (viewportRef) viewportRef.current = viewport; }}
        fitViewOptions={fitOptions} minZoom={0.2} maxZoom={2} zoomOnDoubleClick={false}
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
    </div>
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
