import { beforeEach, describe, expect, it, vi } from "vitest";
import tauriConfig from "../../src-tauri/tauri.conf.json";
import { clearLogs, toAssetUrl } from "./api";

const tauriApi = vi.hoisted(() => ({
  convertFileSrc: vi.fn((path: string) => path),
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => tauriApi);

describe("api log commands", () => {
  beforeEach(() => {
    tauriApi.invoke.mockReset();
    tauriApi.convertFileSrc.mockClear();
  });

  it("preserves zero retention when clearing all logs", async () => {
    tauriApi.invoke.mockResolvedValue(0);

    await clearLogs(0);

    expect(tauriApi.invoke).toHaveBeenCalledWith("clear_logs", { beforeDays: 0 });
  });
});

describe("asset URLs", () => {
  beforeEach(() => {
    tauriApi.convertFileSrc.mockClear();
  });

  it("lets Tauri encode Windows file paths without rewriting separators", () => {
    const imagePath = String.raw`C:\Users\Chen\AppData\Roaming\com.astrostudio.desktop\images\2026\04\28\image.png`;

    expect(toAssetUrl(imagePath)).toBe(imagePath);
    expect(tauriApi.convertFileSrc).toHaveBeenCalledWith(imagePath);
  });

  it("passes macOS file paths to Tauri unchanged", () => {
    const imagePath = "/Users/chen/Library/Application Support/com.astrostudio.desktop/images/2026/04/28/image.png";

    expect(toAssetUrl(imagePath)).toBe(imagePath);
    expect(tauriApi.convertFileSrc).toHaveBeenCalledWith(imagePath);
  });

  it("allows the Windows asset protocol host in the Tauri CSP", () => {
    const csp = tauriConfig.app.security.csp;

    expect(csp).toContain("asset:");
    expect(csp).toContain("http://asset.localhost");
  });
});
