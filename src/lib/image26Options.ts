import { isGptImage26 } from "./modelCatalog";
import type { ImageAdvancedOptions } from "../types";
export interface Image26ValidationInput {
  model: string;
  format: string;
  size: string;
  count: number;
  compression?: number;
  background: string;
  editing: boolean;
  options: ImageAdvancedOptions;
}
export function image26ValidationError(
  input: Image26ValidationInput,
): string | null {
  if (!isGptImage26(input.model)) return null;
  const { format, size, count, background, editing, options } = input;
  if (
    ["jpeg", "webp"].includes(format) &&
    input.compression !== undefined &&
    (!Number.isInteger(input.compression) ||
      input.compression < 0 ||
      input.compression > 100)
  )
    return "compressionInvalid";
  if (format === "svg" && editing) return "svgGenerateOnly";
  if (format === "svg" && count !== 1) return "svgSingle";
  if (background === "transparent" && !["png", "webp", "svg"].includes(format))
    return "transparentFormat";
  if (editing && options.markupPath && options.maskPath)
    return "overlayConflict";
  if (format === "gif") {
    const frames = options.frameCount ?? 9;
    const duration = options.frameDurationMs ?? 200;
    if (!Number.isInteger(frames) || frames < 2 || frames > 16)
      return "gifFramesInvalid";
    if (!Number.isInteger(duration) || duration < 20 || duration > 4294967295)
      return "gifDurationInvalid";
    if (editing && options.maskPath) return "gifMaskUnsupported";
  }
  if (size !== "auto") {
    if (!/^\d+x\d+$/.test(size)) return "sizeInvalid";
    const [w, h] = size.split("x").map(Number);
    if (
      !Number.isSafeInteger(w) ||
      !Number.isSafeInteger(h) ||
      w <= 0 ||
      h <= 0 ||
      w % 16 ||
      h % 16
    )
      return "sizeInvalid";
    if (
      format !== "gif" &&
      (w > 3840 ||
        h > 3840 ||
        Math.max(w, h) > Math.min(w, h) * 3 ||
        w * h < 655360 ||
        w * h > 8294400)
    )
      return "sizeInvalid";
  }
  return null;
}
