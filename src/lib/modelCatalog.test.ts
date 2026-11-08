import { describe, expect, it } from "vitest";
import { getImageModelCatalogEntry } from "./modelCatalog";

describe("model catalog", () => {
  it("exposes common GPT Image 2 output sizes from the OpenAI API", () => {
    expect(
      getImageModelCatalogEntry("gpt-image-2").parameterCapabilities.sizes,
    ).toEqual([
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

  it("keeps GPT Image 2 moderation and input fidelity hidden in the composer", () => {
    expect(
      getImageModelCatalogEntry("gpt-image-2").parameterCapabilities
        .moderationLevels,
    ).toEqual(["auto"]);
    expect(
      getImageModelCatalogEntry("gpt-image-2").parameterCapabilities
        .inputFidelityOptions,
    ).toEqual(["high"]);
  });

  it("keeps Gemini image models on the supported aspect-ratio presets", () => {
    expect(
      getImageModelCatalogEntry("nano-banana").parameterCapabilities.sizes,
    ).toEqual(["1024x1024", "1536x1024", "1024x1536"]);
  });
});

it("separates new model capabilities instead of exposing OpenAI controls everywhere", () => {
  expect(
    getImageModelCatalogEntry("gpt-image-2.5-flare").parameterCapabilities
      .qualities,
  ).toContain("max");
  expect(
    getImageModelCatalogEntry("gpt-image-2").parameterCapabilities.qualities,
  ).not.toContain("max");
  const grok = getImageModelCatalogEntry("grok-imagine-image-2.0");
  expect(grok.maxReferenceImages).toBe(5);
  expect(grok.parameterCapabilities.qualities).toEqual([
    "auto",
    "low",
    "medium",
  ]);
  expect(grok.parameterCapabilities.resolutions).toEqual(["1K", "2K"]);
  const seedream = getImageModelCatalogEntry("doubao-seedream-5-0-pro-260628");
  expect(seedream.connectionDefaults.editUrl).toBe(
    seedream.connectionDefaults.generationUrl,
  );
  expect(seedream.parameterCapabilities.imageCounts).toEqual([1]);
  expect(
    getImageModelCatalogEntry("gemini-nano-banana-2.1").connectionDefaults
      .generationUrl,
  ).toMatch(/\/interactions$/);
});

it.each(["gpt-image-2.6-flare", "gpt-image-2.6-sunburst"])(
  "exposes the complete 2.6 catalog for %s without rewriting 2.5 profiles",
  (model) => {
    const entry = getImageModelCatalogEntry(model);
    expect(entry.providerModelId).toBe(model);
    expect(entry.parameterCapabilities.outputFormats).toEqual([
      "png",
      "jpeg",
      "webp",
      "svg",
      "gif",
    ]);
    expect(entry.parameterCapabilities.qualities).toContain("max");
    expect(entry.parameterCapabilities.imageCounts).toEqual([
      1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
    ]);
    expect(entry.parameterCapabilities.moderationLevels).toEqual([
      "auto",
      "low",
    ]);
    expect(
      getImageModelCatalogEntry("gpt-image-2.5-flare").providerModelId,
    ).toBe("gpt-image-2.5-flare");
  },
);
