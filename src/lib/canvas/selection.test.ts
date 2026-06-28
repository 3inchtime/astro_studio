import { describe, expect, it } from "vitest";
import { canvasRectToScreenRect } from "./bounds";
import { createCanvasDocumentContent, createImageObject, createStrokeObject } from "./document";
import {
  getSelectableCanvasObjects,
  hitTestCanvasObjectId,
  reconcileSelectedObjectIds,
  selectCanvasObjectsInRect,
  toggleSelectedObjectId,
} from "./selection";

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
          x: 20,
          y: 20,
          width: 120,
          height: 80,
        }),
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
          x: 30,
          y: 30,
          width: 120,
          height: 80,
        }),
        createStrokeObject({
          id: "stroke-1",
          size: 8,
          points: [260, 260, 310, 300],
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
          x: 0,
          y: 0,
          width: 500,
          height: 500,
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
          x: 520,
          y: 520,
          width: 100,
          height: 100,
        }),
      ],
    },
  ],
});

const sameLayerContent = createCanvasDocumentContent({
  layers: [
    {
      id: "overlap",
      name: "Overlap",
      visible: true,
      locked: false,
      objects: [
        createImageObject({
          id: "same-layer-earlier",
          image_path: "/tmp/earlier.png",
          x: 10,
          y: 10,
          width: 100,
          height: 100,
        }),
        createImageObject({
          id: "same-layer-later",
          image_path: "/tmp/later.png",
          x: 40,
          y: 40,
          width: 100,
          height: 100,
        }),
      ],
    },
  ],
});

describe("canvas selection helpers", () => {
  it("returns selectable objects from visible unlocked layers only", () => {
    expect(getSelectableCanvasObjects(content).map((entry) => entry.object.id)).toEqual([
      "image-top",
      "image-bottom",
      "stroke-1",
    ]);
  });

  it("hit-tests in reverse visual order", () => {
    expect(hitTestCanvasObjectId(content, { x: 40, y: 40 })).toBe("image-top");
    expect(hitTestCanvasObjectId(content, { x: 285, y: 280 })).toBe("stroke-1");
    expect(hitTestCanvasObjectId(content, { x: 540, y: 540 })).toBeNull();
    expect(hitTestCanvasObjectId(content, { x: 480, y: 480 })).toBeNull();
  });

  it("prioritizes later objects within the same layer when hit-testing", () => {
    expect(hitTestCanvasObjectId(sameLayerContent, { x: 50, y: 50 })).toBe("same-layer-later");
  });

  it("selects objects whose bounds intersect a marquee rectangle", () => {
    expect(
      selectCanvasObjectsInRect(content, { x: 15, y: 15, width: 150, height: 120 }),
    ).toEqual(["image-top", "image-bottom"]);
    expect(
      selectCanvasObjectsInRect(content, { x: 515, y: 515, width: 120, height: 120 }),
    ).toEqual([]);
  });

  it("toggles and reconciles selected ids", () => {
    expect(toggleSelectedObjectId(["image-top"], "image-bottom")).toEqual([
      "image-top",
      "image-bottom",
    ]);
    expect(toggleSelectedObjectId(["image-top", "image-bottom"], "image-top")).toEqual([
      "image-bottom",
    ]);
    expect(
      reconcileSelectedObjectIds(content, [
        "image-top",
        "missing",
        "locked-image",
        "hidden-image",
      ]),
    ).toEqual(["image-top"]);
    expect(
      reconcileSelectedObjectIds(content, [
        "image-top",
        "image-top",
        "missing",
        "image-bottom",
      ]),
    ).toEqual(["image-top", "image-bottom"]);
  });

  it("rejects screen-space rectangles for canvas-space selection at compile time", () => {
    const screenRect = canvasRectToScreenRect(
      { x: 0, y: 0, width: 10, height: 10 },
      { x: 0, y: 0, scale: 1 },
    );

    // @ts-expect-error screen-space rectangles must not be accepted for canvas-space selection.
    expect(selectCanvasObjectsInRect(content, screenRect)).toEqual([]);
  });
});
