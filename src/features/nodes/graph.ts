import { DEFAULT_IMAGE_MODEL, findImageModel, getImageModelCatalogEntry } from "../../lib/modelCatalog";
import type { ImageModel } from "../../types";

export type NodeKind = "text" | "reference" | "generate";

export interface WorkflowNode {
  id: string;
  kind: NodeKind;
  x: number;
  y: number;
  text: string;
  path: string;
  model: ImageModel;
  resultPath?: string;
  jobId?: string;
  clientRequestId?: string;
  error?: string;
}

export interface WorkflowGraph {
  version: 1;
  nodes: WorkflowNode[];
  edges: { source: string; target: string }[];
}

export type GraphErrorCode =
  | "missingNode" | "invalidTarget" | "self" | "duplicate" | "cycle"
  | "missingPrompt" | "emptyPrompt" | "missingImage" | "unsupportedImages" | "tooManyImages";

export class GraphError extends Error {
  constructor(public readonly code: GraphErrorCode) {
    super(code);
    this.name = "GraphError";
  }
}

export function makeNode(kind: NodeKind, x: number, y: number): WorkflowNode {
  return { id: crypto.randomUUID(), kind, x, y, text: "", path: "", model: DEFAULT_IMAGE_MODEL };
}

export function initialGraph(): WorkflowGraph {
  const prompt = makeNode("text", 40, 80);
  const generate = makeNode("generate", 370, 120);
  return { version: 1, nodes: [prompt, generate], edges: [{ source: prompt.id, target: generate.id }] };
}

export function connectNodes(graph: WorkflowGraph, source: string, target: string): WorkflowGraph {
  const destination = graph.nodes.find(node => node.id === target);
  if (!destination || !graph.nodes.some(node => node.id === source)) throw new GraphError("missingNode");
  if (source === target) throw new GraphError("self");
  if (destination.kind !== "generate") throw new GraphError("invalidTarget");

  const outgoing = new Map<string, string[]>();
  for (const edge of graph.edges) {
    if (edge.source === source && edge.target === target) throw new GraphError("duplicate");
    const children = outgoing.get(edge.source);
    if (children) children.push(edge.target);
    else outgoing.set(edge.source, [edge.target]);
  }

  // Adding source → target closes a cycle if target can already reach source.
  const pending = [target];
  const seen = new Set<string>();
  while (pending.length) {
    const current = pending.pop()!;
    if (current === source) throw new GraphError("cycle");
    if (seen.has(current)) continue;
    seen.add(current);
    for (const next of outgoing.get(current) ?? []) pending.push(next);
  }
  return { ...graph, edges: [...graph.edges, { source, target }] };
}

export function resolveInputs(graph: WorkflowGraph, target: string): { prompt: string; paths: string[] } {
  const nodes = new Map(graph.nodes.map(node => [node.id, node]));
  const generator = nodes.get(target);
  if (!generator) throw new GraphError("missingNode");
  if (generator.kind !== "generate") throw new GraphError("invalidTarget");

  let hasPromptNode = false;
  const prompts: string[] = [];
  const paths: string[] = [];
  for (const edge of graph.edges) {
    if (edge.target !== target) continue;
    const input = nodes.get(edge.source);
    if (!input) throw new GraphError("missingNode");
    if (input.kind === "text") {
      hasPromptNode = true;
      const text = input.text.trim();
      if (text) prompts.push(text);
    } else {
      const path = input.kind === "reference" ? input.path : input.resultPath;
      if (!path?.trim()) throw new GraphError("missingImage");
      paths.push(path);
    }
  }
  if (!prompts.length) throw new GraphError(hasPromptNode ? "emptyPrompt" : "missingPrompt");
  const model = getImageModelCatalogEntry(generator.model);
  if (paths.length && !model.supportsEdit) throw new GraphError("unsupportedImages");
  if (paths.length > (model.maxReferenceImages ?? 16)) throw new GraphError("tooManyImages");
  return { prompt: prompts.join("\n\n"), paths };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isWorkflowNode(value: unknown): value is WorkflowNode {
  if (!isRecord(value)) return false;
  return typeof value.id === "string" && value.id.trim() !== ""
    && (value.kind === "generate" || value.kind === "text" || value.kind === "reference")
    && Number.isFinite(value.x) && Number.isFinite(value.y)
    && typeof value.text === "string" && typeof value.path === "string"
    && typeof value.model === "string" && findImageModel(value.model)?.id === value.model
    && ["resultPath", "jobId", "clientRequestId", "error"].every(
      key => value[key] === undefined || typeof value[key] === "string",
    );
}

function graphKey(scope: string): string {
  return `astro-studio:nodes:v1:${scope}`;
}

export function readSavedGraph(scope: string): WorkflowGraph | null {
  const stored = localStorage.getItem(graphKey(scope));
  if (stored === null) return null;
  try {
    const data: unknown = JSON.parse(stored);
    if (!isRecord(data) || data.version !== 1 || !Array.isArray(data.nodes)
      || !data.nodes.every(isWorkflowNode) || !Array.isArray(data.edges)
      || new Set(data.nodes.map(node => node.id)).size !== data.nodes.length) return null;

    let graph: WorkflowGraph = { version: 1, nodes: data.nodes, edges: [] };
    for (const edge of data.edges) {
      if (!isRecord(edge) || typeof edge.source !== "string" || typeof edge.target !== "string") return null;
      graph = connectNodes(graph, edge.source, edge.target);
    }
    return graph;
  } catch (error) {
    if (error instanceof SyntaxError || error instanceof GraphError) return null;
    throw error;
  }
}

export function loadGraph(scope: string): WorkflowGraph {
  return readSavedGraph(scope) ?? initialGraph();
}

export function saveGraph(scope: string, graph: WorkflowGraph): void {
  localStorage.setItem(graphKey(scope), JSON.stringify(graph));
}

export function removeGraph(scope: string): void {
  localStorage.removeItem(graphKey(scope));
}
