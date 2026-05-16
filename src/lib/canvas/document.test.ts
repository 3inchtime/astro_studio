import { describe, expect, it } from "vitest";
import {
  createCanvasDocumentContent,
  createImageObject,
  resetImageObjectAspect,
  createStrokeObject,
  getActiveLayer,
  sanitizeCanvasDocumentContent,
  updateImageObject,
} from "./document";

describe("canvas document helpers", () => {
  it("creates a default canvas document content payload", () => {
    const content = createCanvasDocumentContent();

    expect(content.version).toBe(1);
    expect(content.frame.width).toBe(1024);
    expect(content.layers).toHaveLength(1);
    expect(content.layers[0].name).toBe("Sketch");
  });

  it("sanitizes mixed canvas objects", () => {
    const content = sanitizeCanvasDocumentContent({
      version: 2,
      viewport: { x: 10, y: 20, scale: 2 },
      frame: { x: 20, y: 40, width: 512, height: 256, aspect: "2:1" },
      layers: [
        {
          id: "layer-a",
          name: "Refs",
          visible: true,
          locked: false,
          objects: [
            createStrokeObject({ id: "stroke-1", points: [0, 1, 2, 3] }),
            createImageObject({
              id: "image-1",
              image_path: "/tmp/reference.png",
              width: 200,
              height: 100,
            }),
          ],
        },
      ],
    });

    expect(content.layers[0].objects[0].type).toBe("stroke");
    expect(content.layers[0].objects[1].type).toBe("image");
    expect(content.frame.aspect).toBe("2:1");
  });

  it("keeps original image dimensions for later aspect restoration", () => {
    expect(
      createImageObject({
        image_path: "/tmp/reference.png",
        width: 300,
        height: 120,
      }),
    ).toMatchObject({
      original_width: 300,
      original_height: 120,
    });
  });

  it("resets a freely resized image back to its original aspect ratio", () => {
    const content = createCanvasDocumentContent({
      layers: [
        createCanvasLayerWithImage({
          id: "image-1",
          width: 400,
          height: 400,
          original_width: 300,
          original_height: 150,
        }),
      ],
    });

    expect(resetImageObjectAspect(content, "image-1").layers[0].objects[0]).toMatchObject({
      width: 400,
      height: 200,
    });
  });

  it("updates an image object's free transform values", () => {
    const content = createCanvasDocumentContent({
      layers: [
        createCanvasLayerWithImage({
          id: "image-1",
          width: 200,
          height: 100,
        }),
      ],
    });

    expect(
      updateImageObject(content, "image-1", {
        x: 24,
        y: 36,
        width: 180,
        height: 220,
      }).layers[0].objects[0],
    ).toMatchObject({
      x: 24,
      y: 36,
      width: 180,
      height: 220,
    });
  });

  it("falls back to the first layer when the requested one is missing", () => {
    const content = createCanvasDocumentContent();

    expect(getActiveLayer(content, "missing")?.id).toBe(content.layers[0].id);
  });
});

function createCanvasLayerWithImage(image: {
  id: string;
  width: number;
  height: number;
  original_width?: number;
  original_height?: number;
}) {
  return {
    id: "layer-1",
    name: "Sketch",
    visible: true,
    locked: false,
    objects: [
      createImageObject({
        id: image.id,
        image_path: "/tmp/reference.png",
        width: image.width,
        height: image.height,
        original_width: image.original_width,
        original_height: image.original_height,
      }),
    ],
  };
}
