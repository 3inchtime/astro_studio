import { beforeEach, expect, it, vi } from "vitest";
const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke, convertFileSrc: (s: string) => s, Channel: class { constructor(public onmessage: unknown) {} } }));
beforeEach(() => { vi.resetModules(); invoke.mockReset(); });
it("flushes saves queued during download before handing control to the Windows installer", async () => {
  const { installUpdate } = await import("./api");
  const { queueCanvasSave } = await import("./canvas/persistence");
  let finish!: () => void;
  const saving = new Promise<void>((resolve) => { finish = resolve; });
  invoke.mockImplementation(async (command) => {
    if (command === "download_update") { queueCanvasSave("during-download", () => saving); return "verified-token"; }
  });
  const installing = installUpdate(vi.fn());
  await vi.waitFor(() => expect(invoke).toHaveBeenCalledWith("download_update", expect.anything()));
  expect(invoke).not.toHaveBeenCalledWith("install_update", expect.anything());
  finish(); await installing;
  expect(invoke).toHaveBeenLastCalledWith("install_update", { token: "verified-token" });
});
it("keeps the app running when an outstanding canvas save fails", async () => {
  const { installUpdate } = await import("./api");
  const { queueCanvasSave } = await import("./canvas/persistence");
  invoke.mockResolvedValue("verified-token");
  await expect(queueCanvasSave("failed", async () => { throw new Error("disk full"); }).promise).rejects.toThrow("disk full");
  await expect(installUpdate(vi.fn())).rejects.toThrow("disk full");
  expect(invoke).not.toHaveBeenCalledWith("install_update", expect.anything());
});
