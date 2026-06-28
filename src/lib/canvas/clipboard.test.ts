import { describe, expect, it } from "vitest";
import { createCanvasDocumentContent, createImageObject, createStrokeObject } from "./document";
import { copyCanvasObjects, pasteCanvasObjects } from "./clipboard";

describe("canvas clipboard helpers", () => {
  it("copies selected objects in document order with source layer ids as deep copies", () => {
    const content = createClipboardContent();

    const clipboard = copyCanvasObjects(content, ["stroke-bottom", "missing", "image-top"]);

    expect(
      clipboard?.entries.map((entry) => ({
        layerId: entry.layerId,
        objectId: entry.object.id,
      })),
    ).toEqual([
      { layerId: "top", objectId: "image-top" },
      { layerId: "bottom", objectId: "stroke-bottom" },
    ]);
    expect(clipboard?.entries[0].object).toEqual(content.layers[0].objects[0]);
    expect(clipboard?.entries[0].object).not.toBe(content.layers[0].objects[0]);
    expect(copyCanvasObjects(content, [])).toBeNull();
    expect(copyCanvasObjects(content, ["missing"])).toBeNull();
  });

  it("ignores stale selected ids from locked or hidden layers when copying objects", () => {
    const content = createClipboardContent();

    const clipboard = copyCanvasObjects(content, [
      "image-top",
      "locked-image",
      "hidden-image",
    ]);

    expect(clipboard?.entries.map((entry) => entry.object.id)).toEqual(["image-top"]);
  });

  it("pastes copied objects into their original selectable layers with new ids and an offset", () => {
    const content = createClipboardContent();
    const clipboard = copyCanvasObjects(content, ["stroke-top", "image-top", "stroke-bottom"]);

    const { content: updated, pastedObjectIds } = pasteCanvasObjects(content, clipboard, "top");

    expect(pastedObjectIds).toHaveLength(3);
    expect(pastedObjectIds).not.toContain("image-top");
    expect(pastedObjectIds).not.toContain("stroke-bottom");
    expect(updated.layers[0].objects.map((object) => object.id)).toEqual([
      "image-top",
      "stroke-top",
      pastedObjectIds[0],
      pastedObjectIds[1],
    ]);
    expect(updated.layers[1].objects[2]).toMatchObject({
      type: "stroke",
      points: [29, 30, 31, 32],
    });
    expect(updated.layers[0].objects[2]).toMatchObject({
      type: "image",
      image_path: "/tmp/top.png",
      x: 34,
      y: 44,
      width: 120,
      height: 80,
    });
    expect(updated.layers[0].objects[3]).toMatchObject({
      type: "stroke",
      points: [24, 25, 26, 27],
    });
    expect(updated.layers[1].objects.map((object) => object.id)).toEqual([
      "image-bottom",
      "stroke-bottom",
      pastedObjectIds[2],
    ]);
    expect(pastedObjectIds).toEqual([
      updated.layers[0].objects[2].id,
      updated.layers[0].objects[3].id,
      updated.layers[1].objects[2].id,
    ]);
    expect(content.layers[1].objects.map((object) => object.id)).toEqual([
      "image-bottom",
      "stroke-bottom",
    ]);
  });

  it("falls back to the active layer in reverse source layer order to preserve stacking", () => {
    const sourceContent = createClipboardContent();
    const targetContent = createCanvasDocumentContent({
      layers: [
        {
          id: "target",
          name: "Target",
          visible: true,
          locked: false,
          objects: [
            createImageObject({
              id: "target-existing",
              image_path: "/tmp/target.png",
              width: 100,
              height: 100,
            }),
          ],
        },
      ],
    });
    const clipboard = copyCanvasObjects(sourceContent, ["image-top", "stroke-bottom"]);

    const { content: updated, pastedObjectIds } = pasteCanvasObjects(
      targetContent,
      clipboard,
      "target",
    );

    expect(pastedObjectIds).toEqual([
      updated.layers[0].objects[1].id,
      updated.layers[0].objects[2].id,
    ]);
    expect(updated.layers[0].objects.map((object) => object.id)).toEqual([
      "target-existing",
      ...pastedObjectIds,
    ]);
    expect(updated.layers[0].objects[1]).toMatchObject({
      type: "stroke",
      points: [29, 30, 31, 32],
    });
    expect(updated.layers[0].objects[2]).toMatchObject({
      type: "image",
      image_path: "/tmp/top.png",
      x: 34,
      y: 44,
      width: 120,
      height: 80,
    });
  });

  it("skips copied entries whose original layer now exists but is locked", () => {
    const sourceContent = createCanvasDocumentContent({
      layers: [
        {
          id: "source",
          name: "Source",
          visible: true,
          locked: false,
          objects: [
            createImageObject({
              id: "source-image",
              image_path: "/tmp/source.png",
              width: 100,
              height: 100,
            }),
          ],
        },
        {
          id: "active",
          name: "Active",
          visible: true,
          locked: false,
          objects: [],
        },
      ],
    });
    const clipboard = copyCanvasObjects(sourceContent, ["source-image"]);
    const lockedSourceContent = {
      ...sourceContent,
      layers: [
        {
          ...sourceContent.layers[0],
          locked: true,
        },
        sourceContent.layers[1],
      ],
    };

    const { content: updated, pastedObjectIds } = pasteCanvasObjects(
      lockedSourceContent,
      clipboard,
      "active",
    );

    expect(pastedObjectIds).toEqual([]);
    expect(updated.layers[0].objects.map((object) => object.id)).toEqual(["source-image"]);
    expect(updated.layers[1].objects).toEqual([]);
  });

  it("skips copied entries whose original layer now exists but is hidden", () => {
    const sourceContent = createCanvasDocumentContent({
      layers: [
        {
          id: "source",
          name: "Source",
          visible: true,
          locked: false,
          objects: [
            createImageObject({
              id: "source-image",
              image_path: "/tmp/source.png",
              width: 100,
              height: 100,
            }),
          ],
        },
        {
          id: "active",
          name: "Active",
          visible: true,
          locked: false,
          objects: [],
        },
      ],
    });
    const clipboard = copyCanvasObjects(sourceContent, ["source-image"]);
    const hiddenSourceContent = {
      ...sourceContent,
      layers: [
        {
          ...sourceContent.layers[0],
          visible: false,
        },
        sourceContent.layers[1],
      ],
    };

    const { content: updated, pastedObjectIds } = pasteCanvasObjects(
      hiddenSourceContent,
      clipboard,
      "active",
    );

    expect(pastedObjectIds).toEqual([]);
    expect(updated.layers[0].objects.map((object) => object.id)).toEqual(["source-image"]);
    expect(updated.layers[1].objects).toEqual([]);
  });

  it("returns cloned content and no pasted ids when clipboard is missing", () => {
    const content = createClipboardContent();

    const noClipboardResult = pasteCanvasObjects(content, null, "bottom");

    expect(noClipboardResult.pastedObjectIds).toEqual([]);
    expect(noClipboardResult.content).toEqual(content);
    expect(noClipboardResult.content).not.toBe(content);
  });

  it("no-ops stale clipboard entries when the active fallback layer is locked or hidden", () => {
    const sourceContent = createClipboardContent();
    const clipboard = copyCanvasObjects(sourceContent, ["image-top"]);
    const lockedTargetContent = createCanvasDocumentContent({
      layers: [
        {
          id: "target",
          name: "Target",
          visible: true,
          locked: true,
          objects: [],
        },
      ],
    });
    const hiddenTargetContent = createCanvasDocumentContent({
      layers: [
        {
          id: "target",
          name: "Target",
          visible: false,
          locked: false,
          objects: [],
        },
      ],
    });

    const lockedResult = pasteCanvasObjects(lockedTargetContent, clipboard, "target");
    const hiddenResult = pasteCanvasObjects(hiddenTargetContent, clipboard, "target");

    expect(lockedResult.pastedObjectIds).toEqual([]);
    expect(lockedResult.content).toEqual(lockedTargetContent);
    expect(lockedResult.content).not.toBe(lockedTargetContent);
    expect(hiddenResult.pastedObjectIds).toEqual([]);
    expect(hiddenResult.content).toEqual(hiddenTargetContent);
    expect(hiddenResult.content).not.toBe(hiddenTargetContent);
  });

  it("no-ops paste when the requested active layer is locked or hidden", () => {
    const content = createClipboardContent();
    const clipboard = copyCanvasObjects(content, ["image-top"]);

    const lockedResult = pasteCanvasObjects(content, clipboard, "locked");
    const hiddenResult = pasteCanvasObjects(content, clipboard, "hidden");

    expect(lockedResult.pastedObjectIds).toEqual([]);
    expect(lockedResult.content).toEqual(content);
    expect(lockedResult.content).not.toBe(content);
    expect(hiddenResult.pastedObjectIds).toEqual([]);
    expect(hiddenResult.content).toEqual(content);
    expect(hiddenResult.content).not.toBe(content);
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
      {
        id: "locked",
        name: "Locked",
        visible: true,
        locked: true,
        objects: [
          createImageObject({
            id: "locked-image",
            image_path: "/tmp/locked.png",
            x: 300,
            y: 400,
            width: 80,
            height: 40,
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
            x: 500,
            y: 600,
            width: 80,
            height: 40,
          }),
        ],
      },
    ],
  });
}
