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
