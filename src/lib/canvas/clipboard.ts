import type { CanvasDocumentContent, CanvasObject } from "../../types";
import { cloneCanvasDocumentContent, isCanvasLayerSelectable } from "./document";

export interface CanvasClipboardEntry {
  layerId: string;
  object: CanvasObject;
}

export interface CanvasClipboard {
  entries: CanvasClipboardEntry[];
}

export function copyCanvasObjects(
  content: CanvasDocumentContent,
  objectIds: string[],
): CanvasClipboard | null {
  const selectedIds = new Set(objectIds);
  if (!selectedIds.size) {
    return null;
  }

  const entries = content.layers.flatMap((layer) => {
    if (!isCanvasLayerSelectable(layer)) {
      return [];
    }

    return layer.objects
      .filter((object) => selectedIds.has(object.id))
      .map((object) => ({
        layerId: layer.id,
        object: cloneCanvasObject(object),
      }));
  });

  if (!entries.length) {
    return null;
  }

  return {
    entries,
  };
}

export function pasteCanvasObjects(
  content: CanvasDocumentContent,
  clipboard: CanvasClipboard | null,
  activeLayerId: string | null,
  offset = 24,
): { content: CanvasDocumentContent; pastedObjectIds: string[] } {
  const clonedContent = cloneCanvasDocumentContent(content);
  const requestedActiveLayer = clonedContent.layers.find((layer) => layer.id === activeLayerId);
  if (requestedActiveLayer && !isCanvasLayerSelectable(requestedActiveLayer)) {
    return { content: clonedContent, pastedObjectIds: [] };
  }

  const activeLayer = requestedActiveLayer;

  if (!clipboard?.entries.length) {
    return { content: clonedContent, pastedObjectIds: [] };
  }

  const pastedObjectIds: string[] = [];
  const fallbackEntries: CanvasClipboardEntry[] = [];

  clipboard.entries.forEach((entry) => {
    const originalLayer = clonedContent.layers.find((layer) => layer.id === entry.layerId);

    if (!originalLayer) {
      fallbackEntries.push(entry);
      return;
    }

    if (!isCanvasLayerSelectable(originalLayer)) {
      return;
    }

    const pastedObject = createPastedObject(entry.object, offset);
    originalLayer.objects = [...originalLayer.objects, pastedObject];
    pastedObjectIds.push(pastedObject.id);
  });

  if (activeLayer && fallbackEntries.length) {
    const pastedFallbackObjects = getEntriesInReverseLayerOrder(fallbackEntries).map((entry) =>
      createPastedObject(entry.object, offset),
    );

    activeLayer.objects = [...activeLayer.objects, ...pastedFallbackObjects];
    pastedObjectIds.push(...pastedFallbackObjects.map((object) => object.id));
  }

  return {
    content: clonedContent,
    pastedObjectIds,
  };
}

function cloneCanvasObject(object: CanvasObject): CanvasObject {
  return JSON.parse(JSON.stringify(object)) as CanvasObject;
}

function createPastedObject(object: CanvasObject, offset: number): CanvasObject {
  return offsetCanvasObject(
    {
      ...cloneCanvasObject(object),
      id: crypto.randomUUID(),
    },
    offset,
  );
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

function getEntriesInReverseLayerOrder(entries: CanvasClipboardEntry[]): CanvasClipboardEntry[] {
  const layerOrder = [...new Set(entries.map((entry) => entry.layerId))];
  const layerRank = new Map(layerOrder.map((layerId, index) => [layerId, index]));

  return [...entries].sort((first, second) => {
    const firstRank = layerRank.get(first.layerId) ?? 0;
    const secondRank = layerRank.get(second.layerId) ?? 0;
    return secondRank - firstRank;
  });
}
