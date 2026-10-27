import { isCanvasPathObject } from "./document";
import type {
  CanvasDocumentContent,
  CanvasLayer,
  CanvasObject,
} from "../../types";
import type { CanvasRect } from "./bounds";
import {
  getCanvasObjectBounds,
  getCanvasArrowHeadPoints,
  rectsIntersect,
} from "./bounds";

export interface SelectableCanvasObject {
  layer: CanvasLayer;
  object: CanvasObject;
}

export function getSelectableCanvasObjects(
  content: CanvasDocumentContent,
): SelectableCanvasObject[] {
  return content.layers.flatMap((layer) => {
    if (!layer.visible || layer.locked) return [];
    return layer.objects.map((object) => ({ layer, object }));
  });
}

export function hitTestCanvasObjectId(
  content: CanvasDocumentContent,
  point: { x: number; y: number },
  toleranceScreen = 6,
): string | null {
  for (const layer of content.layers) {
    if (!layer.visible || layer.locked) continue;

    for (let index = layer.objects.length - 1; index >= 0; index -= 1) {
      const object = layer.objects[index];
      if (isCanvasPathObject(object)) {
        const tolerance = Math.max(
          object.size / 2,
          toleranceScreen / content.viewport.scale,
        );
        const points =
          object.type === "arrow"
            ? [...object.points, ...getCanvasArrowHeadPoints(object)]
            : object.points;
        for (let i = 0; i + 3 < points.length; i += 2) {
          const ax = points[i],
            ay = points[i + 1],
            dx = points[i + 2] - ax,
            dy = points[i + 3] - ay;
          const t = Math.max(
            0,
            Math.min(
              1,
              ((point.x - ax) * dx + (point.y - ay) * dy) /
                (dx * dx + dy * dy || 1),
            ),
          );
          if (
            Math.hypot(point.x - ax - t * dx, point.y - ay - t * dy) <=
            tolerance
          )
            return object.id;
        }
        if (
          points.length === 2 &&
          Math.hypot(point.x - points[0], point.y - points[1]) <= tolerance
        )
          return object.id;
        continue;
      }
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

export function toggleSelectedObjectId(
  selectedObjectIds: string[],
  objectId: string,
): string[] {
  return selectedObjectIds.includes(objectId)
    ? selectedObjectIds.filter((selectedId) => selectedId !== objectId)
    : [...selectedObjectIds, objectId];
}

export function reconcileSelectedObjectIds(
  content: CanvasDocumentContent,
  selectedObjectIds: string[],
): string[] {
  const selectableIds = new Set(
    getSelectableCanvasObjects(content).map(({ object }) => object.id),
  );
  const reconciledIds: string[] = [];
  const seenIds = new Set<string>();

  selectedObjectIds.forEach((objectId) => {
    if (!selectableIds.has(objectId) || seenIds.has(objectId)) return;
    seenIds.add(objectId);
    reconciledIds.push(objectId);
  });

  return reconciledIds;
}
