import type { CanvasTextObject, CanvasTextStyle } from "../../types";

// Local font stacks keep canvas documents usable offline on macOS and Windows.
export const CANVAS_FONTS = [
  { label: "fontSans", value: "sans-serif" },
  { label: "fontSerif", value: "serif" },
  { label: "fontMono", value: "monospace" },
  { label: "fontHei", value: '"PingFang SC", "Microsoft YaHei", sans-serif' },
  { label: "fontSong", value: '"Songti SC", SimSun, serif' },
  { label: "fontKai", value: '"Kaiti SC", KaiTi, serif' },
] as const;

export const DEFAULT_TEXT_STYLE: CanvasTextStyle = {
  fontFamily: "sans-serif",
  fontWeight: "normal",
  fontStyle: "normal",
  textAlign: "left",
  letterSpacing: 0,
  lineHeight: 1.2,
};

export function normalizeTextStyle(
  value: Partial<CanvasTextStyle>,
): CanvasTextStyle {
  return {
    fontFamily: CANVAS_FONTS.some((font) => font.value === value.fontFamily)
      ? value.fontFamily!
      : "sans-serif",
    fontWeight: value.fontWeight === "bold" ? "bold" : "normal",
    fontStyle: value.fontStyle === "italic" ? "italic" : "normal",
    textAlign:
      value.textAlign === "center" || value.textAlign === "right"
        ? value.textAlign
        : "left",
    letterSpacing: Number.isFinite(value.letterSpacing)
      ? value.letterSpacing!
      : 0,
    lineHeight: Number.isFinite(value.lineHeight)
      ? Math.max(0.5, Math.min(5, value.lineHeight!))
      : 1.2,
  };
}

export function canvasTextFont(object: CanvasTextObject, scale = 1) {
  const style = normalizeTextStyle(object);
  return `${style.fontStyle} ${style.fontWeight} ${object.fontSize * scale}px ${style.fontFamily}`;
}

type Measure = ((text: string) => number) & {
  nativeTracking?: boolean;
  inkBounds?: (text: string) => { left: number; right: number };
};
function textMeasure(object: CanvasTextObject): Measure {
  const ctx =
    typeof document === "undefined"
      ? null
      : document.createElement("canvas").getContext("2d");
  if (ctx?.measureText) {
    ctx.font = canvasTextFont(object);
    const nativeTracking = "letterSpacing" in ctx;
    if (nativeTracking)
      ctx.letterSpacing = `${normalizeTextStyle(object).letterSpacing}px`;
    return Object.assign((text: string) => ctx.measureText(text).width, {
      nativeTracking,
      inkBounds: (text: string) => {
        const metrics = ctx.measureText(text);
        return {
          left: Math.min(0, -(metrics.actualBoundingBoxLeft || 0)),
          right: Math.max(
            0,
            metrics.width,
            metrics.actualBoundingBoxRight || 0,
          ),
        };
      },
    });
  }
  return (text) => Array.from(text).length * object.fontSize;
}

// Segment grapheme clusters so tracking never splits combining marks or emoji.
const segmenter = new (
  Intl as typeof Intl & {
    Segmenter: new (
      locale?: string,
      options?: { granularity: string },
    ) => {
      segment: (text: string) => Iterable<{ segment: string }>;
    };
  }
).Segmenter(undefined, { granularity: "grapheme" });

export function layoutCanvasText(
  object: CanvasTextObject,
  measure = textMeasure(object),
) {
  const style = normalizeTextStyle(object);
  const wholeLine = measure.nativeTracking || style.letterSpacing === 0;
  const lines = object.text.split("\n").map((line) => {
    const glyphs = Array.from(segmenter.segment(line), (part) => part.segment);
    const widths = wholeLine ? [] : glyphs.map(measure);
    let pen = 0,
      left = 0,
      right = 0;
    const positions = widths.map((width) => {
      const x = pen;
      left = Math.min(left, x);
      right = Math.max(right, x + width);
      pen += width + style.letterSpacing;
      return x;
    });
    const ink =
      wholeLine && style.letterSpacing < 0
        ? measure.inkBounds?.(line)
        : undefined;
    return {
      line,
      glyphs,
      positions: positions.map((x) => x - left),
      origin: ink ? -ink.left : 0,
      width: ink
        ? ink.right - ink.left
        : wholeLine
          ? Math.max(0, measure(line))
          : right - left,
    };
  });
  const width = Math.max(
    1,
    Math.ceil(Math.max(...lines.map((line) => line.width))),
  );
  const height =
    (lines.length - 1) * object.fontSize * style.lineHeight +
    object.fontSize * Math.max(1, style.lineHeight);
  // Very tight tracking can place ink left of the text origin. Use one shared
  // inset for the whole block, also applied to the textarea, to keep editing
  // and drawing identical without clipping negative advances.
  if (measure.nativeTracking && style.letterSpacing < 0 && measure.inkBounds) {
    const advances = lines.map((line) => Math.max(0, measure(line.line)));
    const innerWidth = Math.max(1, ...advances);
    const aligned = lines.map((line, index) => {
      const x =
        style.textAlign === "right"
          ? innerWidth - advances[index]
          : style.textAlign === "center"
            ? (innerWidth - advances[index]) / 2
            : 0;
      return { line, x, ink: measure.inkBounds!(line.line) };
    });
    const paddingLeft = -Math.min(
      0,
      ...aligned.map(({ x, ink }) => x + ink.left),
    );
    const paddingRight = Math.max(
      0,
      ...aligned.map(({ x, ink }) => x + ink.right - innerWidth),
    );
    return {
      width: Math.ceil(innerWidth + paddingLeft + paddingRight),
      height,
      runs: aligned.map(({ line, x }, index) => ({
        text: line.line,
        x: x + paddingLeft,
        y: index * object.fontSize * style.lineHeight,
      })),
      nativeTracking: true,
      editor: { width: innerWidth, paddingLeft, paddingRight },
    };
  }
  const runs: { text: string; x: number; y: number }[] = [];
  lines.forEach((line, index) => {
    const x =
      style.textAlign === "right"
        ? object.width - line.width
        : style.textAlign === "center"
          ? (object.width - line.width) / 2
          : 0;
    const y = index * object.fontSize * style.lineHeight;
    if (wholeLine) runs.push({ text: line.line, x: x + line.origin, y });
    else
      line.glyphs.forEach((glyph, i) => {
        runs.push({ text: glyph, x: x + line.positions[i], y });
      });
  });
  return {
    width,
    height,
    runs,
    nativeTracking: measure.nativeTracking ?? false,
    editor: undefined,
  };
}

export function fitCanvasText(object: CanvasTextObject): CanvasTextObject {
  const next = { ...object, ...normalizeTextStyle(object) };
  const { width, height } = layoutCanvasText(next);
  return { ...next, width, height };
}

export function drawCanvasText(
  context: {
    setAttr: (key: string, value: string) => void;
    fillText: (text: string, x: number, y: number) => void;
  },
  object: CanvasTextObject,
  scale = 1,
  measure?: Measure,
) {
  context.setAttr("font", canvasTextFont(object, scale));
  context.setAttr("fillStyle", object.color);
  context.setAttr("textBaseline", "top");
  context.setAttr("textAlign", "left");
  const layout = layoutCanvasText(object, measure);
  context.setAttr(
    "letterSpacing",
    `${layout.nativeTracking ? normalizeTextStyle(object).letterSpacing * scale : 0}px`,
  );
  for (const run of layout.runs)
    context.fillText(run.text, run.x * scale, run.y * scale);
}

export function rotateCanvasText(
  object: CanvasTextObject,
  degrees: number,
): CanvasTextObject {
  if (!Number.isFinite(degrees)) return object;
  const rotation = ((degrees % 360) + 360) % 360;
  const a = (object.rotation * Math.PI) / 180,
    b = (rotation * Math.PI) / 180;
  const w = object.width / 2,
    h = object.height / 2;
  return {
    ...object,
    rotation,
    x:
      object.x +
      w * Math.cos(a) -
      h * Math.sin(a) -
      w * Math.cos(b) +
      h * Math.sin(b),
    y:
      object.y +
      w * Math.sin(a) +
      h * Math.cos(a) -
      w * Math.sin(b) -
      h * Math.cos(b),
  };
}
