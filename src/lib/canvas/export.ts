import { getCanvasArrowHeadPoints } from "./bounds";
import { isCanvasPathObject } from "./document";
import type { CanvasDocumentContent, CanvasObject } from "../../types";
import { getCanvasLayersBackToFront } from "./document";
import { toAssetUrl } from "../api";

export async function exportCanvasFrame(
  content: CanvasDocumentContent,
): Promise<string> {
  if (typeof document === "undefined")
    throw new Error("Canvas rendering is unavailable");
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(content.frame.width));
  canvas.height = Math.max(1, Math.round(content.frame.height));
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Canvas rendering context is unavailable");
  context.fillStyle = "#f8f7f4";
  context.fillRect(0, 0, canvas.width, canvas.height);
  for (const layer of getCanvasLayersBackToFront(content.layers)) {
    if (!layer.visible) continue;
    const surface = document.createElement("canvas");
    surface.width = canvas.width;
    surface.height = canvas.height;
    const ctx = surface.getContext("2d");
    if (!ctx) throw new Error("Canvas layer rendering is unavailable");
    for (const object of layer.objects) await drawObject(ctx, object, content);
    context.drawImage(surface, 0, 0);
  }
  return canvas.toDataURL("image/png");
}
async function drawObject(
  ctx: CanvasRenderingContext2D,
  o: CanvasObject,
  content: CanvasDocumentContent,
) {
  ctx.save();
  try {
    ctx.globalAlpha = o.opacity;
    if (isCanvasPathObject(o)) {
      if (o.points.length < 2) return;
      ctx.beginPath();
      ctx.lineCap = "round";
      ctx.lineJoin = "round";
      ctx.lineWidth = o.size;
      ctx.strokeStyle = o.color;
      if (o.type === "stroke" && o.tool === "eraser")
        ctx.globalCompositeOperation = "destination-out";
      for (let i = 0; i + 1 < o.points.length; i += 2) {
        const x = o.points[i] - content.frame.x,
          y = o.points[i + 1] - content.frame.y;
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      if (o.points.length === 2)
        ctx.lineTo(
          o.points[0] - content.frame.x + 0.01,
          o.points[1] - content.frame.y,
        );
      ctx.stroke();
      if (o.type === "arrow" && o.points.length >= 4) {
        const head = getCanvasArrowHeadPoints(o);
        ctx.beginPath();
        ctx.moveTo(head[0] - content.frame.x, head[1] - content.frame.y);
        ctx.lineTo(head[2] - content.frame.x, head[3] - content.frame.y);
        ctx.lineTo(head[4] - content.frame.x, head[5] - content.frame.y);
        ctx.stroke();
      }
    } else {
      ctx.translate(o.x - content.frame.x, o.y - content.frame.y);
      ctx.rotate((o.rotation * Math.PI) / 180);
      if (o.type === "image") {
        ctx.drawImage(await loadImage(o.image_path), 0, 0, o.width, o.height);
        return;
      }
      if (o.type === "text") {
        ctx.fillStyle = o.color;
        ctx.font = `${o.fontSize}px sans-serif`;
        ctx.textBaseline = "top";
        o.text
          .split("\n")
          .forEach((line, i) =>
            ctx.fillText(line, 0, i * o.fontSize * 1.2),
          );
        return;
      }
      ctx.beginPath();
      ctx.lineWidth = o.size;
      ctx.strokeStyle = o.color;
      ctx.fillStyle = o.fill;
      if (o.type === "ellipse")
        ctx.ellipse(
          o.width / 2,
          o.height / 2,
          o.width / 2,
          o.height / 2,
          0,
          0,
          Math.PI * 2,
        );
      else ctx.rect(0, 0, o.width, o.height);
      if (o.fill && o.fill !== "transparent") ctx.fill();
      ctx.stroke();
    }
  } finally {
    ctx.restore();
  }
}
export async function loadImage(path: string): Promise<HTMLImageElement> {
  if (typeof window === "undefined")
    throw new Error("Image loading is unavailable");
  return new Promise((resolve, reject) => {
    const image = new window.Image();
    // Tauri asset URLs have a different origin from the webview. Request CORS
    // before setting src so drawing a local image does not taint PNG exports.
    image.crossOrigin = "anonymous";
    const timer = window.setTimeout(
      () => finish(new Error(`Image load timed out: ${path}`)),
      10000,
    );
    let settled = false;
    function finish(error?: Error) {
      if (settled) return;
      settled = true;
      window.clearTimeout(timer);
      image.onload = null;
      image.onerror = null;
      if (error) reject(error);
      else resolve(image);
    }
    image.onload = () => finish();
    image.onerror = () => finish(new Error(`Unable to load image: ${path}`));
    image.src = toAssetUrl(path);
  });
}
export async function readImageSize(
  path: string,
): Promise<{ width: number; height: number }> {
  const image = await loadImage(path);
  return {
    width: image.naturalWidth || image.width,
    height: image.naturalHeight || image.height,
  };
}
