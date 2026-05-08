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
