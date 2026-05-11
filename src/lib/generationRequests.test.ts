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
      prompt: "A luminous glass observatory",
      model: "nano-banana",
      imageCount: 3,
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
      prompt: "A luminous glass observatory",
      model: "nano-banana",
      imageCount: 3,
      conversationId: "conversation-1",
      projectId: "project-1",
      sourceImagePaths: ["/tmp/source-1.png", "/tmp/source-2.png"],
    });
  });

  it("includes supported visible GPT edit controls with source image paths", () => {
    expect(buildEditParams(request())).toEqual({
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
