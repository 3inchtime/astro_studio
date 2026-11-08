import { describe, expect, it } from "vitest";
import { buildEditParams, buildGenerationParams } from "./generationRequests";
import type { RetryGenerationRequest } from "../types";

function request(
  overrides: Partial<RetryGenerationRequest> = {},
): RetryGenerationRequest {
  return {
    prompt: "A luminous glass observatory",
    model: "gpt-image-2",
    size: "1536x1024",
    quality: "high",
    background: "transparent",
    outputFormat: "webp",
    moderation: "low",
    inputFidelity: "low",
    imageCount: 3,
    conversationId: "conversation-1",
    projectId: "project-1",
    editSources: [
      {
        id: "source-1",
        path: "/tmp/source-1.png",
        label: "source-1.png",
      },
    ],
    ...overrides,
  };
}

describe("generation request helpers", () => {
  it("includes supported visible GPT generation controls", () => {
    expect(buildGenerationParams(request())).toEqual({
      size: "1536x1024",
      prompt: "A luminous glass observatory",
      model: "gpt-image-2",
      quality: "high",
      background: "transparent",
      outputFormat: "webp",
      imageCount: 3,
      conversationId: "conversation-1",
      projectId: "project-1",
    });
  });

  it("hides unsupported Nano Banana controls while keeping image count", () => {
    expect(
      buildGenerationParams(
        request({
          model: "nano-banana",
        }),
      ),
    ).toEqual({
      size: "1536x1024",
      prompt: "A luminous glass observatory",
      model: "nano-banana",
      imageCount: 1,
      conversationId: "conversation-1",
      projectId: "project-1",
    });
  });

  it("keeps source image paths for Nano Banana edits without unsupported edit controls", () => {
    expect(
      buildEditParams(
        request({
          model: "nano-banana",
          editSources: [
            {
              id: "source-1",
              path: "/tmp/source-1.png",
              label: "source-1.png",
            },
            {
              id: "source-2",
              path: "/tmp/source-2.png",
              label: "source-2.png",
            },
          ],
        }),
      ),
    ).toEqual({
      size: "1536x1024",
      prompt: "A luminous glass observatory",
      model: "nano-banana",
      imageCount: 1,
      conversationId: "conversation-1",
      projectId: "project-1",
      sourceImagePaths: ["/tmp/source-1.png", "/tmp/source-2.png"],
    });
  });

  it("includes supported visible GPT edit controls with source image paths", () => {
    expect(buildEditParams(request())).toEqual({
      size: "1536x1024",
      prompt: "A luminous glass observatory",
      model: "gpt-image-2",
      quality: "high",
      background: "transparent",
      outputFormat: "webp",
      imageCount: 3,
      conversationId: "conversation-1",
      projectId: "project-1",
      sourceImagePaths: ["/tmp/source-1.png"],
    });
  });
});

it("preserves ratio and resolution across generation and reference-image retry requests", () => {
  const retry = request({
    model: "grok-imagine-image-2.0",
    aspectRatio: "16:9",
    resolution: "2K",
    quality: "max",
  });
  for (const params of [buildGenerationParams(retry), buildEditParams(retry)]) {
    expect(params).toMatchObject({
      aspectRatio: "16:9",
      resolution: "2K",
      quality: "auto",
    });
    expect(params).not.toHaveProperty("background");
    expect(params).not.toHaveProperty("outputFormat");
  }
  expect(
    buildGenerationParams(
      request({ model: "doubao-seedream-5-0-pro-260628", imageCount: 4 }),
    ),
  ).toMatchObject({ imageCount: 1 });
});

it("round trips 2.6 GIF controls and removes stale raster controls", () => {
  const params = buildEditParams(
    request({
      model: "gpt-image-2.6-flare",
      outputFormat: "gif",
      background: "auto",
      imageCount: 10,
      outputCompression: 50,
      advancedOptions: {
        complexity: "medium",
        frameCount: 12,
        frameDurationMs: 80,
        stream: true,
        partialImages: 3,
        svg: { model: "author" },
      },
    }),
  );
  expect(params).toMatchObject({
    model: "gpt-image-2.6-flare",
    outputFormat: "gif",
    imageCount: 10,
    advancedOptions: {
      complexity: "medium",
      frameCount: 12,
      frameDurationMs: 80,
    },
  });
  expect(params).not.toHaveProperty("outputCompression");
  expect(params.advancedOptions).not.toHaveProperty("stream");
  expect(params.advancedOptions).not.toHaveProperty("svg");
});

it("keeps SVG author settings and omits them for raster requests", () => {
  const retry = request({
    model: "gpt-image-2.6-sunburst",
    outputFormat: "svg",
    background: "auto",
    imageCount: 1,
    editSources: [],
    advancedOptions: {
      complexity: "high",
      svg: { model: "author", reasoning: { effort: "high" } },
      frameCount: 9,
    },
  });
  expect(buildGenerationParams(retry).advancedOptions).toEqual({
    complexity: "high",
    svg: { model: "author", reasoning: { effort: "high" } },
  });
  expect(
    buildGenerationParams({
      ...retry,
      outputFormat: "webp",
      outputCompression: 72,
    }).outputCompression,
  ).toBe(72);
  expect(
    buildGenerationParams({ ...retry, outputFormat: "webp" }).advancedOptions,
  ).toEqual({ complexity: "high" });
});

it("keeps edit annotations on retries but never sends them to generation or other providers", () => {
  const retry = request({
    model: "gpt-image-2.6-flare",
    advancedOptions: { markupPath: "/tmp/markup.png", complexity: "low" },
  });
  expect(buildEditParams(retry).advancedOptions).toEqual(retry.advancedOptions);
  expect(buildGenerationParams(retry).advancedOptions).toEqual({
    complexity: "low",
  });
  expect(
    buildEditParams({ ...retry, model: "nano-banana" }),
  ).not.toHaveProperty("advancedOptions");
});
