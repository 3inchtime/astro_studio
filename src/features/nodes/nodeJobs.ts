import type { TFunction } from "i18next";
import { getGenerationJob, listGenerationJobs, waitGenerationResult } from "../../lib/api";
import { getGenerationRecipeForRequest } from "../../lib/recipeApi";
import { conversationDraftKey } from "../../lib/providerSettingsDraft";
import { connectNodes, readSavedGraph, removeGraph, saveGraph, type WorkflowGraph, type WorkflowNode } from "./graph";

export interface NodeJob {
  scope: string;
  project: string | null;
  conversation: string | null;
  nodeId: string;
  requestId: string;
}

export interface GraphChange {
  scope: string;
  destination: string;
  conversation: string | null;
  graph: WorkflowGraph;
}

// A receipt can arrive after switching to the composer, another chat, or a new mount.
export const graphChangeListeners = new Set<(change: GraphChange) => void>();
export const awaitingReceipt = new Map<string, number>();

function mergeAssignedDraft(source: WorkflowGraph, assigned: WorkflowGraph): WorkflowGraph {
  // Keep edits saved in the assigned chat, including moves of a shared node.
  const nodes = new Map(source.nodes.map(node => [node.id, node]));
  for (const node of assigned.nodes) nodes.set(node.id, node);
  let merged = { ...assigned, nodes: Array.from(nodes.values()) };
  for (const edge of source.edges) {
    if (!merged.edges.some(other => other.source === edge.source && other.target === edge.target)) {
      merged = connectNodes(merged, edge.source, edge.target);
    }
  }
  return merged;
}

export function updateNodeJob(job: NodeJob, patch: Partial<WorkflowNode>, conversation = job.conversation) {
  let graph = readSavedGraph(job.scope);
  if (!graph) return;
  const node = graph.nodes.find(n => n.id === job.nodeId && n.clientRequestId === job.requestId);
  if (!node) return;
  const destination = conversation === job.conversation ? job.scope : conversationDraftKey(job.project, conversation);
  if (destination === job.scope) {
    if ((Object.keys(patch) as (keyof WorkflowNode)[]).every(key => node[key] === patch[key])) return;
  } else {
    const assigned = readSavedGraph(destination);
    if (assigned) graph = mergeAssignedDraft(graph, assigned);
  }
  graph.nodes = graph.nodes.map(n => n.id === node.id ? { ...n, clientRequestId: job.requestId, ...patch } : n);
  saveGraph(destination, graph);
  if (destination !== job.scope) removeGraph(job.scope);
  for (const notify of graphChangeListeners) notify({ scope: job.scope, destination, conversation, graph });
}

export async function reconcileNodeJob(job: NodeJob, jobId: string | undefined, t: TFunction, active: () => boolean) {
  let record;
  if (jobId) {
    record = await getGenerationJob(jobId);
  } else {
    const recipe = await getGenerationRecipeForRequest(job.requestId);
    if (recipe && active()) {
      const page = await listGenerationJobs({ generation_id: recipe.generation_id, limit: 1 });
      record = page.items[0];
    }
  }
  if (!active()) return;
  const submittedAt = awaitingReceipt.get(job.requestId);
  if (!record && submittedAt !== undefined && Date.now() - submittedAt < 15_000) return;
  if (!record || record.client_request_id !== job.requestId || record.request.project_id !== (job.project ?? "default")
    || (job.conversation && record.request.conversation_id !== job.conversation)) {
    updateNodeJob(job, { error: t("nodes.errors.uncertain") });
    return;
  }
  if (!jobId || !job.conversation) {
    updateNodeJob(job, { jobId: record.id, error: undefined }, record.request.conversation_id);
    return;
  }
  if (record.status === "queued" || record.status === "running") {
    updateNodeJob(job, { error: undefined });
    return;
  }
  if (record.error_code === "provider_outcome_unknown") {
    updateNodeJob(job, { error: t("nodes.errors.uncertain") });
    return;
  }
  const done = { jobId: undefined, clientRequestId: undefined };
  if (record.status === "completed") {
    const result = await waitGenerationResult(record.id);
    if (!active()) return;
    const path = result.images[0]?.file_path;
    updateNodeJob(job, path ? { ...done, resultPath: path, error: undefined } : { ...done, error: t("nodes.errors.noImage") });
  } else {
    const providerMissing = ["provider_configuration_invalid", "provider_profile_missing"].includes(record.error_code ?? "");
    updateNodeJob(job, { ...done, error: providerMissing ? t("nodes.errors.providerSetup") : record.error_message || t("nodes.errors.generationFailed") });
  }
}
