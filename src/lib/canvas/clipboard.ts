import type { CanvasDocumentContent, CanvasObject } from "../../types";
import { cloneCanvasDocumentContent } from "./document";

export interface CanvasClipboard {
  objects: CanvasObject[];
}

export function copyCanvasObjects(
  content: CanvasDocumentContent,
  objectIds: string[],
): CanvasClipboard | null {
  const selectedIds = new Set(objectIds);
  if (!selectedIds.size) {
    return null;
  }

  const objects = content.layers.flatMap((layer) =>
    layer.objects.filter((object) => selectedIds.has(object.id)),
  );

  if (!objects.length) {
    return null;
  }

  return {
    objects: cloneCanvasObjects(objects),
  };
}

export function pasteCanvasObjects(
  content: CanvasDocumentContent,
  clipboard: CanvasClipboard | null,
  activeLayerId: string | null,
  offset = 24,
): { content: CanvasDocumentContent; pastedObjectIds: string[] } {
  const clonedContent = cloneCanvasDocumentContent(content);
  const activeLayer = clonedContent.layers.find((layer) => layer.id === activeLayerId);

  if (!clipboard?.objects.length || !activeLayer) {
    return { content: clonedContent, pastedObjectIds: [] };
  }

  const pastedObjects = cloneCanvasObjects(clipboard.objects).map((object) =>
    offsetCanvasObject(
      {
        ...object,
        id: crypto.randomUUID(),
      },
      offset,
    ),
  );

  activeLayer.objects = [...activeLayer.objects, ...pastedObjects];

  return {
    content: clonedContent,
    pastedObjectIds: pastedObjects.map((object) => object.id),
  };
}

function cloneCanvasObjects(objects: CanvasObject[]): CanvasObject[] {
  return JSON.parse(JSON.stringify(objects)) as CanvasObject[];
}

function offsetCanvasObject(object: CanvasObject, offset: number): CanvasObject {
  if (object.type === "image") {
    return {
      ...object,
      x: object.x + offset,
      y: object.y + offset,
    };
  }

  return {
    ...object,
    points: object.points.map((point) => point + offset),
  };
}
