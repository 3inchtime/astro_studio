import type { CanvasFrame, CanvasViewport } from "../../types";

export interface CanvasScreenRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function frameToScreenRect(
  frame: CanvasFrame,
  viewport: CanvasViewport,
): CanvasScreenRect {
  return {
    x: frame.x * viewport.scale + viewport.x,
    y: frame.y * viewport.scale + viewport.y,
    width: frame.width * viewport.scale,
    height: frame.height * viewport.scale,
  };
}

export function screenPointToCanvasPoint(
  point: { x: number; y: number },
  viewport: CanvasViewport,
): { x: number; y: number } {
  return {
    x: (point.x - viewport.x) / viewport.scale,
    y: (point.y - viewport.y) / viewport.scale,
  };
}

export function clampZoom(nextScale: number): number {
  return Math.min(4, Math.max(0.2, nextScale));
}

export function panViewportFromPointerDelta(
  viewport: CanvasViewport,
  startPointer: { x: number; y: number },
  currentPointer: { x: number; y: number },
): CanvasViewport {
  return {
    ...viewport,
    x: viewport.x + currentPointer.x - startPointer.x,
    y: viewport.y + currentPointer.y - startPointer.y,
  };
}

export function isSecondaryButtonPan(button: number): boolean {
  return button === 2;
}

export function zoomViewportAtScreenPoint(
  viewport: CanvasViewport,
  screenPoint: { x: number; y: number },
  nextScale: number,
): CanvasViewport {
  const scale = clampZoom(nextScale);
  const canvasPoint = screenPointToCanvasPoint(screenPoint, viewport);

  return {
    scale,
    x: screenPoint.x - canvasPoint.x * scale,
    y: screenPoint.y - canvasPoint.y * scale,
  };
}

export function resizeFrameByAspect(
  frame: CanvasFrame,
  aspect: string,
): CanvasFrame {
  const [widthRatio, heightRatio] = aspect.split(":").map(Number);
  if (!widthRatio || !heightRatio) {
    return frame;
  }

  const width = frame.width;
  const height = width / (widthRatio / heightRatio);

  return {
    ...frame,
    height,
    aspect,
  };
}
