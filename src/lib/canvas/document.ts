import type {
  CanvasDocumentContent,
  CanvasFrame,
  CanvasImageObject,
  CanvasLayer,
  CanvasObject,
  CanvasStrokeObject,
  CanvasTool,
  CanvasViewport,
} from "../../types";

export const DEFAULT_CANVAS_SIZE = 1024;
export const DEFAULT_BRUSH_COLOR = "#1f2937";
export const DEFAULT_BRUSH_SIZE = 6;
export const DEFAULT_ASPECT = "1:1";

export function createCanvasViewport(
  overrides: Partial<CanvasViewport> = {},
): CanvasViewport {
  return {
    x: 0,
    y: 0,
    scale: 1,
    ...overrides,
  };
}

export function createCanvasFrame(
  overrides: Partial<CanvasFrame> = {},
): CanvasFrame {
  return {
    x: 0,
    y: 0,
    width: DEFAULT_CANVAS_SIZE,
    height: DEFAULT_CANVAS_SIZE,
    aspect: DEFAULT_ASPECT,
    ...overrides,
  };
}

export function createCanvasLayer(
  overrides: Partial<CanvasLayer> = {},
): CanvasLayer {
  return {
    id: overrides.id ?? crypto.randomUUID(),
    name: overrides.name ?? "Sketch",
    visible: overrides.visible ?? true,
    locked: overrides.locked ?? false,
    objects: overrides.objects ?? [],
  };
}

export function createCanvasDocumentContent(
  overrides: Partial<CanvasDocumentContent> = {},
): CanvasDocumentContent {
  return {
    version: 1,
    viewport: createCanvasViewport(overrides.viewport),
    frame: createCanvasFrame(overrides.frame),
    layers: overrides.layers ?? [createCanvasLayer({ id: "layer-1" })],
  };
}

export function createStrokeObject(params: {
  color?: string;
  size?: number;
  opacity?: number;
  points?: number[];
  tool?: Extract<CanvasTool, "brush" | "eraser">;
  id?: string;
} = {}): CanvasStrokeObject {
  return {
    type: "stroke",
    id: params.id ?? crypto.randomUUID(),
    tool: params.tool ?? "brush",
    points: params.points ?? [],
    color: params.color ?? DEFAULT_BRUSH_COLOR,
    size: params.size ?? DEFAULT_BRUSH_SIZE,
    opacity: params.opacity ?? 1,
  };
}

export function createImageObject(params: {
  image_path: string;
  x?: number;
  y?: number;
  width: number;
  height: number;
  original_width?: number;
  original_height?: number;
  rotation?: number;
  opacity?: number;
  id?: string;
}): CanvasImageObject {
  return {
    type: "image",
    id: params.id ?? crypto.randomUUID(),
    image_path: params.image_path,
    x: params.x ?? 0,
    y: params.y ?? 0,
    width: params.width,
    height: params.height,
    original_width: params.original_width ?? params.width,
    original_height: params.original_height ?? params.height,
    rotation: params.rotation ?? 0,
    opacity: params.opacity ?? 1,
  };
}

export function cloneCanvasDocumentContent(
  content: CanvasDocumentContent,
): CanvasDocumentContent {
  return JSON.parse(JSON.stringify(content)) as CanvasDocumentContent;
}

export function sanitizeCanvasDocumentContent(
  content?: CanvasDocumentContent | null,
): CanvasDocumentContent {
  if (!content) {
    return createCanvasDocumentContent();
  }

  return {
    version: content.version ?? 1,
    viewport: createCanvasViewport(content.viewport),
    frame: createCanvasFrame(content.frame),
    layers:
      content.layers.length > 0
        ? content.layers.map((layer) => ({
            id: layer.id || crypto.randomUUID(),
            name: layer.name || "Layer",
            visible: layer.visible ?? true,
            locked: layer.locked ?? false,
            objects: layer.objects.map(sanitizeCanvasObject),
          }))
        : [createCanvasLayer({ id: "layer-1" })],
  };
}

function sanitizeCanvasObject(object: CanvasObject): CanvasObject {
  if (object.type === "stroke") {
    return createStrokeObject({
      id: object.id,
      tool: object.tool,
      points: object.points,
      color: object.color,
      size: object.size,
      opacity: object.opacity,
    });
  }

  return createImageObject({
    id: object.id,
    image_path: object.image_path,
    x: object.x,
    y: object.y,
    width: object.width,
    height: object.height,
    original_width: object.original_width ?? object.width,
    original_height: object.original_height ?? object.height,
    rotation: object.rotation,
    opacity: object.opacity,
  });
}

export function updateImageObject(
  content: CanvasDocumentContent,
  objectId: string,
  updates: Partial<Pick<CanvasImageObject, "x" | "y" | "width" | "height" | "rotation">>,
): CanvasDocumentContent {
  return {
    ...cloneCanvasDocumentContent(content),
    layers: content.layers.map((layer) => ({
      ...layer,
      objects: layer.objects.map((object) =>
        object.type === "image" && object.id === objectId
          ? {
              ...object,
              ...updates,
              width: Math.max(8, updates.width ?? object.width),
              height: Math.max(8, updates.height ?? object.height),
            }
          : object,
      ),
    })),
  };
}

export function resetImageObjectAspect(
  content: CanvasDocumentContent,
  objectId: string,
): CanvasDocumentContent {
  let target: CanvasImageObject | null = null;
  for (const layer of content.layers) {
    const object = layer.objects.find(
      (entry): entry is CanvasImageObject => entry.type === "image" && entry.id === objectId,
    );
    if (object) {
      target = object;
      break;
    }
  }

  if (!target) {
    return cloneCanvasDocumentContent(content);
  }

  const aspect = target.original_width / target.original_height;
  if (!Number.isFinite(aspect) || aspect <= 0) {
    return cloneCanvasDocumentContent(content);
  }

  return updateImageObject(content, objectId, {
    width: target.width,
    height: target.width / aspect,
  });
}

export function getActiveLayer(
  content: CanvasDocumentContent,
  layerId: string | null,
): CanvasLayer | null {
  if (!content.layers.length) {
    return null;
  }

  if (!layerId) {
    return content.layers[0];
  }

  return content.layers.find((layer) => layer.id === layerId) ?? content.layers[0];
}
