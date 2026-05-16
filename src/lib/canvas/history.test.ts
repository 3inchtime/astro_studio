import { describe, expect, it } from "vitest";
import { createHistory, pushHistory, redoHistory, undoHistory } from "./history";

describe("canvas history helpers", () => {
  it("pushes snapshots and clears redo state", () => {
    const initial = createHistory(1);
    const updated = pushHistory(initial, 2);

    expect(updated.past).toEqual([1]);
    expect(updated.present).toBe(2);
    expect(updated.future).toEqual([]);
  });

  it("supports undo and redo", () => {
    const history = pushHistory(pushHistory(createHistory("a"), "b"), "c");
    const undone = undoHistory(history);
    const redone = redoHistory(undone);

    expect(undone.present).toBe("b");
    expect(redone.present).toBe("c");
  });
});
