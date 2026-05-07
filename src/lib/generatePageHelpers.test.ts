import { describe, expect, it, vi } from "vitest";
import type { EditSourceImage, PromptFavorite } from "../types";
import {
  createUploadedEditSource,
  editSourcesToMessageImages,
  mergeEditSources,
  normalizePromptFavorite,
  upsertPromptFavorite,
} from "./generatePageHelpers";

describe("generatePageHelpers", () => {
  it("merges edit sources by path and lets incoming sources replace existing duplicates", () => {
    const current: EditSourceImage[] = [
      { id: "source-a", path: "/tmp/a.png", label: "a.png" },
      {
        id: "source-b-old",
        path: "/tmp/b.png",
        label: "old-b.png",
        imageId: "image-old",
      },
    ];
    const incoming: EditSourceImage[] = [
      {
        id: "source-b-new",
        path: "/tmp/b.png",
        label: "new-b.png",
        imageId: "image-new",
      },
      { id: "source-c", path: "/tmp/c.png", label: "c.png" },
    ];

    expect(mergeEditSources(current, incoming)).toEqual([
      { id: "source-a", path: "/tmp/a.png", label: "a.png" },
      {
        id: "source-b-new",
        path: "/tmp/b.png",
        label: "new-b.png",
        imageId: "image-new",
      },
      { id: "source-c", path: "/tmp/c.png", label: "c.png" },
    ]);
  });

  it("creates uploaded edit source labels from normalized file paths", () => {
    const randomUUID = vi
      .spyOn(crypto, "randomUUID")
      .mockReturnValue("uuid-1" as `${string}-${string}-${string}-${string}-${string}`);

    expect(createUploadedEditSource("C:\\Users\\chen\\source image.png")).toEqual(
      {
        id: "uuid-1:C:/Users/chen/source image.png",
        path: "C:\\Users\\chen\\source image.png",
        label: "source image.png",
      },
    );

    randomUUID.mockRestore();
  });

  it("converts edit sources into message images while preserving existing image metadata", () => {
    expect(
      editSourcesToMessageImages(
        [
          {
            id: "uploaded",
            path: "/tmp/uploaded.png",
            label: "uploaded.png",
          },
          {
            id: "message-source",
            path: "/tmp/message-source.png",
            label: "message-source.png",
            imageId: "image-1",
            generationId: "generation-1",
          },
        ],
        "generation-new",
      ),
    ).toEqual([
      {
        imageId: "generation-new-source-0",
        generationId: "generation-new",
        path: "/tmp/uploaded.png",
        thumbnailPath: "/tmp/uploaded.png",
      },
      {
        imageId: "image-1",
        generationId: "generation-1",
        path: "/tmp/message-source.png",
        thumbnailPath: "/tmp/message-source.png",
      },
    ]);
  });

  it("normalizes prompt favorite matching with trimming and locale lowercase", () => {
    expect(normalizePromptFavorite("  A Blue-Violet SUNRISE  ")).toBe(
      "a blue-violet sunrise",
    );
  });

  it("upserts prompt favorites by id or normalized prompt with newest first", () => {
    const current: PromptFavorite[] = [
      createFavorite("favorite-1", "A glass mountain"),
      createFavorite("favorite-2", "A quiet river"),
      createFavorite("favorite-3", "A city at night"),
    ];

    expect(
      upsertPromptFavorite(
        current,
        createFavorite("favorite-new", "  A QUIET RIVER  "),
      ),
    ).toEqual([
      createFavorite("favorite-new", "  A QUIET RIVER  "),
      createFavorite("favorite-1", "A glass mountain"),
      createFavorite("favorite-3", "A city at night"),
    ]);
  });
});

function createFavorite(id: string, prompt: string): PromptFavorite {
  return {
    id,
    prompt,
    created_at: "2026-04-28T00:00:00Z",
    updated_at: "2026-04-28T00:00:00Z",
  };
}
