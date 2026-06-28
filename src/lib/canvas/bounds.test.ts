import { describe, expect, it } from "vitest";
import { createCanvasDocumentContent, createImageObject, createStrokeObject } from "./document";
import {
  canvasRectToScreenRect,
  getCanvasObjectBounds,
  getCombinedCanvasBounds,
  rectsIntersect,
} from "./bounds";

describe("canvas bounds helpers", () => {
  it("computes image bounds from position and dimensions", () => {
    const image = createImageObject({
      id: "image-1",
      image_path: "/tmp/a.png",
      x: 20,
      y: 30,
      width: 200,
      height: 120,
    });

    expect(getCanvasObjectBounds(image)).toEqual({
      x: 20,
      y: 30,
      width: 200,
      height: 120,
    });
  });

  it("computes rotated image visual bounds around the image origin", () => {
    const image = createImageObject({
      id: "image-rotated",
      image_path: "/tmp/rotated.png",
      x: 10,
      y: 20,
      width: 100,
      height: 50,
      rotation: 90,
    });

    expect(getCanvasObjectBounds(image)).toEqual({
      x: -40,
      y: 20,
      width: 50,
      height: 100,
    });
  });

  it("computes stroke bounds with stroke size padding", () => {
    const stroke = createStrokeObject({
      id: "stroke-1",
      size: 12,
      points: [10, 40, 60, 20, 90, 80],
    });

    expect(getCanvasObjectBounds(stroke)).toEqual({
      x: 4,
      y: 14,
      width: 92,
      height: 72,
    });
  });

  it("combines selected object bounds across layers", () => {
    const content = createCanvasDocumentContent({
      layers: [
        {
          id: "layer-1",
          name: "Sketch",
          visible: true,
          locked: false,
          objects: [
            createImageObject({
              id: "image-1",
              image_path: "/tmp/a.png",
              x: 20,
              y: 30,
              width: 200,
              height: 120,
            }),
            createStrokeObject({
              id: "stroke-1",
              size: 10,
              points: [300, 250, 360, 290],
            }),
          ],
        },
      ],
    });

    expect(getCombinedCanvasBounds(content, ["image-1", "stroke-1"])).toEqual({
      x: 20,
      y: 30,
      width: 345,
      height: 265,
    });
    expect(getCombinedCanvasBounds(content, ["missing"])).toBeNull();
  });

  it("checks rectangle intersection and projects to screen space", () => {
    const screenRect = canvasRectToScreenRect(
      { x: 10, y: 20, width: 30, height: 40 },
      { x: 100, y: 200, scale: 2 },
    );

    expect(
      rectsIntersect(
        { x: 0, y: 0, width: 100, height: 100 },
        { x: 90, y: 90, width: 20, height: 20 },
      ),
    ).toBe(true);
    expect(
      rectsIntersect(
        { x: 0, y: 0, width: 100, height: 100 },
        { x: 120, y: 120, width: 20, height: 20 },
      ),
    ).toBe(false);
    expect(screenRect).toMatchObject({ x: 120, y: 240, width: 60, height: 80 });
    expect(screenRect.__space).toBe("screen");
  });
});
