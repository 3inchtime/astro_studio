import type {
  CanvasDocumentContent,
  CanvasImageObject,
  CanvasStrokeObject,
} from "../../types";

const FALLBACK_DATA_URL =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4////fwAJ+wP9KobjigAAAABJRU5ErkJggg==";

export async function exportCanvasFrame(
  content: CanvasDocumentContent,
): Promise<string> {
  if (typeof document === "undefined") {
    return FALLBACK_DATA_URL;
  }

  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(content.frame.width));
  canvas.height = Math.max(1, Math.round(content.frame.height));
  const context = canvas.getContext("2d");
  if (!context) {
    return FALLBACK_DATA_URL;
  }

  context.fillStyle = "#f8f7f4";
  context.fillRect(0, 0, canvas.width, canvas.height);

  for (const layer of content.layers) {
    if (!layer.visible) {
      continue;
    }

    for (const object of layer.objects) {
      if (object.type === "stroke") {
        drawStroke(context, object, content);
      } else {
        await drawImageObject(context, object, content);
      }
    }
  }

  try {
    return canvas.toDataURL("image/png");
  } catch {
    return FALLBACK_DATA_URL;
  }
}

function drawStroke(
  context: CanvasRenderingContext2D,
  object: CanvasStrokeObject,
  content: CanvasDocumentContent,
) {
  if (object.points.length < 4) {
    return;
  }

  context.save();
  context.beginPath();
  context.lineCap = "round";
  context.lineJoin = "round";
  context.lineWidth = object.size;
  context.globalAlpha = object.opacity;
  context.strokeStyle = object.color;
  if (object.tool === "eraser") {
    context.globalCompositeOperation = "destination-out";
  }

  for (let index = 0; index < object.points.length; index += 2) {
    const x = object.points[index] - content.frame.x;
    const y = object.points[index + 1] - content.frame.y;
    if (index === 0) {
      context.moveTo(x, y);
    } else {
      context.lineTo(x, y);
    }
  }
  context.stroke();
  context.restore();
}

async function drawImageObject(
  context: CanvasRenderingContext2D,
  object: CanvasImageObject,
  content: CanvasDocumentContent,
) {
  const image = await loadImage(object.image_path);
  if (!image) {
    return;
  }

  context.save();
  context.globalAlpha = object.opacity;
  context.translate(
    object.x - content.frame.x + object.width / 2,
    object.y - content.frame.y + object.height / 2,
  );
  context.rotate((object.rotation * Math.PI) / 180);
  context.drawImage(
    image,
    -object.width / 2,
    -object.height / 2,
    object.width,
    object.height,
  );
  context.restore();
}

export async function loadImage(path: string): Promise<HTMLImageElement | null> {
  if (typeof window === "undefined") {
    return null;
  }

  return new Promise((resolve) => {
    const image = new window.Image();
    let settled = false;
    const finish = (value: HTMLImageElement | null) => {
      if (settled) {
        return;
      }
      settled = true;
      resolve(value);
    };

    image.onload = () => finish(image);
    image.onerror = () => finish(null);
    window.setTimeout(() => finish(null), 50);
    image.src = path;
  });
}

export async function readImageSize(path: string): Promise<{
  width: number;
  height: number;
}> {
  const image = await loadImage(path);
  if (!image) {
    return { width: 512, height: 512 };
  }

  return {
    width: image.naturalWidth || 512,
    height: image.naturalHeight || 512,
  };
}
