import { describe, expect, it } from "vitest";
import { createCanvasDocumentContent, createImageObject } from "./document";
import { reorderCanvasObjects } from "./ordering";

describe("canvas ordering helpers", () => {
  it("moves selected objects forward one step within each layer", () => {
    const updated = reorderCanvasObjects(createOrderingContent(), ["b", "c", "g"], "forward");

    expect(getObjectIds(updated, "top")).toEqual(["a", "d", "b", "c", "e"]);
    expect(getObjectIds(updated, "bottom")).toEqual(["f", "h", "g"]);
  });

  it("moves selected objects backward one step within each layer", () => {
    const updated = reorderCanvasObjects(createOrderingContent(), ["b", "c", "g"], "backward");

    expect(getObjectIds(updated, "top")).toEqual(["b", "c", "a", "d", "e"]);
    expect(getObjectIds(updated, "bottom")).toEqual(["g", "f", "h"]);
  });

  it("moves selected objects to the front of their layer", () => {
    const updated = reorderCanvasObjects(createOrderingContent(), ["b", "d", "g"], "front");

    expect(getObjectIds(updated, "top")).toEqual(["a", "c", "e", "b", "d"]);
    expect(getObjectIds(updated, "bottom")).toEqual(["f", "h", "g"]);
  });

  it("moves selected objects to the back of their layer", () => {
    const updated = reorderCanvasObjects(createOrderingContent(), ["b", "d", "g"], "back");

    expect(getObjectIds(updated, "top")).toEqual(["b", "d", "a", "c", "e"]);
    expect(getObjectIds(updated, "bottom")).toEqual(["g", "f", "h"]);
  });

  it("ignores stale selected ids from locked or hidden layers when reordering objects", () => {
    const content = createCanvasDocumentContent({
      layers: [
        {
          id: "editable",
          name: "Editable",
          visible: true,
          locked: false,
          objects: ["a", "b", "c"].map(createObject),
        },
        {
          id: "locked",
          name: "Locked",
          visible: true,
          locked: true,
          objects: ["locked-a", "locked-b", "locked-c"].map(createObject),
        },
        {
          id: "hidden",
          name: "Hidden",
          visible: false,
          locked: false,
          objects: ["hidden-a", "hidden-b", "hidden-c"].map(createObject),
        },
      ],
    });

    const updated = reorderCanvasObjects(
      content,
      ["b", "locked-a", "locked-b", "hidden-a", "hidden-b"],
      "front",
    );

    expect(getObjectIds(updated, "editable")).toEqual(["a", "c", "b"]);
    expect(getObjectIds(updated, "locked")).toEqual(["locked-a", "locked-b", "locked-c"]);
    expect(getObjectIds(updated, "hidden")).toEqual(["hidden-a", "hidden-b", "hidden-c"]);
  });
});

function createOrderingContent() {
  return createCanvasDocumentContent({
    layers: [
      {
        id: "top",
        name: "Top",
        visible: true,
        locked: false,
        objects: ["a", "b", "c", "d", "e"].map(createObject),
      },
      {
        id: "bottom",
        name: "Bottom",
        visible: true,
        locked: false,
        objects: ["f", "g", "h"].map(createObject),
      },
    ],
  });
}

function createObject(id: string) {
  return createImageObject({
    id,
    image_path: `/tmp/${id}.png`,
    width: 100,
    height: 100,
  });
}

function getObjectIds(content: ReturnType<typeof createOrderingContent>, layerId: string): string[] {
  return content.layers.find((layer) => layer.id === layerId)?.objects.map((object) => object.id) ?? [];
}
