import { renderHook, act, waitFor, cleanup } from "@testing-library/react";
import { describe, it, expect, vi, afterEach } from "vitest";
import { useCanvasGenerationRecovery } from "./useCanvasGenerationRecovery";
import { createCanvasDocumentContent } from "./document";
import type { CanvasDocumentContent } from "../../types";
const { lookup, listeners } = vi.hoisted(() => ({
  lookup: vi.fn(),
  listeners: [] as (() => void)[],
}));
vi.mock("../api", () => ({
  getCanvasGenerationStatus: lookup,
  onGenerationComplete: (cb: () => void) => {
    listeners.push(cb);
    return Promise.resolve(vi.fn());
  },
  onGenerationFailed: () => Promise.resolve(vi.fn()),
  onGenerationProgress: () => Promise.resolve(vi.fn()),
}));
const pending = () => {
  const c = createCanvasDocumentContent();
  c.generation_requests = [
    {
      request_id: "req",
      project_id: null,
      frame: { ...c.frame },
      prompt: "draw",
      status: "pending",
      image_ids: [],
      created_at: "2020-01-01T00:00:00Z",
    },
  ];
  return c;
};
const response = {
  generation_id: "g",
  status: "completed",
  error: null,
  images: [
    {
      id: "a",
      generation_id: "g",
      file_path: "/a.png",
      thumbnail_path: "/a.png",
      width: 100,
      height: 100,
      file_size: 1,
    },
  ],
};
afterEach(() => {
  cleanup();
  lookup.mockReset();
  listeners.length = 0;
});
describe("saved canvas request recovery", () => {
  it("recovers saved pending results and ignores repeated terminal events", async () => {
    let content = pending();
    lookup.mockResolvedValue(response);
    const apply = vi.fn((_id: string, next: CanvasDocumentContent) => {
      content = next;
    });
    renderHook(() =>
      useCanvasGenerationRecovery({
        documentId: "doc",
        enabled: true,
        getContent: () => content,
        isCurrent: () => true,
        apply,
        liveRequests: new Set(),
      }),
    );
    await waitFor(() => expect(apply).toHaveBeenCalledTimes(1));
    expect(lookup).toHaveBeenCalledWith("doc", "req");
    expect(content.layers).toHaveLength(2);
    await act(async () => listeners[0]());
    expect(apply).toHaveBeenCalledTimes(1);
  });
  it("keeps query failures pending and compensates on the next event", async () => {
    let content = pending();
    lookup.mockRejectedValueOnce(new Error("busy")).mockResolvedValue(response);
    const apply = vi.fn((_id: string, next: CanvasDocumentContent) => {
      content = next;
    });
    renderHook(() =>
      useCanvasGenerationRecovery({
        documentId: "doc",
        enabled: true,
        getContent: () => content,
        isCurrent: () => true,
        apply,
        liveRequests: new Set(),
      }),
    );
    await waitFor(() => expect(lookup).toHaveBeenCalledTimes(1));
    expect(apply).not.toHaveBeenCalled();
    await act(async () => listeners[0]());
    await waitFor(() => expect(apply).toHaveBeenCalledTimes(1));
  });
  it("marks abandoned missing jobs failed but allows live submissions grace", async () => {
    let content = pending();
    lookup.mockResolvedValue(null);
    const apply = vi.fn((_id: string, next: CanvasDocumentContent) => {
      content = next;
    });
    const liveRequests = new Set(["req"]);
    renderHook(() =>
      useCanvasGenerationRecovery({
        documentId: "doc",
        enabled: true,
        getContent: () => content,
        isCurrent: () => true,
        apply,
        liveRequests,
      }),
    );
    await waitFor(() => expect(lookup).toHaveBeenCalled());
    expect(apply).not.toHaveBeenCalled();
    liveRequests.clear();
    await act(async () => listeners[0]());
    await waitFor(() =>
      expect(content.generation_requests?.[0].status).toBe("failed"),
    );
  });
  it("never applies a lookup after unmount or a document switch", async () => {
    let resolve: (v: typeof response) => void = () => {};
    lookup.mockReturnValue(new Promise((r) => (resolve = r)));
    const apply = vi.fn();
    const content = pending();
    const hook = renderHook(() =>
      useCanvasGenerationRecovery({
        documentId: "doc",
        enabled: true,
        getContent: () => content,
        isCurrent: () => false,
        apply,
        liveRequests: new Set(),
      }),
    );
    hook.unmount();
    await act(async () => resolve(response));
    expect(apply).not.toHaveBeenCalled();
  });
});
