import { describe, expect, it } from "vitest";
import { credentialDraft, credentialWrite } from "./credentialDraft";

describe("credential boundary", () => {
  it("keeps saved keys without resubmitting display status", () => {
    expect(credentialWrite({ id: "one", api_key: "", api_key_configured: true, api_key_masked: "••••••••" })).toEqual({ id: "one", credential_update: { action: "keep" } });
  });
  it("distinguishes replacement, clearing and abandoning an input", () => {
    expect(credentialWrite({ api_key: "new-key" })).toEqual({ credential_update: { action: "replace", value: "new-key" } });
    expect(credentialWrite({ api_key: "", credential_update: { action: "clear" } })).toEqual({ credential_update: { action: "clear" } });
    expect(credentialWrite({ api_key: "" })).toEqual({ credential_update: { action: "keep" } });
  });
  it("drops secrets and replacement actions from a loaded draft", () => {
    const draft = credentialDraft({ id: "one", api_key: "must-not-remain", api_key_configured: true, credential_update: { action: "replace" as const, value: "must-not-remain" } });
    expect(JSON.stringify(draft)).not.toContain("must-not-remain");
    expect(draft.api_key_configured).toBe(true);
  });
});
