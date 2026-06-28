import type { CanvasDocumentContent, CanvasObject, CanvasViewport } from "../../types";

export interface CanvasRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function getCanvasObjectBounds(object: CanvasObject): CanvasRect | null {
  if (object.type === "image") {
    return {
      x: object.x,
      y: object.y,
      width: object.width,
      height: object.height,
    };
  }

  if (object.points.length < 2) {
    return null;
  }

  const xs: number[] = [];
  const ys: number[] = [];
  for (let index = 0; index < object.points.length; index += 2) {
    xs.push(object.points[index]);
    ys.push(object.points[index + 1]);
  }

  const padding = object.size / 2;
  const minX = Math.min(...xs) - padding;
  const minY = Math.min(...ys) - padding;
  const maxX = Math.max(...xs) + padding;
  const maxY = Math.max(...ys) + padding;

  return {
    x: minX,
    y: minY,
    width: maxX - minX,
    height: maxY - minY,
  };
}

export function getCombinedCanvasBounds(
  content: CanvasDocumentContent,
  objectIds: string[],
): CanvasRect | null {
  const selected = new Set(objectIds);
  const bounds: CanvasRect[] = [];

  content.layers.forEach((layer) => {
    layer.objects.forEach((object) => {
      if (!selected.has(object.id)) return;
      const objectBounds = getCanvasObjectBounds(object);
      if (objectBounds) bounds.push(objectBounds);
    });
  });

  return combineCanvasRects(bounds);
}

export function combineCanvasRects(rects: CanvasRect[]): CanvasRect | null {
  if (!rects.length) return null;

  const minX = Math.min(...rects.map((rect) => rect.x));
  const minY = Math.min(...rects.map((rect) => rect.y));
  const maxX = Math.max(...rects.map((rect) => rect.x + rect.width));
  const maxY = Math.max(...rects.map((rect) => rect.y + rect.height));

  return {
    x: minX,
    y: minY,
    width: maxX - minX,
    height: maxY - minY,
  };
}

export function rectsIntersect(first: CanvasRect, second: CanvasRect): boolean {
  return (
    first.x <= second.x + second.width &&
    first.x + first.width >= second.x &&
    first.y <= second.y + second.height &&
    first.y + first.height >= second.y
  );
}

export function canvasRectToScreenRect(rect: CanvasRect, viewport: CanvasViewport): CanvasRect {
  return {
    x: rect.x * viewport.scale + viewport.x,
    y: rect.y * viewport.scale + viewport.y,
    width: rect.width * viewport.scale,
    height: rect.height * viewport.scale,
  };
}
