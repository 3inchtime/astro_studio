import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { conversationDraftKey } from "../../lib/providerSettingsDraft";
import { initialGraph, loadGraph, makeNode, saveGraph, type WorkflowGraph } from "./graph";
import NodeWorkspace from "./NodeWorkspace";

const api = vi.hoisted(() => ({
  enqueueGeneration: vi.fn(), onGenerationJob: vi.fn(), getGenerationJob: vi.fn(), listGenerationJobs: vi.fn(),
  waitGenerationResult: vi.fn(), hasTauriRuntime: vi.fn(() => true), pickSourceImages: vi.fn(), importCanvasAsset: vi.fn(),
}));
const recipe = vi.hoisted(() => vi.fn());
const layout = vi.hoisted(() => ({ setActiveConversationId: vi.fn(), refreshConversations: vi.fn() }));
vi.mock("../../lib/api", () => api);
vi.mock("../../lib/recipeApi", () => ({ getGenerationRecipeForRequest: recipe }));
vi.mock("../../components/layout/AppLayout", () => ({ useLayoutContext: () => layout }));
vi.mock("./NodeCanvas", () => ({
  default: ({ graph, onRun, busyIds, onChange, onDetach, onPick }: {
    graph: WorkflowGraph; onRun: (id: string) => void; busyIds: ReadonlySet<string>;
    onChange: (g: WorkflowGraph) => void; onDetach: (id: string) => void; onPick: (id: string) => void;
  }) => <>
    <output>{JSON.stringify(graph)}</output>
    {graph.nodes.filter(n => n.kind === "reference").map(n => <button key={n.id} onClick={() => onPick(n.id)}>pick</button>)}
    {graph.nodes.filter(n => n.error && n.clientRequestId).map(n => <button key={n.id} onClick={() => onDetach(n.id)}>detach</button>)}
    <button onClick={() => onChange({ ...graph, nodes: graph.nodes.map(n => n.kind === "text" ? { ...n, text: "later edit" } : n) })}>edit</button>
    {graph.nodes.filter(n => n.kind === "generate").map(n =>
      <button key={n.id} data-busy={busyIds.has(n.id)} onClick={() => onRun(n.id)}>{n.id}</button>)}
  </>,
}));

function RoutePath() { return <p>{useLocation().pathname}</p>; }
function mount(conversation: string | null, project: string | null = "project", pathname = "/projects/project/chat") {
  return render(<MemoryRouter initialEntries={[pathname]}>
    <NodeWorkspace scope={conversationDraftKey(project, conversation)} activeProjectId={project} activeConversationId={conversation} />
    <RoutePath />
  </MemoryRouter>);
}

function seed(conversation: string | null, withRef = false) {
  const graph = initialGraph();
  graph.nodes[0].text = "a lighthouse";
  const generator = graph.nodes[1];
  if (withRef) {
    const reference = { ...makeNode("reference", 100, 200), path: "/tmp/source.png" };
    graph.nodes.push(reference);
    graph.edges.push({ source: reference.id, target: generator.id });
  }
  const scope = conversationDraftKey("project", conversation);
  saveGraph(scope, graph);
  return { graph, id: generator.id, scope };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}
const receipt = { conversation_id: "accepted", job_id: "job", status: "queued" };
function completed(client: string) {
  return { id: "job", client_request_id: client, status: "completed", generation_id: "generation",
    request: { project_id: "project", conversation_id: "accepted" } };
}

describe("node workspace generation", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    localStorage.clear();
    sessionStorage.clear();
    api.hasTauriRuntime.mockReturnValue(true);
    api.onGenerationJob.mockResolvedValue(() => {});
    api.getGenerationJob.mockImplementation(() => new Promise(() => {}));
    recipe.mockResolvedValue(null);
  });

  it("records one request before submitting an edit and moves intervening edits into the first acknowledged project chat", async () => {
    const { scope, id } = seed(null, true);
    const ack = deferred<typeof receipt>();
    api.enqueueGeneration.mockImplementation(() => {
      expect(loadGraph(scope).nodes[1].clientRequestId).toBeTruthy();
      return ack.promise;
    });
    mount(null);
    fireEvent.click(screen.getByText(id));
    fireEvent.click(screen.getByText(id));
    fireEvent.click(screen.getByText("edit"));
    expect(api.enqueueGeneration).toHaveBeenCalledTimes(1);
    expect(api.enqueueGeneration).toHaveBeenCalledWith(expect.objectContaining({
      prompt: "a lighthouse", projectId: "project", conversationId: null,
      sourceImagePaths: ["/tmp/source.png"], clientRequestId: expect.any(String),
    }));
    await act(async () => ack.resolve(receipt));
    expect(screen.getByText("/projects/project/chat/accepted")).toBeVisible();
    const promoted = loadGraph(conversationDraftKey("project", "accepted"));
    expect(promoted.nodes).toHaveLength(3);
    expect(promoted.nodes[1].jobId).toBe("job");
    expect(promoted.nodes[0].text).toBe("later edit");
    expect(loadGraph(scope).nodes.some(n => n.id === id)).toBe(false);
  });

  it("preserves a late acknowledgment after leaving and recovers the original job from the assigned conversation", async () => {
    const { id, scope } = seed(null);
    const ack = deferred<typeof receipt>();
    api.enqueueGeneration.mockReturnValue(ack.promise);
    const view = mount(null);
    fireEvent.click(screen.getByText(id));
    const client = loadGraph(scope).nodes[1].clientRequestId!;
    expect(api.enqueueGeneration.mock.calls[0][0]).not.toHaveProperty("sourceImagePaths");
    view.unmount();
    await act(async () => ack.resolve(receipt));
    expect(layout.setActiveConversationId).not.toHaveBeenCalled();
    api.getGenerationJob.mockResolvedValue(completed(client));
    api.waitGenerationResult.mockResolvedValue({ images: [{ file_path: "/tmp/result.png" }] });
    mount("accepted");
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("/tmp/result.png"));
    expect(api.enqueueGeneration).toHaveBeenCalledTimes(1);
    expect(screen.getByText(id)).toHaveAttribute("data-busy", "false");
  });

  it("merges a delayed acknowledgment with edits already saved in the assigned conversation", async () => {
    const { id, graph } = seed(null);
    const ack = deferred<typeof receipt>();
    api.enqueueGeneration.mockReturnValue(ack.promise);
    const first = mount(null);
    fireEvent.click(screen.getByText(id));
    first.unmount();
    const destination = conversationDraftKey("project", "accepted");
    const edited = initialGraph();
    edited.nodes[0].text = "existing destination prompt";
    // A shared node may also have been moved in the assigned chat.
    edited.nodes.push({ ...graph.nodes[1], x: 920 });
    edited.edges.push({ source: edited.nodes[0].id, target: id });
    saveGraph(destination, edited);
    mount("accepted");
    await act(async () => ack.resolve(receipt));
    const merged = loadGraph(destination);
    expect(merged.nodes).toHaveLength(4);
    expect(merged.nodes).toContainEqual(edited.nodes[0]);
    expect(merged.nodes).toContainEqual(edited.nodes[1]);
    expect(merged.nodes).toContainEqual(graph.nodes[0]);
    expect(merged.nodes.find(n => n.id === id)).toMatchObject({ jobId: "job", clientRequestId: expect.any(String), x: 920 });
    expect(merged.edges).toHaveLength(3);
    expect(merged.edges).toEqual(expect.arrayContaining([...graph.edges, ...edited.edges]));
    expect(screen.getByRole("status")).toHaveTextContent("existing destination prompt");
    expect(layout.setActiveConversationId).not.toHaveBeenCalled();
  });

  it("reconciles a lost acknowledgment by its client request without enqueuing another job", async () => {
    const { scope, id } = seed(null);
    api.enqueueGeneration.mockRejectedValue(new Error("timeout"));
    const view = mount(null);
    fireEvent.click(screen.getByText(id));
    await waitFor(() => expect(loadGraph(scope).nodes[1].error).toBeTruthy());
    fireEvent.click(screen.getByText(id));
    const client = loadGraph(scope).nodes[1].clientRequestId!;
    expect(client).toBeTruthy();
    view.unmount();
    recipe.mockResolvedValue({ generation_id: "generation" });
    api.listGenerationJobs.mockResolvedValue({ items: [completed(client)] });
    mount(null);
    await waitFor(() => expect(layout.setActiveConversationId).toHaveBeenCalledWith("accepted"));
    expect(api.enqueueGeneration).toHaveBeenCalledTimes(1);
  });

  it("rechecks immediately after a matching terminal event received during the first read", async () => {
    const { graph, scope } = seed("accepted");
    Object.assign(graph.nodes[1], { clientRequestId: "client", jobId: "job" });
    saveGraph(scope, graph);
    const firstRead = deferred<ReturnType<typeof completed>>();
    api.getGenerationJob.mockReturnValueOnce(firstRead.promise).mockResolvedValue(completed("client"));
    api.waitGenerationResult.mockResolvedValue({ images: [{ file_path: "/tmp/variation.png" }] });
    mount("accepted");
    await act(async () => {});
    expect(api.getGenerationJob).toHaveBeenCalledTimes(1);
    expect(api.onGenerationJob).toHaveBeenCalledOnce();
    const notify = api.onGenerationJob.mock.calls[0][0];
    const terminal = { client_request_id: "client", job_id: "job", status: "completed" };
    await act(async () => {
      notify(terminal);
      notify(terminal);
      expect(api.getGenerationJob).toHaveBeenCalledTimes(1);
      firstRead.resolve({ ...completed("client"), status: "running" });
    });
    expect(api.getGenerationJob).toHaveBeenCalledTimes(2);
    expect(loadGraph(scope).nodes[1].resultPath).toBe("/tmp/variation.png");
    expect(api.enqueueGeneration).not.toHaveBeenCalled();
  });

  it("ignores other requests, job IDs and active statuses while waiting for its own terminal event", async () => {
    const { graph, scope } = seed("accepted");
    Object.assign(graph.nodes[1], { clientRequestId: "client", jobId: "job" });
    saveGraph(scope, graph);
    api.getGenerationJob.mockResolvedValue({ ...completed("client"), status: "running" });
    mount("accepted");
    await act(async () => {});
    expect(api.onGenerationJob).toHaveBeenCalledOnce();
    const notify = api.onGenerationJob.mock.calls[0][0];
    await act(async () => {
      notify({ client_request_id: "other", job_id: "job", status: "completed" });
      notify({ client_request_id: "client", job_id: "other", status: "completed" });
      notify({ client_request_id: "client", job_id: "job", status: "running" });
    });
    expect(api.getGenerationJob).toHaveBeenCalledTimes(1);
    api.getGenerationJob.mockResolvedValue({ ...completed("client"), status: "failed", error_message: "fixture error" });
    await act(async () => notify({ client_request_id: "client", job_id: "job", status: "failed" }));
    expect(loadGraph(scope).nodes[1].error).toBe("fixture error");
  });

  it("matches the client request before a job ID has been recovered", async () => {
    const { graph, scope } = seed("accepted");
    graph.nodes[1].clientRequestId = "lost-receipt";
    saveGraph(scope, graph);
    const job = completed("lost-receipt");
    api.getGenerationJob.mockResolvedValue(job);
    api.waitGenerationResult.mockResolvedValue({ images: [{ file_path: "/tmp/recovered.png" }] });
    mount("accepted");
    await act(async () => {});
    recipe.mockResolvedValue({ generation_id: "generation" });
    api.listGenerationJobs.mockResolvedValue({ items: [job] });
    await act(async () => api.onGenerationJob.mock.calls[0][0]({
      client_request_id: "lost-receipt", job_id: "job", status: "completed",
    }));
    expect(loadGraph(scope).nodes[1].resultPath).toBe("/tmp/recovered.png");
    expect(api.enqueueGeneration).not.toHaveBeenCalled();
  });

  it("continues polling when the desktop event subscription fails", async () => {
    vi.useFakeTimers();
    try {
      const { graph, scope } = seed("accepted");
      Object.assign(graph.nodes[1], { clientRequestId: "client", jobId: "job" });
      saveGraph(scope, graph);
      api.onGenerationJob.mockRejectedValue(new Error("bridge unavailable"));
      api.getGenerationJob.mockResolvedValueOnce({ ...completed("client"), status: "queued" })
        .mockResolvedValue(completed("client"));
      api.waitGenerationResult.mockResolvedValue({ images: [{ file_path: "/tmp/polled.png" }] });
      mount("accepted");
      await act(async () => {});
      expect(api.getGenerationJob).toHaveBeenCalledTimes(1);
      await act(async () => vi.advanceTimersByTimeAsync(2500));
      expect(loadGraph(scope).nodes[1].resultPath).toBe("/tmp/polled.png");
    } finally { vi.useRealTimers(); }
  });

  it("releases a late event subscription and ignores delivery after unmount", async () => {
    const { graph, scope } = seed("accepted");
    Object.assign(graph.nodes[1], { clientRequestId: "client", jobId: "job" });
    saveGraph(scope, graph);
    const release = vi.fn();
    const registration = deferred<typeof release>();
    api.onGenerationJob.mockReturnValue(registration.promise);
    const view = mount("accepted");
    expect(api.onGenerationJob).toHaveBeenCalledOnce();
    const notify = api.onGenerationJob.mock.calls[0][0];
    view.unmount();
    const previousReads = api.getGenerationJob.mock.calls.length;
    await act(async () => {
      registration.resolve(release);
      notify({ client_request_id: "client", job_id: "job", status: "completed" });
    });
    expect(release).toHaveBeenCalledOnce();
    expect(api.getGenerationJob).toHaveBeenCalledTimes(previousReads);
    expect(loadGraph(scope).nodes[1].clientRequestId).toBe("client");
  });

  it("keeps an unknown provider outcome locked across a remount", async () => {
    const { graph, id, scope } = seed("accepted");
    Object.assign(graph.nodes[1], { clientRequestId: "client", jobId: "job" });
    saveGraph(scope, graph);
    api.getGenerationJob.mockResolvedValue({ ...completed("client"), status: "interrupted", error_code: "provider_outcome_unknown" });
    mount("accepted");
    await waitFor(() => expect(loadGraph(scope).nodes[1].error).toBeTruthy());
    fireEvent.click(screen.getByText(id));
    expect(api.enqueueGeneration).not.toHaveBeenCalled();
    expect(loadGraph(scope).nodes[1].clientRequestId).toBe("client");
  });

  it("stops tracking an uncertain request only when asked, leaving the queue untouched", async () => {
    const { graph, scope } = seed("accepted");
    Object.assign(graph.nodes[1], { clientRequestId: "unconfirmed" });
    saveGraph(scope, graph);
    mount("accepted");
    fireEvent.click(await screen.findByText("detach"));
    expect(loadGraph(scope).nodes[1].clientRequestId).toBeUndefined();
    expect(api.enqueueGeneration).not.toHaveBeenCalled();
  });

  it("retains a previous successful output if a new request is rejected", async () => {
    const { graph, scope, id } = seed("accepted");
    graph.nodes[1].resultPath = "/tmp/previous-result.png";
    saveGraph(scope, graph);
    api.enqueueGeneration.mockRejectedValue({ kind: "ApiKeyNotSet", detail: { model: graph.nodes[1].model } });
    mount("accepted");
    fireEvent.click(screen.getByText(id));
    await waitFor(() => expect(loadGraph(scope).nodes[1].error).toBeTruthy());
    expect(loadGraph(scope).nodes[1].resultPath).toBe("/tmp/previous-result.png");
    expect(loadGraph(scope).nodes[1].clientRequestId).toBeUndefined();
  });

  it("includes edits made on a second mount of the same scope while IPC is pending", async () => {
    const { id } = seed(null);
    const ack = deferred<typeof receipt>();
    api.enqueueGeneration.mockReturnValue(ack.promise);
    const first = mount(null);
    fireEvent.click(screen.getByText(id));
    first.unmount();
    mount(null);
    fireEvent.click(screen.getByText("edit"));
    await act(async () => ack.resolve(receipt));
    expect(loadGraph(conversationDraftKey("project", "accepted")).nodes[0].text).toBe("later edit");
    expect(layout.setActiveConversationId).toHaveBeenCalledWith("accepted");
  });

  it("updates the actual route for the default project when opened from its project page", async () => {
    const { graph, id } = seed(null);
    saveGraph(conversationDraftKey("default", null), graph);
    api.enqueueGeneration.mockResolvedValue(receipt);
    mount(null, "default", "/projects/default/chat");
    fireEvent.click(screen.getByText(id));
    expect(await screen.findByText("/projects/default/chat/accepted")).toBeVisible();
  });

  it("persists the picked reference and reports unavailable desktop generation without submitting", async () => {
    const { scope, id } = seed("accepted", true);
    api.hasTauriRuntime.mockReturnValue(false);
    api.pickSourceImages.mockResolvedValue(["/tmp/new.png"]);
    mount("accepted");
    fireEvent.click(screen.getByText("pick"));
    await waitFor(() => expect(loadGraph(scope).nodes[2].path).toBe("/tmp/new.png"));
    fireEvent.click(screen.getByText(id));
    expect(loadGraph(scope).nodes[1].error).toBeTruthy();
    expect(loadGraph(scope).nodes[1].clientRequestId).toBeUndefined();
    expect(api.enqueueGeneration).not.toHaveBeenCalled();
  });

  it("imports a selected reference once and submits the saved managed path after reopening", async () => {
    const { scope, id } = seed("accepted", true);
    const selection = deferred<string[]>();
    api.pickSourceImages.mockReturnValue(selection.promise);
    const imported = "/library/canvas/assets/local.png";
    api.importCanvasAsset.mockResolvedValue({ id: "local", file_path: imported });
    const first = mount("accepted");
    fireEvent.click(screen.getByText("pick"));
    fireEvent.click(screen.getByText("pick"));
    expect(api.pickSourceImages).toHaveBeenCalledTimes(1);
    await act(async () => selection.resolve(["/Downloads/reference.png"]));
    expect(api.importCanvasAsset).toHaveBeenCalledWith("/Downloads/reference.png");
    expect(loadGraph(scope).nodes[2].path).toBe(imported);
    first.unmount();
    api.enqueueGeneration.mockImplementation(() => new Promise(() => {}));
    mount("accepted");
    fireEvent.click(screen.getByText(id));
    expect(api.enqueueGeneration).toHaveBeenCalledWith(expect.objectContaining({ sourceImagePaths: [imported] }));
    expect(api.pickSourceImages).toHaveBeenCalledTimes(1);
  });

  it("keeps the previous reference when a native import fails", async () => {
    const { scope } = seed("accepted", true);
    api.pickSourceImages.mockResolvedValue(["/Downloads/invalid.png"]);
    api.importCanvasAsset.mockRejectedValue(new Error("Invalid image data"));
    mount("accepted");
    fireEvent.click(screen.getByText("pick"));
    await waitFor(() => expect(loadGraph(scope).nodes[2].error).toBeTruthy());
    expect(loadGraph(scope).nodes[2].path).toBe("/tmp/source.png");
  });

  it("prevents submission if the pending request cannot be saved", async () => {
    const { id } = seed("accepted");
    mount("accepted");
    const storage = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw Error("quota"); });
    try {
      fireEvent.click(screen.getByText(id));
      expect(await screen.findByRole("alert")).toBeVisible();
      expect(api.enqueueGeneration).not.toHaveBeenCalled();
    } finally { storage.mockRestore(); }
  });
});
