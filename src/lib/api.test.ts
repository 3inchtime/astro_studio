import { beforeEach, describe, expect, it, vi } from "vitest";
import { clearLogs } from "./api";

const tauriApi = vi.hoisted(() => ({
  convertFileSrc: vi.fn((path: string) => path),
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => tauriApi);

describe("api log commands", () => {
  beforeEach(() => {
    tauriApi.invoke.mockReset();
  });

  it("preserves zero retention when clearing all logs", async () => {
    tauriApi.invoke.mockResolvedValue(0);

    await clearLogs(0);

    expect(tauriApi.invoke).toHaveBeenCalledWith("clear_logs", { beforeDays: 0 });
  });
});
