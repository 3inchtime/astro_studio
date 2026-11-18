import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "../../lib/desktop";
import type { EditSourceImage } from "../../types";

export async function pickSourceImages(): Promise<string[]> {
  if (!hasTauriRuntime()) {
    return pickBrowserSourceImages();
  }

  return invoke("pick_source_images");
}

async function pickBrowserSourceImages(): Promise<string[]> {
  if (typeof document === "undefined") {
    return [];
  }

  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/*";
    input.multiple = true;
    input.style.position = "fixed";
    input.style.left = "-9999px";
    document.body.appendChild(input);

    input.addEventListener(
      "change",
      async () => {
        const files = Array.from(input.files ?? []);
        const results = await Promise.all(files.map(fileToDataUrl));
        input.remove();
        resolve(results.filter((value): value is string => Boolean(value)));
      },
      { once: true },
    );

    input.addEventListener(
      "cancel",
      () => {
        input.remove();
        resolve([]);
      },
      { once: true },
    );

    input.click();
  });
}

async function fileToDataUrl(file: File): Promise<string | null> {
  return new Promise((resolve) => {
    const reader = new FileReader();
    reader.onload = () =>
      resolve(typeof reader.result === "string" ? reader.result : null);
    reader.onerror = () => resolve(null);
    reader.readAsDataURL(file);
  });
}

export async function copyImageToClipboard(imagePath: string): Promise<void> {
  await invoke("copy_image_to_clipboard", { imagePath });
}

export async function saveImageToFile(imagePath: string): Promise<void> {
  await invoke("save_image_to_file", { imagePath });
}

export function messageImageToEditSource(image: {
  path: string;
  imageId?: string;
  generationId?: string;
}): EditSourceImage {
  const normalizedPath = image.path.replace(/\\/g, "/");
  const fileName = normalizedPath.split("/").pop() || "source-image";

  return {
    id: `${image.imageId ?? image.generationId ?? "source"}:${normalizedPath}`,
    path: image.path,
    label: fileName,
    imageId: image.imageId,
    generationId: image.generationId,
  };
}
