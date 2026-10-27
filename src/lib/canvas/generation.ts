import type { CanvasDocumentContent, GeneratedImage } from "../../types";
import { getCanvasObjectBounds } from "./bounds";
import { createCanvasLayer, createImageObject } from "./document";

export function failCanvasGeneration(
  content: CanvasDocumentContent,
  requestId: string,
  error: string,
): CanvasDocumentContent {
  if (
    !content.generation_requests?.some(
      (request) =>
        request.request_id === requestId && request.status === "pending",
    )
  )
    return content;
  return {
    ...content,
    generation_requests: content.generation_requests?.map((r) =>
      r.request_id === requestId && r.status === "pending"
        ? { ...r, status: "failed", error }
        : r,
    ),
  };
}
export function completeCanvasGeneration(
  content: CanvasDocumentContent,
  requestId: string,
  generationId: string,
  images: GeneratedImage[],
): CanvasDocumentContent {
  const request = content.generation_requests?.find(
    (r) => r.request_id === requestId,
  );
  if (!request || request.status !== "pending") return content;
  const unique = [
    ...new Map(images.map((image) => [image.id, image])).values(),
  ].filter((image) => image.width > 0 && image.height > 0);
  if (!unique.length)
    return failCanvasGeneration(content, requestId, "No images returned");
  const occupied = content.layers
    .filter((l) => l.visible)
    .flatMap((l) => l.objects.map(getCanvasObjectBounds))
    .filter((b) => b !== null);
  let x = request.frame.x + request.frame.width + 32;
  const objects = unique.map((image) => {
    const width = (request.frame.height * image.width) / image.height,
      height = request.frame.height,
      y = request.frame.y;
    let collision;
    while (
      (collision = occupied.find(
        (bounds) =>
          x < bounds.x + bounds.width + 32 &&
          x + width + 32 > bounds.x &&
          y < bounds.y + bounds.height + 32 &&
          y + height + 32 > bounds.y,
      ))
    )
      x = collision.x + collision.width + 32;
    const object = createImageObject({
      id: `canvas-result:${requestId}:${image.id}`,
      image_path: image.file_path,
      x,
      y,
      width,
      height,
      original_width: image.width,
      original_height: image.height,
    });
    occupied.push({ x, y, width, height });
    x += width + 32;
    return object;
  });
  const layer = createCanvasLayer({ name: request.prompt.slice(0, 48) });
  layer.objects = objects;
  return {
    ...content,
    layers: [layer, ...content.layers],
    generation_requests: content.generation_requests?.map((r) =>
      r.request_id === requestId
        ? {
            ...r,
            generation_id: generationId,
            status: "completed",
            error: undefined,
            image_ids: unique.map((i) => i.id),
          }
        : r,
    ),
  };
}
