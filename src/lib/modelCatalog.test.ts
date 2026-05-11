import { describe, expect, it } from "vitest";
import { getImageModelCatalogEntry } from "./modelCatalog";

describe("model catalog", () => {
  it("exposes common GPT Image 2 output sizes from the OpenAI API", () => {
    expect(getImageModelCatalogEntry("gpt-image-2").parameterCapabilities.sizes)
      .toEqual([
        "auto",
        "1024x1024",
        "1536x1024",
        "1024x1536",
        "2048x2048",
        "2048x1152",
        "3840x2160",
        "2160x3840",
      ]);
  });

  it("keeps Gemini image models on the supported aspect-ratio presets", () => {
    expect(getImageModelCatalogEntry("nano-banana").parameterCapabilities.sizes)
      .toEqual(["1024x1024", "1536x1024", "1024x1536"]);
  });
});
