import { describe, expect, it } from "vitest";
import { consumePendingPrompt, savePendingPrompt } from "./pendingPrompt";

describe("pendingPrompt", () => {
  it("stores one prompt and consumes it once", () => {
    savePendingPrompt("cinematic portrait");

    expect(consumePendingPrompt()).toBe("cinematic portrait");
    expect(consumePendingPrompt()).toBeNull();
  });
});
