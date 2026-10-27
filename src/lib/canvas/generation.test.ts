import { describe, it, expect } from "vitest";
import { createCanvasDocumentContent, createImageObject } from "./document";
import { completeCanvasGeneration, failCanvasGeneration } from "./generation";
import type { CanvasGenerationRequest, GeneratedImage } from "../../types";
const request: CanvasGenerationRequest = {
  request_id: "r",
  project_id: null,
  frame: { x: 0, y: 0, width: 100, height: 100, aspect: "1:1" },
  prompt: "hello",
  status: "pending",
  image_ids: [],
  created_at: new Date().toISOString(),
};
const image = (id: string): GeneratedImage => ({
  id,
  generation_id: "g",
  file_path: `/${id}.png`,
  thumbnail_path: `/${id}.png`,
  width: 200,
  height: 100,
  file_size: 1,
});
describe("canvas generation merge", () => {
  it("places a batch beside the submission frame, preserving latest edits and avoiding collisions", () => {
    const content = createCanvasDocumentContent();
    content.generation_requests = [request];
    content.layers[0].objects = [
      createImageObject({
        image_path: "/other.png",
        x: 132,
        y: 0,
        width: 200,
        height: 100,
        original_width: 200,
        original_height: 100,
      }),
    ];
    content.generation_requests[0] = { ...request, error: "IPC reply lost" };
    const next = completeCanvasGeneration(content, "r", "g", [
      image("a"),
      image("b"),
    ]);
    expect(next.layers).toHaveLength(2);
    expect(next.generation_requests?.[0].error).toBeUndefined();
    expect(next.layers[1]).toEqual(content.layers[0]);
    const objects = next.layers[0].objects;
    expect(objects.map((o) => (o.type === "image" ? o.x : 0))).toEqual([
      364, 596,
    ]);
    expect(next.generation_requests?.[0]).toMatchObject({
      status: "completed",
      image_ids: ["a", "b"],
      generation_id: "g",
    });
    expect(completeCanvasGeneration(next, "r", "g", [image("a")])).toBe(next);
    expect(failCanvasGeneration(next, "r", "late failed event")).toBe(next);
  });
  it("deduplicates images and treats an empty success as failed without deleting content", () => {
    const content = createCanvasDocumentContent();
    content.generation_requests = [request];
    expect(
      completeCanvasGeneration(content, "r", "g", [image("a"), image("a")])
        .layers[0].objects,
    ).toHaveLength(1);
    expect(
      completeCanvasGeneration(content, "r", "g", []).generation_requests?.[0]
        .status,
    ).toBe("failed");
    expect(failCanvasGeneration(content, "r", "oops").layers).toEqual(
      content.layers,
    );
  });
});
