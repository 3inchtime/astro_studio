import { beforeEach, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke, Channel: class {}, convertFileSrc: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

beforeEach(() => {
  vi.resetModules();
  mocks.invoke.mockReset();
  Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: { invoke: mocks.invoke } });
});

it.each([
  ["retention cleanup", "save_trash_settings", "saveTrashSettings", 1],
  ["canvas deletion", "delete_canvas_document", "deleteCanvasDocument", "board"],
] as const)("waits for accepted canvas saves before %s", async (_label, command, method, argument) => {
  const api = await import("./api");
  const { queueCanvasSave } = await import("./canvas/persistence");
  let resolve!: () => void;
  const disk = new Promise<void>((done) => { resolve = done; });
  queueCanvasSave("board", () => disk);
  const action = method === "saveTrashSettings"
    ? api.saveTrashSettings(argument as number)
    : api.deleteCanvasDocument(argument as string);
  await Promise.resolve();
  expect(mocks.invoke).not.toHaveBeenCalled();
  resolve();
  await action;
  expect(mocks.invoke).toHaveBeenCalledWith(command, method === "saveTrashSettings" ? { retentionDays: 1 } : { id: "board" });
});

it.each(["saveTrashSettings", "deleteCanvasDocument"] as const)("blocks %s when a canvas save failed", async (method) => {
  const api = await import("./api");
  const { queueCanvasSave } = await import("./canvas/persistence");
  await expect(queueCanvasSave("board", async () => { throw new Error("Disk full"); }).promise).rejects.toThrow("Disk full");
  const action = method === "saveTrashSettings" ? api.saveTrashSettings(1) : api.deleteCanvasDocument("board");
  await expect(action).rejects.toThrow("Disk full");
  expect(mocks.invoke).not.toHaveBeenCalled();
});
