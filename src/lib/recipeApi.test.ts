import { describe, expect, it } from "vitest";
import { recipeToRequest } from "./recipeApi";
import type { GenerationRecipe } from "../types/recipe";
const recipe: GenerationRecipe = {
  version: 1,
  generation_id: "source",
  provenance: "job",
  provider_profile_id: "saved-provider",
  endpoint_snapshot: "https://example.org/images/edits",
  warnings: [],
  prompt: "draw",
  model: "gpt-image-2.6-flare",
  request_kind: "edit",
  source_image_paths: ["/second.png", "/first.png"],
  options: {
    size: "auto",
    quality: "xhigh",
    output_format: "webp",
    output_compression: 77,
    background: "transparent",
    image_count: 2,
    moderation: "low",
    input_fidelity: "high",
    advanced_options: {
      stream: true,
      partialImages: 2,
      markupPath: "/overlay.png",
    },
    aspect_ratio: "16:9",
    resolution: "2K",
  },
};
describe("recipe hydration", () => {
  it("preserves all advanced controls, ordered sources and exact provider without reusing request identity", () => {
    const request = recipeToRequest(recipe);
    expect(request).toMatchObject({
      sourceGenerationId: "source",
      providerProfileId: "saved-provider",
      quality: "xhigh",
      outputCompression: 77,
      aspectRatio: "16:9",
      resolution: "2K",
      advancedOptions: recipe.options.advanced_options,
    });
    expect(request.editSources.map((s) => s.path)).toEqual(
      recipe.source_image_paths,
    );
    expect(request.clientRequestId).toBeUndefined();
  });
  it("hydrates streaming controls from older job option fields", () => {
    const request = recipeToRequest({
      ...recipe,
      options: {
        ...recipe.options,
        advanced_options: undefined,
        stream: true,
        partial_images: 2,
      },
    });
    expect(request.advancedOptions).toMatchObject({
      stream: true,
      partialImages: 2,
    });
  });

  it("does not silently normalize an unknown model or future schema", () => {
    expect(() =>
      recipeToRequest({ ...recipe, model: "future-model" }),
    ).toThrow();
    expect(() => recipeToRequest({ ...recipe, version: 2 })).toThrow();
  });
});
