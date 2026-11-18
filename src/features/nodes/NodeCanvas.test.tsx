import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { useState } from "react";
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import NodeCanvas from "./NodeCanvas";
import { makeNode, type WorkflowGraph } from "./graph";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string, args?: Record<string, string>) => args?.name ? key + "/" + args.name : key }),
}));

beforeAll(() => {
  vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} unobserve() {} });
});
afterEach(cleanup);
afterAll(() => vi.unstubAllGlobals());

function renderCanvas(busy = false, resultPath?: string) {
  const prompt = makeNode("text", 0, 0);
  const target = { ...makeNode("generate", 400, 0), resultPath };
  let current: WorkflowGraph = { version: 1, nodes: [prompt, target], edges: [] };
  const run = vi.fn();
  function Harness() {
    const [graph, setGraph] = useState(current);
    return <NodeCanvas graph={graph} busyIds={new Set(busy ? [target.id] : [])}
      onChange={next => { current = next; setGraph(next); }} onPick={vi.fn()} onRun={run} />;
  }
  const view = render(<Harness />);
  return { ...view, getGraph: () => current, prompt, target, run };
}

describe("node canvas", () => {
  it("connects a prompt with the keyboard and deletes its incident connections", () => {
    const { container, getGraph, prompt, target } = renderCanvas();
    const cards = container.querySelectorAll<HTMLElement>(".react-flow__node");
    const sourcePort = within(cards[0]).getByLabelText(/nodes.connectOutput/);
    const inputPort = within(cards[1]).getByLabelText(/nodes.connectInput/);
    fireEvent.keyDown(sourcePort, { key: "Enter" });
    fireEvent.keyDown(inputPort, { key: " " });
    expect(getGraph().edges).toEqual([{ source: prompt.id, target: target.id }]);
    fireEvent.change(within(cards[0]).getByRole("textbox", { hidden: true }), {target: {value: "Midnight blue"}});
    expect(getGraph().nodes[0].text).toBe("Midnight blue");
    fireEvent.click(within(cards[0]).getByLabelText("nodes.remove"));
    expect(getGraph().edges).toEqual([]);
    expect(getGraph().nodes.map(node => node.id)).toEqual([target.id]);
  });

  it("disables generation and editing on a busy node", () => {
    const { container, getGraph, run } = renderCanvas(true);
    const target = container.querySelectorAll<HTMLElement>(".react-flow__node")[1];
    expect(within(target).getByLabelText("nodes.remove")).toBeDisabled();
    expect(within(target).getByLabelText("nodes.model")).toBeDisabled();
    fireEvent.click(within(target).getByText("nodes.run"));
    expect(run).not.toHaveBeenCalled();
    expect(getGraph().nodes).toHaveLength(2);
  });

  it("starts a variation from a result with an editable prompt and a connected generator", () => {
    const { getGraph, target } = renderCanvas(false, "/output.png");
    fireEvent.click(screen.getByText("nodes.variation"));
    const [prompt, generator] = getGraph().nodes.slice(2);
    expect(prompt).toMatchObject({ kind: "text", text: "" });
    expect(generator).toMatchObject({ kind: "generate", model: target.model });
    expect(getGraph().edges).toEqual([{ source: target.id, target: generator.id }, { source: prompt.id, target: generator.id }]);
  });

  it("adds a reference image node from the palette", () => {
    const { getGraph } = renderCanvas();
    fireEvent.click(within(screen.getByRole("group", { name: "nodes.add" })).getByRole("button", { name: "nodes.types.reference" }));
    expect(getGraph().nodes.map(node => node.kind)).toEqual(["text", "generate", "reference"]);
  });
});
