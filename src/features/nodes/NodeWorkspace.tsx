import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { ROUTES } from "../../app/routes/contracts";
import { useLayoutContext } from "../../components/layout/AppLayout";
import { enqueueGeneration, hasTauriRuntime, importCanvasAsset, pickSourceImages } from "../../lib/api";
import { generationErrorMessage } from "../../lib/generationMessages";
import { buildEditParams, buildGenerationParams } from "../../lib/generationRequests";
import { defaultParameters } from "../generation/composerParameters";
import { GraphError, initialGraph, loadGraph, resolveInputs, saveGraph, type WorkflowGraph, type WorkflowNode } from "./graph";
import { awaitingReceipt, graphChangeListeners, reconcileNodeJob, updateNodeJob, type GraphChange, type NodeJob } from "./nodeJobs";
import NodeCanvas from "./NodeCanvas";

interface WorkspaceProps {
  scope: string;
  activeProjectId: string | null;
  activeConversationId: string | null;
}

function JobTracker({ scope, project, conversation, nodeId, requestId, jobId, onStorageFailure }: NodeJob & {
  jobId?: string;
  onStorageFailure: () => void;
}) {
  const { t } = useTranslation();
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    const job = { scope, project, conversation, nodeId, requestId };
    async function check() {
      try {
        await reconcileNodeJob(job, jobId, t, () => active);
      } catch {
        if (active) {
          try { updateNodeJob(job, { error: t("nodes.errors.uncertain") }); }
          catch { onStorageFailure(); }
        }
      } finally {
        if (active) timer = setTimeout(check, 2500);
      }
    }
    void check();
    return () => { active = false; clearTimeout(timer); };
  }, [scope, project, conversation, nodeId, requestId, jobId, t, onStorageFailure]);
  return null;
}

export default function NodeWorkspace({ scope, activeProjectId: project, activeConversationId: conversation }: WorkspaceProps) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { setActiveConversationId, refreshConversations } = useLayoutContext();
  const [initial] = useState(() => {
    try { return { graph: loadGraph(scope), failed: false }; }
    catch { return { graph: initialGraph(), failed: true }; }
  });
  const [graph, setGraph] = useState(initial.graph);
  const [storageFailed, setStorageFailed] = useState(initial.failed);
  const liveGraph = useRef(graph);
  const mounted = useRef(false);
  const picking = useRef(new Set<string>());
  const onStorageFailure = useCallback(() => setStorageFailed(true), []);

  useLayoutEffect(() => {
    mounted.current = true;
    function receive(change: GraphChange) {
      if (change.scope !== scope && change.destination !== scope) return;
      liveGraph.current = change.graph;
      setGraph(change.graph);
      setStorageFailed(false);
      if (change.scope === scope && change.destination !== scope && change.conversation) {
        setActiveConversationId(change.conversation);
        if (project) navigate(ROUTES.projectChat(project, change.conversation), { replace: true });
        else navigate(ROUTES.generate, { replace: true, state: { activateConversationId: change.conversation } });
      }
    }
    graphChangeListeners.add(receive);
    return () => { mounted.current = false; graphChangeListeners.delete(receive); };
  }, [scope, project, navigate, setActiveConversationId]);

  function changeGraph(next: WorkflowGraph) {
    try {
      saveGraph(scope, next);
      liveGraph.current = next;
      setGraph(next);
      setStorageFailed(false);
      return true;
    } catch {
      onStorageFailure();
      return false;
    }
  }

  function patchNode(id: string, patch: Partial<WorkflowNode>) {
    const current = liveGraph.current;
    return changeGraph({ ...current, nodes: current.nodes.map(n => n.id === id ? { ...n, ...patch } : n) });
  }

  function detach(id: string) {
    const node = liveGraph.current.nodes.find(n => n.id === id);
    if (node?.clientRequestId && node.error && patchNode(id, { clientRequestId: undefined, jobId: undefined, error: undefined })) {
      awaitingReceipt.delete(node.clientRequestId);
    }
  }

  async function pick(id: string) {
    if (picking.current.has(id)) return;
    picking.current.add(id);
    try {
      const [path] = await pickSourceImages();
      if (!mounted.current || !path || !liveGraph.current.nodes.some(n => n.id === id)) return;
      const savedPath = hasTauriRuntime() ? (await importCanvasAsset(path)).file_path : path;
      if (mounted.current) patchNode(id, { path: savedPath, error: undefined });
    } catch (error) {
      if (mounted.current) patchNode(id, { error: generationErrorMessage(error) || t("nodes.errors.pickFailed") });
    } finally {
      picking.current.delete(id);
    }
  }

  async function run(id: string) {
    const current = liveGraph.current;
    const node = current.nodes.find(n => n.id === id);
    if (node?.kind !== "generate" || node.clientRequestId || (!conversation && current.nodes.some(n => n.clientRequestId))) return;
    let inputs;
    try { inputs = resolveInputs(current, id); }
    catch (error) {
      patchNode(id, { error: t(error instanceof GraphError ? `nodes.errors.${error.code}` : "nodes.errors.generationFailed") });
      return;
    }
    if (!hasTauriRuntime()) {
      patchNode(id, { error: t("nodes.errors.desktopOnly") });
      return;
    }

    const requestId = crypto.randomUUID();
    const request = {
      ...defaultParameters(node.model), prompt: inputs.prompt, model: node.model, clientRequestId: requestId,
      projectId: project, conversationId: conversation,
      editSources: inputs.paths.map(path => ({ id: path, label: path, path })),
    };
    const params = inputs.paths.length ? buildEditParams(request) : buildGenerationParams(request);
    const job = { scope, project, conversation, nodeId: id, requestId };
    if (!patchNode(id, { clientRequestId: requestId, jobId: undefined, error: undefined })) return;
    awaitingReceipt.set(requestId, Date.now());
    try {
      const receipt = await enqueueGeneration(params);
      try { updateNodeJob(job, { jobId: receipt.job_id, error: undefined }, receipt.conversation_id); }
      catch { if (mounted.current) onStorageFailure(); }
      refreshConversations();
    } catch (error) {
      const kind = error && typeof error === "object" && "kind" in error ? error.kind : undefined;
      const providerMissing = kind === "ApiKeyNotSet" || kind === "ProviderProfileNotFound" || kind === "CredentialUnavailable";
      const rejected = providerMissing || kind === "Validation";
      try {
        updateNodeJob(job, {
          clientRequestId: rejected ? undefined : requestId,
          error: providerMissing ? t("nodes.errors.providerSetup") : rejected
            ? generationErrorMessage(error) || t("nodes.errors.generationFailed") : t("nodes.errors.uncertain"),
        });
      } catch { if (mounted.current) onStorageFailure(); }
    } finally { awaitingReceipt.delete(requestId); }
  }

  const pending = graph.nodes.filter(n => n.clientRequestId);
  const busyIds = new Set((!conversation && pending.length ? graph.nodes.filter(n => n.kind === "generate") : pending).map(n => n.id));
  return <div className="relative flex min-h-0 flex-1 flex-col">
    {storageFailed && <p role="alert" className="px-4 py-2 text-sm text-red-600">{t("nodes.errors.saveFailed")}</p>}
    <NodeCanvas graph={graph} busyIds={busyIds} onChange={changeGraph} onRun={run} onPick={pick} onDetach={detach} />
    {pending.map(n => <JobTracker key={n.clientRequestId} scope={scope} project={project} conversation={conversation}
      nodeId={n.id} requestId={n.clientRequestId!} jobId={n.jobId} onStorageFailure={onStorageFailure} />)}
  </div>;
}
