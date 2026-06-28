import { describe, expect, it } from "vitest";
import { createCanvasDocumentContent, createImageObject, createStrokeObject } from "./document";
import { copyCanvasObjects, pasteCanvasObjects } from "./clipboard";

describe("canvas clipboard helpers", () => {
  it("copies selected objects in document order as deep copies", () => {
    const content = createClipboardContent();

    const clipboard = copyCanvasObjects(content, ["stroke-bottom", "missing", "image-top"]);

    expect(clipboard?.objects.map((object) => object.id)).toEqual(["image-top", "stroke-bottom"]);
    expect(clipboard?.objects[0]).toEqual(content.layers[0].objects[0]);
    expect(clipboard?.objects[0]).not.toBe(content.layers[0].objects[0]);
    expect(copyCanvasObjects(content, [])).toBeNull();
    expect(copyCanvasObjects(content, ["missing"])).toBeNull();
  });

  it("pastes copied objects into the active layer with new ids and an offset", () => {
    const content = createClipboardContent();
    const clipboard = copyCanvasObjects(content, ["image-top", "stroke-bottom"]);

    const { content: updated, pastedObjectIds } = pasteCanvasObjects(content, clipboard, "bottom");

    expect(pastedObjectIds).toHaveLength(2);
    expect(pastedObjectIds).not.toContain("image-top");
    expect(pastedObjectIds).not.toContain("stroke-bottom");
    expect(updated.layers[1].objects.map((object) => object.id)).toEqual([
      "image-bottom",
      "stroke-bottom",
      ...pastedObjectIds,
    ]);
    expect(updated.layers[1].objects[2]).toMatchObject({
      type: "image",
      image_path: "/tmp/top.png",
      x: 34,
      y: 44,
      width: 120,
      height: 80,
    });
    expect(updated.layers[1].objects[3]).toMatchObject({
      type: "stroke",
      points: [29, 30, 31, 32],
    });
    expect(content.layers[1].objects.map((object) => object.id)).toEqual([
      "image-bottom",
      "stroke-bottom",
    ]);
  });

  it("returns cloned content and no pasted ids when clipboard or active layer is missing", () => {
    const content = createClipboardContent();
    const clipboard = copyCanvasObjects(content, ["image-top"]);

    const noClipboardResult = pasteCanvasObjects(content, null, "bottom");
    const missingLayerResult = pasteCanvasObjects(content, clipboard, "missing");

    expect(noClipboardResult.pastedObjectIds).toEqual([]);
    expect(noClipboardResult.content).toEqual(content);
    expect(noClipboardResult.content).not.toBe(content);
    expect(missingLayerResult.pastedObjectIds).toEqual([]);
    expect(missingLayerResult.content).toEqual(content);
    expect(missingLayerResult.content).not.toBe(content);
  });
});

function createClipboardContent() {
  return createCanvasDocumentContent({
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
            x: 10,
            y: 20,
            width: 120,
            height: 80,
          }),
          createStrokeObject({
            id: "stroke-top",
            points: [0, 1, 2, 3],
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
            x: 100,
            y: 200,
            width: 80,
            height: 40,
          }),
          createStrokeObject({
            id: "stroke-bottom",
            points: [5, 6, 7, 8],
          }),
        ],
      },
    ],
  });
}
