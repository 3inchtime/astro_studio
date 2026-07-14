import { describe, expect, it } from "vitest";
import {
  createCanvasDocumentContent,
  createImageObject,
  getCanvasLayersBackToFront,
  removeCanvasObjects,
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

  it("ignores stale image ids from locked or hidden layers when updating image objects", () => {
    const content = createCanvasDocumentContent({
      layers: [
        {
          id: "locked",
          name: "Locked",
          visible: true,
          locked: true,
          objects: [
            createImageObject({
              id: "locked-image",
              image_path: "/tmp/locked.png",
              x: 10,
              y: 20,
              width: 200,
              height: 100,
            }),
          ],
        },
        {
          id: "hidden",
          name: "Hidden",
          visible: false,
          locked: false,
          objects: [
            createImageObject({
              id: "hidden-image",
              image_path: "/tmp/hidden.png",
              x: 30,
              y: 40,
              width: 240,
              height: 120,
            }),
          ],
        },
      ],
    });

    const updatedLocked = updateImageObject(content, "locked-image", {
      x: 100,
      y: 120,
      width: 80,
      height: 90,
    });
    const updatedHidden = updateImageObject(content, "hidden-image", {
      x: 140,
      y: 160,
      width: 80,
      height: 90,
    });

    expect(updatedLocked.layers[0].objects[0]).toMatchObject({
      x: 10,
      y: 20,
      width: 200,
      height: 100,
    });
    expect(updatedHidden.layers[1].objects[0]).toMatchObject({
      x: 30,
      y: 40,
      width: 240,
      height: 120,
    });
  });

  it("ignores stale image ids from locked or hidden layers when resetting image aspect", () => {
    const content = createCanvasDocumentContent({
      layers: [
        {
          id: "locked",
          name: "Locked",
          visible: true,
          locked: true,
          objects: [
            createImageObject({
              id: "locked-image",
              image_path: "/tmp/locked.png",
              width: 400,
              height: 400,
              original_width: 300,
              original_height: 150,
            }),
          ],
        },
        {
          id: "hidden",
          name: "Hidden",
          visible: false,
          locked: false,
          objects: [
            createImageObject({
              id: "hidden-image",
              image_path: "/tmp/hidden.png",
              width: 500,
              height: 500,
              original_width: 250,
              original_height: 100,
            }),
          ],
        },
      ],
    });

    const resetLocked = resetImageObjectAspect(content, "locked-image");
    const resetHidden = resetImageObjectAspect(content, "hidden-image");

    expect(resetLocked.layers[0].objects[0]).toMatchObject({
      width: 400,
      height: 400,
    });
    expect(resetHidden.layers[1].objects[0]).toMatchObject({
      width: 500,
      height: 500,
    });
  });

  it("removes selected objects across all layers without changing other layers or objects", () => {
    const content = createCanvasDocumentContent({
      layers: [
        {
          id: "top",
          name: "Top",
          visible: true,
          locked: false,
          objects: [
            createImageObject({
              id: "image-top",
              image_path: "/tmp/top.png",
              width: 120,
              height: 80,
            }),
            createStrokeObject({ id: "stroke-top", points: [0, 0, 20, 20] }),
          ],
        },
        {
          id: "bottom",
          name: "Bottom",
          visible: true,
          locked: false,
          objects: [
            createImageObject({
              id: "image-bottom",
              image_path: "/tmp/bottom.png",
              width: 200,
              height: 100,
            }),
            createStrokeObject({ id: "stroke-bottom", points: [10, 10, 40, 40] }),
          ],
        },
      ],
    });

    const updated = removeCanvasObjects(content, ["stroke-top", "image-bottom", "missing"]);

    expect(updated.layers.map((layer) => layer.id)).toEqual(["top", "bottom"]);
    expect(updated.layers[0].objects.map((object) => object.id)).toEqual(["image-top"]);
    expect(updated.layers[1].objects.map((object) => object.id)).toEqual(["stroke-bottom"]);
    expect(content.layers[0].objects.map((object) => object.id)).toEqual([
      "image-top",
      "stroke-top",
    ]);
  });

  it("ignores stale selected ids from locked or hidden layers when removing objects", () => {
    const content = createCanvasDocumentContent({
      layers: [
        {
          id: "editable",
          name: "Editable",
          visible: true,
          locked: false,
          objects: [createStrokeObject({ id: "editable-stroke", points: [0, 0, 10, 10] })],
        },
        {
          id: "locked",
          name: "Locked",
          visible: true,
          locked: true,
          objects: [createStrokeObject({ id: "locked-stroke", points: [20, 20, 30, 30] })],
        },
        {
          id: "hidden",
          name: "Hidden",
          visible: false,
          locked: false,
          objects: [createStrokeObject({ id: "hidden-stroke", points: [40, 40, 50, 50] })],
        },
      ],
    });

    const updated = removeCanvasObjects(content, [
      "editable-stroke",
      "locked-stroke",
      "hidden-stroke",
    ]);

    expect(updated.layers[0].objects.map((object) => object.id)).toEqual([]);
    expect(updated.layers[1].objects.map((object) => object.id)).toEqual(["locked-stroke"]);
    expect(updated.layers[2].objects.map((object) => object.id)).toEqual(["hidden-stroke"]);
  });

  it("falls back to the first layer when the requested one is missing", () => {
    const content = createCanvasDocumentContent();

    expect(getActiveLayer(content, "missing")?.id).toBe(content.layers[0].id);
  });

  it("returns layers back-to-front without mutating the document order", () => {
    const layers = [
      createCanvasLayerForOrder("top"),
      createCanvasLayerForOrder("middle"),
      createCanvasLayerForOrder("bottom"),
    ];

    const orderedLayers = getCanvasLayersBackToFront(layers);

    expect(orderedLayers.map((layer) => layer.id)).toEqual(["bottom", "middle", "top"]);
    expect(layers.map((layer) => layer.id)).toEqual(["top", "middle", "bottom"]);
    expect(orderedLayers[0]).toBe(layers[2]);
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

function createCanvasLayerForOrder(id: string) {
  return {
    id,
    name: id,
    visible: true,
    locked: false,
    objects: [],
  };
}
