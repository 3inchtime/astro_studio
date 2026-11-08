import { describe, expect, it } from "vitest";
import {
  completeGenerationMessage,
  generationsToMessages,
} from "./generationMessages";
import type { GenerateResponse, GenerationResult, Message } from "../types";

function generationResult(
  id: string,
  status: string,
  error_message?: string | null,
): GenerationResult {
  return {
    generation: {
      id,
      prompt: `Prompt ${id}`,
      engine: "gpt-image-2",
      request_kind: "generate",
      size: "1024x1024",
      quality: "auto",
      background: "auto",
      output_format: "png",
      output_compression: 100,
      moderation: "auto",
      input_fidelity: "high",
      image_count: 1,
      source_image_count: 0,
      source_image_paths: [],
      status,
      error_message,
      created_at: "2026-04-26T00:00:00Z",
      deleted_at: null,
    },
    images: [
      {
        id: `image-${id}`,
        generation_id: id,
        file_path: `/tmp/${id}.png`,
        thumbnail_path: `/tmp/${id}-thumb.png`,
        width: 1024,
        height: 1024,
        file_size: 2048,
      },
    ],
  };
}

describe("generationsToMessages", () => {
  it("maps completed generations to complete assistant messages with images", () => {
    const messages = generationsToMessages([
      generationResult("generation-1", "completed"),
    ]);

    expect(messages).toEqual([
      {
        id: "user-generation-1",
        role: "user",
        content: "Prompt generation-1",
        sourceImages: [],
        status: "complete",
        createdAt: "2026-04-26T00:00:00Z",
      },
      {
        id: "assistant-generation-1",
        role: "assistant",
        content: "",
        generationId: "generation-1",
        images: [
          {
            imageId: "image-generation-1",
            generationId: "generation-1",
            path: "/tmp/generation-1.png",
            thumbnailPath: "/tmp/generation-1-thumb.png",
          },
        ],
        error: undefined,
        status: "complete",
        createdAt: "2026-04-26T00:00:00Z",
      },
    ]);
  });

  it("maps failed generations to failed assistant messages with error text", () => {
    const messages = generationsToMessages([
      generationResult("generation-2", "failed", "API request failed"),
    ]);

    expect(messages[1]).toMatchObject({
      id: "assistant-generation-2",
      role: "assistant",
      error: "API request failed",
      status: "failed",
    });
  });

  it("maps non-completed and non-failed generations to processing assistant messages", () => {
    const messages = generationsToMessages([
      generationResult("generation-3", "processing"),
    ]);

    expect(messages[1]).toMatchObject({
      id: "assistant-generation-3",
      role: "assistant",
      status: "processing",
    });
  });
});

describe("completeGenerationMessage", () => {
  const completedResult: GenerateResponse = {
    generation_id: "generation-new",
    conversation_id: "conversation-1",
    images: [
      {
        id: "image-new",
        generation_id: "generation-new",
        file_path: "/tmp/generated.png",
        thumbnail_path: "/tmp/generated-thumb.png",
        width: 1024,
        height: 1024,
        file_size: 2048,
      },
    ],
  };

  it("completes a reloaded processing message that already has the real generation id", () => {
    const messages: Message[] = [
      {
        id: "assistant-generation-new",
        role: "assistant",
        content: "",
        generationId: "generation-new",
        status: "processing",
        createdAt: "2026-04-26T00:00:00Z",
      },
    ];

    expect(
      completeGenerationMessage(messages, "temp-local", completedResult)[0],
    ).toMatchObject({
      id: "assistant-generation-new",
      generationId: "generation-new",
      status: "complete",
      images: [
        {
          imageId: "image-new",
          generationId: "generation-new",
          path: "/tmp/generated.png",
          thumbnailPath: "/tmp/generated-thumb.png",
        },
      ],
    });
  });
});

it("restores failed 2.6 retry options from persisted metadata", () => {
  const result = generationResult("gif", "failed");
  result.generation.engine = "gpt-image-2.6-flare";
  result.generation.output_format = "gif";
  result.generation.request_metadata = JSON.stringify({
    model: "gpt-image-2.6-flare",
    project_id: "p1",
    conversation_id: "c1",
    advanced_options: {
      complexity: "low",
      frameCount: 12,
      frameDurationMs: 80,
    },
  });
  expect(generationsToMessages([result])[1].retryRequest).toMatchObject({
    model: "gpt-image-2.6-flare",
    outputFormat: "gif",
    projectId: "p1",
    conversationId: "c1",
    advancedOptions: { complexity: "low", frameCount: 12, frameDurationMs: 80 },
  });
  result.generation.request_metadata = "invalid";
  expect(() => generationsToMessages([result])).not.toThrow();
  expect(generationsToMessages([result])[1].retryRequest).toBeUndefined();
});

it("matches streamed previews by client request identity and ignores completed requests", async () => {
  const { applyGenerationPartial } = await import("./generationMessages");
  const pending: Message = {
    id: "assistant-client",
    role: "assistant",
    content: "",
    status: "processing",
    generationId: "client",
    createdAt: "",
  };
  const event = {
    generation_id: "server",
    client_request_id: "client",
    partial_image_index: 0,
    b64_json: "aGVsbG8=",
    output_format: "webp",
  };
  expect(applyGenerationPartial([pending], event)[0]).toMatchObject({
    generationId: "server",
    partialPreview: "data:image/webp;base64,aGVsbG8=",
  });
  expect(
    applyGenerationPartial([{ ...pending, status: "complete" }], event)[0],
  ).not.toHaveProperty("partialPreview");
});

it("shows interrupted requests as retryable failures instead of endless loading", () => {
  const result = generationResult(
    "interrupted",
    "interrupted",
    "Provider outcome unknown",
  );
  result.generation.engine = "gpt-image-2.6-flare";
  result.generation.request_metadata = JSON.stringify({
    model: result.generation.engine,
    project_id: "p1",
    conversation_id: "c1",
  });
  const message = generationsToMessages([result])[1];
  expect(message.status).toBe("failed");
  expect(message.retryRequest?.model).toBe("gpt-image-2.6-flare");
});
