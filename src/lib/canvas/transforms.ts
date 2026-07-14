import type { CanvasDocumentContent } from "../../types";
import { cloneCanvasDocumentContent, isCanvasLayerSelectable } from "./document";

export function translateCanvasObjects(
  content: CanvasDocumentContent,
  objectIds: string[],
  offset: { dx: number; dy: number },
): CanvasDocumentContent {
  const selectedIds = new Set(objectIds);
  const clonedContent = cloneCanvasDocumentContent(content);

  return {
    ...clonedContent,
    layers: clonedContent.layers.map((layer) => ({
      ...layer,
      objects: isCanvasLayerSelectable(layer)
        ? layer.objects.map((object) => {
            if (!selectedIds.has(object.id)) {
              return object;
            }

            if (object.type === "image") {
              return {
                ...object,
                x: object.x + offset.dx,
                y: object.y + offset.dy,
              };
            }

            return {
              ...object,
              points: object.points.map((point, index) =>
                index % 2 === 0 ? point + offset.dx : point + offset.dy,
              ),
            };
          })
        : layer.objects,
    })),
  };
}
