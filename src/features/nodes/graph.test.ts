import { beforeEach, describe, expect, it } from "vitest";
import {
  connectNodes, GraphError, initialGraph, loadGraph, makeNode, removeGraph, resolveInputs, saveGraph,
  type WorkflowGraph, type WorkflowNode,
} from "./graph";

function graphWith(...nodes: WorkflowNode[]): WorkflowGraph {
  return { version: 1, nodes, edges: [] };
}

function expectCode(operation: () => unknown, code: GraphError["code"]) {
  expect(operation).toThrow(expect.objectContaining({ name: "GraphError", code }));
}

describe("workflow connections", () => {
  it("starts with a prompt connected to a generation node", () => {
    const graph = initialGraph();
    const [text, generator] = graph.nodes;
    expect(text.kind).toBe("text");
    expect(generator.kind).toBe("generate");
    expect(generator.x - text.x).toBeGreaterThanOrEqual(300);
    expect(graph.edges).toEqual([{ source: text.id, target: generator.id }]);
    expect(text.id).not.toBe(initialGraph().nodes[0].id);
  });

  it("rejects missing endpoints, duplicate edges, invalid targets and self-connections", () => {
    const graph = initialGraph();
    const [text, generator] = graph.nodes;
    expectCode(() => connectNodes(graph, "missing", generator.id), "missingNode");
    expectCode(() => connectNodes(graph, generator.id, "missing"), "missingNode");
    expectCode(() => connectNodes(graph, generator.id, generator.id), "self");
    expectCode(() => connectNodes(graph, text.id, generator.id), "duplicate");
    expectCode(() => connectNodes(graph, generator.id, text.id), "invalidTarget");
  });

  it("allows branching and merging but rejects indirect cycles", () => {
    const [a, b, c, d] = Array.from({ length: 4 }, () => makeNode("generate", 0, 0));
    const start = graphWith(a, b, c, d);
    let graph = start;
    for (const [source, target] of [[a, b], [a, c], [b, d], [c, d]]) {
      graph = connectNodes(graph, source.id, target.id);
    }
    expect(start.edges).toEqual([]);
    expect(graph.edges).toHaveLength(4);
    expectCode(() => connectNodes(graph, d.id, a.id), "cycle");
  });
});

describe("generation inputs", () => {
  it("combines connected nonempty prompts, references and previous results in connection order", () => {
    const a = { ...makeNode("text", 0, 0), text: " mist " };
    const blank = makeNode("text", 0, 0);
    const b = { ...a, id: "b", text: "soft light" };
    const ref = { ...makeNode("reference", 0, 0), path: "/外部/image.jpg" };
    const prev = { ...makeNode("generate", 0, 0), resultPath: "/library/result.png" };
    const target = makeNode("generate", 0, 0);
    let graph = graphWith(a, b, ref, prev, target, blank, { ...a, id: "unconnected" });
    for (const node of [prev, b, ref, blank, a]) graph = connectNodes(graph, node.id, target.id);
    expect(resolveInputs(graph, target.id)).toEqual({ prompt: "soft light\n\nmist", paths: [prev.resultPath, ref.path] });
  });

  it("requires a nonempty prompt and an image from each connected image source", () => {
    let graph = initialGraph();
    const [text, target] = graph.nodes;
    text.text = "  ";
    expectCode(() => resolveInputs(graph, target.id), "missingPrompt");
    text.text = "landscape";
    for (const kind of ["reference", "generate"] as const) {
      const empty = makeNode(kind, 0, 0);
      const attached = connectNodes({ ...graph, nodes: [...graph.nodes, empty] }, empty.id, target.id);
      expectCode(() => resolveInputs(attached, target.id), "missingImage");
    }
    expectCode(() => resolveInputs(graph, text.id), "invalidTarget");
    expectCode(() => resolveInputs(graph, "missing"), "missingNode");
  });

  it("uses the target model's reference limit", () => {
    let graph = initialGraph();
    const [text, target] = graph.nodes;
    text.text = "compose";
    target.model = "grok-imagine-image-2.0";
    for (let index = 0; index < 6; index++) {
      const ref = { ...makeNode("reference", 0, 0), path: String(index) };
      graph = connectNodes({ ...graph, nodes: [...graph.nodes, ref] }, ref.id, target.id);
    }
    expectCode(() => resolveInputs(graph, target.id), "tooManyImages");
    target.model = "gpt-image-2";
    expect(resolveInputs(graph, target.id).paths).toHaveLength(6);
  });
});

describe("scoped workflow persistence", () => {
  beforeEach(() => localStorage.clear());

  it("preserves job identity and user-deleted nodes across reloads and isolates scopes", () => {
    const graph = initialGraph();
    Object.assign(graph.nodes[1], { x: -28, jobId: "job-1", clientRequestId: "attempt", resultPath: "/out.png", error: "timeout" });
    saveGraph("chat/1", graph);
    saveGraph("chat/10", graphWith());
    expect(loadGraph("chat/1")).toEqual(graph);
    expect(loadGraph("chat/10")).toEqual(graphWith());
    removeGraph("chat/1");
    expect(loadGraph("chat/1")).not.toEqual(graph);
    expect(loadGraph("chat/10")).toEqual(graphWith());
  });

  it.each([
    ["invalid JSON", () => "{"],
    ["unknown version", (graph: WorkflowGraph) => JSON.stringify({ ...graph, version: 2 })],
    ["invalid node", (graph: WorkflowGraph) => JSON.stringify({ ...graph, nodes: [null] })],
    ["duplicate node", (graph: WorkflowGraph) => JSON.stringify({ ...graph, nodes: [...graph.nodes, graph.nodes[0]] })],
    ["invalid model", (graph: WorkflowGraph) => { graph.nodes[1].model = "unknown" as never; return JSON.stringify(graph); }],
    ["invalid coordinates", (graph: WorkflowGraph) => { graph.nodes[0].x = Infinity; return JSON.stringify(graph); }],
    ["invalid job id", (graph: WorkflowGraph) => { graph.nodes[1].jobId = 2 as never; return JSON.stringify(graph); }],
    ["dangling edge", (graph: WorkflowGraph) => { graph.edges[0].source = "missing"; return JSON.stringify(graph); }],
    ["cycle", (graph: WorkflowGraph) => { graph.nodes[0].kind = "generate"; graph.edges.push({ source: graph.nodes[1].id, target: graph.nodes[0].id }); return JSON.stringify(graph); }],
  ])("opens a usable starter for %s", (_, corrupt) => {
    const graph = initialGraph();
    saveGraph("one", graph);
    const key = localStorage.key(0)!;
    localStorage.setItem(key, corrupt(graph));
    const restored = loadGraph("one");
    expect(restored.nodes.map(node => node.kind)).toEqual(["text", "generate"]);
    expect(restored.edges).toEqual([{ source: restored.nodes[0].id, target: restored.nodes[1].id }]);
    expect(restored.nodes[0].id).not.toBe(graph.nodes[0].id);
  });
});
