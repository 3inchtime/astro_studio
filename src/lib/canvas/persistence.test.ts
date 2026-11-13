import { beforeEach, describe, expect, it, vi } from "vitest";

function deferred() {
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

beforeEach(() => vi.resetModules());

describe("canvas saves before library maintenance", () => {
  it("waits for accepted saves across every document", async () => {
    const { queueCanvasSave, flushCanvasSaves } = await import("./persistence");
    const first = deferred();
    const second = deferred();
    queueCanvasSave("first", () => first.promise);
    queueCanvasSave("second", () => second.promise);
    let complete = false;
    const flush = flushCanvasSaves().then(() => {
      complete = true;
    });
    first.resolve();
    await Promise.resolve();
    await Promise.resolve();
    expect(complete).toBe(false);
    second.resolve();
    await flush;
    expect(complete).toBe(true);
  });

  it("includes a newer snapshot queued while flushing an older one", async () => {
    const { queueCanvasSave, flushCanvasSaves } = await import("./persistence");
    const old = deferred();
    const latest = deferred();
    const written: string[] = [];
    queueCanvasSave("same", async () => {
      await old.promise;
      written.push("old");
    });
    let complete = false;
    const flush = flushCanvasSaves().then(() => {
      complete = true;
    });
    const newest = queueCanvasSave("same", async () => {
      await latest.promise;
      written.push("latest");
    });
    old.resolve();
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    expect(complete).toBe(false);
    latest.resolve();
    await newest.promise;
    await flush;
    expect(written).toEqual(["old", "latest"]);
  });

  it("rejects maintenance on a failed save until an explicit retry succeeds", async () => {
    const { queueCanvasSave, flushCanvasSaves, waitForCanvasSaves } =
      await import("./persistence");
    const write = vi
      .fn()
      .mockRejectedValueOnce(new Error("Disk is full"))
      .mockResolvedValueOnce(undefined);
    const save = queueCanvasSave("failed", write);
    await expect(save.promise).rejects.toThrow("Disk is full");
    await expect(flushCanvasSaves()).rejects.toThrow("Disk is full");
    expect(write).toHaveBeenCalledTimes(1);
    await waitForCanvasSaves("failed", true, () => true);
    await expect(flushCanvasSaves()).resolves.toBeUndefined();
    expect(write).toHaveBeenCalledTimes(2);
  });

  it("waits for preview rendering included in the queued write", async () => {
    const { queueCanvasSave, flushCanvasSaves } = await import("./persistence");
    const preview = deferred();
    const diskWrite = vi.fn().mockResolvedValue(undefined);
    queueCanvasSave("preview", async () => {
      await preview.promise;
      await diskWrite();
    });
    let complete = false;
    const flush = flushCanvasSaves().then(() => {
      complete = true;
    });
    await Promise.resolve();
    expect(diskWrite).not.toHaveBeenCalled();
    expect(complete).toBe(false);
    preview.resolve();
    await flush;
    expect(diskWrite).toHaveBeenCalledTimes(1);
  });
});
