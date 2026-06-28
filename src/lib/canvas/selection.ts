import type { CanvasDocumentContent, CanvasLayer, CanvasObject } from "../../types";
import type { CanvasRect } from "./bounds";
import { getCanvasObjectBounds, rectsIntersect } from "./bounds";

export interface SelectableCanvasObject {
  layer: CanvasLayer;
  object: CanvasObject;
}

export function getSelectableCanvasObjects(content: CanvasDocumentContent): SelectableCanvasObject[] {
  return content.layers.flatMap((layer) => {
    if (!layer.visible || layer.locked) return [];
    return layer.objects.map((object) => ({ layer, object }));
  });
}

export function hitTestCanvasObjectId(
  content: CanvasDocumentContent,
  point: { x: number; y: number },
): string | null {
  for (const layer of content.layers) {
    if (!layer.visible || layer.locked) continue;

    for (let index = layer.objects.length - 1; index >= 0; index -= 1) {
      const object = layer.objects[index];
      const bounds = getCanvasObjectBounds(object);
      if (
        bounds &&
        point.x >= bounds.x &&
        point.x <= bounds.x + bounds.width &&
        point.y >= bounds.y &&
        point.y <= bounds.y + bounds.height
      ) {
        return object.id;
      }
    }
  }
  return null;
}

export function selectCanvasObjectsInRect(
  content: CanvasDocumentContent,
  rect: CanvasRect,
): string[] {
  return getSelectableCanvasObjects(content)
    .filter(({ object }) => {
      const bounds = getCanvasObjectBounds(object);
      return bounds ? rectsIntersect(rect, bounds) : false;
    })
    .map(({ object }) => object.id);
}

export function toggleSelectedObjectId(selectedObjectIds: string[], objectId: string): string[] {
  return selectedObjectIds.includes(objectId)
    ? selectedObjectIds.filter((selectedId) => selectedId !== objectId)
    : [...selectedObjectIds, objectId];
}

export function reconcileSelectedObjectIds(
  content: CanvasDocumentContent,
  selectedObjectIds: string[],
): string[] {
  const selectableIds = new Set(getSelectableCanvasObjects(content).map(({ object }) => object.id));
  return selectedObjectIds.filter((objectId) => selectableIds.has(objectId));
}
