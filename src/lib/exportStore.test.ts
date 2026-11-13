import { beforeEach, expect, it, vi } from "vitest";
import { useExportStore } from "./exportStore";
import { exportGenerations } from "./exportApi";
vi.mock("./exportApi", () => ({
  exportGenerations: vi.fn(),
  cancelExport: vi.fn(),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}));
beforeEach(() => {
  vi.clearAllMocks();
  useExportStore.setState({
    busy: false,
    result: null,
    error: null,
    operationId: null,
  });
});
it("persists partial completion and retries only the backend retry token", async () => {
  vi.mocked(exportGenerations)
    .mockResolvedValueOnce({
      total: 3,
      completed: 2,
      cancelled_image_ids: [],
      succeeded_image_ids: ["a", "b"],
      failed: [{ image_id: "c", generation_id: "g", code: "source_missing" }],
      cancelled: false,
      retry_token: "retry",
    })
    .mockResolvedValueOnce({
      total: 3,
      completed: 3,
      cancelled_image_ids: [],
      succeeded_image_ids: ["c"],
      failed: [],
      cancelled: false,
      retry_token: null,
    });
  await useExportStore.getState().start(["g"], false);
  expect(useExportStore.getState().result?.completed).toBe(2);
  await useExportStore.getState().retry();
  expect(exportGenerations).toHaveBeenLastCalledWith(
    [],
    false,
    expect.any(String),
    "retry",
  );
  expect(useExportStore.getState().result?.completed).toBe(3);
});
it("treats directory dialog dismissal as cancellation, not failure", async () => {
  vi.mocked(exportGenerations).mockResolvedValue(null);
  await useExportStore.getState().start(["g"], false);
  expect(useExportStore.getState().error).toBeNull();
  expect(useExportStore.getState().busy).toBe(false);
  expect(useExportStore.getState().result).toBeNull();
});
