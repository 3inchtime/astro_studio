import type { CanvasDocumentContent, CanvasObject, CanvasViewport } from "../../types";

export interface CanvasRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface CanvasScreenRect extends CanvasRect {
  readonly __space: "screen";
}

export function getCanvasObjectBounds(object: CanvasObject): CanvasRect | null {
  if (object.type === "image") {
    if (object.rotation === 0) {
      return {
        x: object.x,
        y: object.y,
        width: object.width,
        height: object.height,
      };
    }

    const radians = (object.rotation * Math.PI) / 180;
    const cos = normalizeFloatingPoint(Math.cos(radians));
    const sin = normalizeFloatingPoint(Math.sin(radians));
    const corners = [
      { x: object.x, y: object.y },
      rotateImageCorner(object.x + object.width, object.y, object.x, object.y, cos, sin),
      rotateImageCorner(
        object.x + object.width,
        object.y + object.height,
        object.x,
        object.y,
        cos,
        sin,
      ),
      rotateImageCorner(object.x, object.y + object.height, object.x, object.y, cos, sin),
    ];
    const xs = corners.map((corner) => corner.x);
    const ys = corners.map((corner) => corner.y);
    const minX = Math.min(...xs);
    const minY = Math.min(...ys);
    const maxX = Math.max(...xs);
    const maxY = Math.max(...ys);

    return {
      x: minX,
      y: minY,
      width: maxX - minX,
      height: maxY - minY,
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

export function canvasRectToScreenRect(
  rect: CanvasRect,
  viewport: CanvasViewport,
): CanvasScreenRect {
  return {
    x: rect.x * viewport.scale + viewport.x,
    y: rect.y * viewport.scale + viewport.y,
    width: rect.width * viewport.scale,
    height: rect.height * viewport.scale,
    __space: "screen",
  };
}

function rotateImageCorner(
  x: number,
  y: number,
  originX: number,
  originY: number,
  cos: number,
  sin: number,
): Pick<CanvasRect, "x" | "y"> {
  const dx = x - originX;
  const dy = y - originY;

  return {
    x: originX + dx * cos - dy * sin,
    y: originY + dx * sin + dy * cos,
  };
}

function normalizeFloatingPoint(value: number): number {
  return Math.abs(value) < Number.EPSILON ? 0 : value;
}
