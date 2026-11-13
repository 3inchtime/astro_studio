import { beforeEach, expect, it, vi } from "vitest";
import { getLlmConfigs, saveLlmConfigs } from "./api";
const mocks = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke, Channel: class {}, convertFileSrc: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
beforeEach(() => mocks.invoke.mockReset());
it("sends LLM credential actions without raw api_key or display fields", async () => {
  const config = { id: "one", name: "One", protocol: "openai" as const, model: "model", base_url: "https://example.test", capability: "text" as const, enabled: true, api_key: "replacement", api_key_configured: true, api_key_masked: "••••••••" };
  await saveLlmConfigs([config]);
  const wire = mocks.invoke.mock.calls[0][1].configs[0];
  expect(wire).not.toHaveProperty("api_key");
  expect(wire).not.toHaveProperty("api_key_configured");
  expect(wire.credential_update).toEqual({ action: "replace", value: "replacement" });
});
it("never puts a secret returned by an older backend in LLM query state", async () => {
  mocks.invoke.mockResolvedValue([{ id: "one", api_key: "legacy-raw-key", api_key_configured: true }]);
  expect(JSON.stringify(await getLlmConfigs())).not.toContain("legacy-raw-key");
});
