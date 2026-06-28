import type { CanvasDocumentContent, CanvasObject } from "../../types";
import { cloneCanvasDocumentContent } from "./document";

export type CanvasOrderDirection = "forward" | "backward" | "front" | "back";

export function reorderCanvasObjects(
  content: CanvasDocumentContent,
  objectIds: string[],
  direction: CanvasOrderDirection,
): CanvasDocumentContent {
  const selectedIds = new Set(objectIds);
  const clonedContent = cloneCanvasDocumentContent(content);

  return {
    ...clonedContent,
    layers: clonedContent.layers.map((layer) => ({
      ...layer,
      objects: reorderLayerObjects(layer.objects, selectedIds, direction),
    })),
  };
}

function reorderLayerObjects(
  objects: CanvasObject[],
  selectedIds: Set<string>,
  direction: CanvasOrderDirection,
): CanvasObject[] {
  if (direction === "front") {
    const selected = objects.filter((object) => selectedIds.has(object.id));
    const unselected = objects.filter((object) => !selectedIds.has(object.id));
    return [...unselected, ...selected];
  }

  if (direction === "back") {
    const selected = objects.filter((object) => selectedIds.has(object.id));
    const unselected = objects.filter((object) => !selectedIds.has(object.id));
    return [...selected, ...unselected];
  }

  return direction === "forward"
    ? moveSelectedForward(objects, selectedIds)
    : moveSelectedBackward(objects, selectedIds);
}

function moveSelectedForward(objects: CanvasObject[], selectedIds: Set<string>): CanvasObject[] {
  const reordered = [...objects];

  for (let index = reordered.length - 2; index >= 0; index -= 1) {
    const current = reordered[index];
    const next = reordered[index + 1];
    if (!selectedIds.has(current.id) || selectedIds.has(next.id)) {
      continue;
    }
    reordered[index] = next;
    reordered[index + 1] = current;
  }

  return reordered;
}

function moveSelectedBackward(objects: CanvasObject[], selectedIds: Set<string>): CanvasObject[] {
  const reordered = [...objects];

  for (let index = 1; index < reordered.length; index += 1) {
    const current = reordered[index];
    const previous = reordered[index - 1];
    if (!selectedIds.has(current.id) || selectedIds.has(previous.id)) {
      continue;
    }
    reordered[index] = previous;
    reordered[index - 1] = current;
  }

  return reordered;
}
