import { describe, expect, it } from "vitest";
import { selectLoadedGenerations, runGenerationBatch } from "./collectionBatch";

const records = [
  { generation: { id: "a" }, images: [{ id: "a1" }, { id: "a2" }] },
  { generation: { id: "b" }, images: [{ id: "b1" }] },
];

describe("collection batches", () => {
  it("selects only loaded generation records and never hidden IDs", () => {
    expect(selectLoadedGenerations(records, new Set(["a", "hidden"])).map((row) => row.generation.id)).toEqual(["a"]);
  });
  it("continues after failures and retries only unfinished image writes", async () => {
    const checkpoint = new Set<string>();
    const calls: string[] = [];
    let fail = true;
    let active = 0;
    const operation = async (id: string) => {
      expect(active).toBe(0);
      active++;
      calls.push(id);
      await Promise.resolve();
      active--;
      if (id === "a2" && fail) throw new Error("offline");
    };
    const first = await runGenerationBatch(records, "favorite", operation, checkpoint);
    expect(first.succeeded).toEqual(["b"]);
    expect(first.failed).toEqual(["a"]);
    expect(calls).toEqual(["a1", "a2", "b1"]);
    fail = false;
    const remaining = selectLoadedGenerations(records, new Set(first.failed));
    const second = await runGenerationBatch(remaining, "favorite", operation, checkpoint);
    expect(second.succeeded).toEqual(["a"]);
    expect(calls).toEqual(["a1", "a2", "b1", "a2"]);
  });
  it("deletes a multi-image generation exactly once and retains failed generations", async () => {
    const calls: string[] = [];
    const result = await runGenerationBatch(records, "delete", async (id) => {
      calls.push(id);
      if (id === "b") throw new Error("locked");
    }, new Set());
    expect(calls).toEqual(["a", "b"]);
    expect(result).toEqual({ succeeded: ["a"], failed: ["b"] });
  });
});
