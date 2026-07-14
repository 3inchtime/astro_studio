import { describe, expect, it } from "vitest";
import { createCanvasDocumentContent, createImageObject, createStrokeObject } from "./document";
import { translateCanvasObjects } from "./transforms";

describe("canvas transform helpers", () => {
  it("translates selected image positions and stroke points across layers", () => {
    const content = createCanvasDocumentContent({
      layers: [
        {
          id: "top",
          name: "Top",
          visible: true,
          locked: false,
          objects: [
            createImageObject({
              id: "image-selected",
              image_path: "/tmp/selected.png",
              x: 10,
              y: 20,
              width: 120,
              height: 80,
            }),
            createImageObject({
              id: "image-still",
              image_path: "/tmp/still.png",
              x: 100,
              y: 200,
              width: 80,
              height: 40,
            }),
          ],
        },
        {
          id: "bottom",
          name: "Bottom",
          visible: true,
          locked: false,
          objects: [
            createStrokeObject({
              id: "stroke-selected",
              points: [0, 10, 20, 30, 40, 50],
            }),
            createStrokeObject({
              id: "stroke-still",
              points: [5, 6, 7, 8],
            }),
          ],
        },
      ],
    });

    const updated = translateCanvasObjects(content, ["image-selected", "stroke-selected"], {
      dx: 12,
      dy: -8,
    });

    expect(updated.layers[0].objects[0]).toMatchObject({ x: 22, y: 12 });
    expect(updated.layers[0].objects[1]).toMatchObject({ x: 100, y: 200 });
    expect(updated.layers[1].objects[0]).toMatchObject({ points: [12, 2, 32, 22, 52, 42] });
    expect(updated.layers[1].objects[1]).toMatchObject({ points: [5, 6, 7, 8] });
    expect(content.layers[1].objects[0]).toMatchObject({ points: [0, 10, 20, 30, 40, 50] });
  });

  it("ignores stale selected ids from locked or hidden layers when translating objects", () => {
    const content = createCanvasDocumentContent({
      layers: [
        {
          id: "editable",
          name: "Editable",
          visible: true,
          locked: false,
          objects: [
            createImageObject({
              id: "editable-image",
              image_path: "/tmp/editable.png",
              x: 10,
              y: 20,
              width: 100,
              height: 100,
            }),
          ],
        },
        {
          id: "locked",
          name: "Locked",
          visible: true,
          locked: true,
          objects: [
            createImageObject({
              id: "locked-image",
              image_path: "/tmp/locked.png",
              x: 30,
              y: 40,
              width: 100,
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
            createStrokeObject({
              id: "hidden-stroke",
              points: [5, 6, 7, 8],
            }),
          ],
        },
      ],
    });

    const updated = translateCanvasObjects(
      content,
      ["editable-image", "locked-image", "hidden-stroke"],
      { dx: 10, dy: 12 },
    );

    expect(updated.layers[0].objects[0]).toMatchObject({ x: 20, y: 32 });
    expect(updated.layers[1].objects[0]).toMatchObject({ x: 30, y: 40 });
    expect(updated.layers[2].objects[0]).toMatchObject({ points: [5, 6, 7, 8] });
  });
});
