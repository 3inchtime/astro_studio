import { describe, expect, it } from "vitest";
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
    expect(hitTestCanvasObjectId(content, { x: 480, y: 480 })).toBeNull();
  });

  it("selects objects whose bounds intersect a marquee rectangle", () => {
    expect(
      selectCanvasObjectsInRect(content, { x: 15, y: 15, width: 150, height: 120 }),
    ).toEqual(["image-top", "image-bottom"]);
  });

  it("toggles and reconciles selected ids", () => {
    expect(toggleSelectedObjectId(["image-top"], "image-bottom")).toEqual([
      "image-top",
      "image-bottom",
    ]);
    expect(toggleSelectedObjectId(["image-top", "image-bottom"], "image-top")).toEqual([
      "image-bottom",
    ]);
    expect(reconcileSelectedObjectIds(content, ["image-top", "missing", "locked-image"])).toEqual([
      "image-top",
    ]);
  });
});
