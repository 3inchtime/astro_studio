import { isCanvasPathObject } from "./document";
import type { CanvasDocumentContent, CanvasObject } from "../../types";
import {
  getCanvasObjectBounds,
  getCombinedCanvasBounds,
  type CanvasRect,
} from "./bounds";
import {
  cloneCanvasDocumentContent,
  isCanvasLayerSelectable,
} from "./document";
import { translateCanvasObjects } from "./transforms";
import { getSelectableCanvasObjects } from "./selection";
export type CanvasObjectStyle = {
  color?: string;
  size?: number;
  opacity?: number;
  fill?: string;
  fontSize?: number;
  text?: string;
};
export type CanvasAlignment =
  "left" | "center" | "right" | "top" | "middle" | "bottom";
function mapSelected(
  content: CanvasDocumentContent,
  ids: string[],
  transform: (object: CanvasObject) => CanvasObject,
): CanvasDocumentContent {
  const next = cloneCanvasDocumentContent(content);
  const selected = new Set(ids);
  next.layers.forEach((layer) => {
    if (isCanvasLayerSelectable(layer))
      layer.objects = layer.objects.map((o) =>
        selected.has(o.id) ? transform(o) : o,
      );
  });
  return next;
}
function selectedBounds(content: CanvasDocumentContent, ids: string[]) {
  return getCombinedCanvasBounds(
    content,
    getSelectableCanvasObjects(content)
      .filter((e) => ids.includes(e.object.id))
      .map((e) => e.object.id),
  );
}
export function scaleCanvasObjects(
  content: CanvasDocumentContent,
  ids: string[],
  target: CanvasRect,
): CanvasDocumentContent {
  if (
    ![target.x, target.y, target.width, target.height].every(Number.isFinite) ||
    target.width < 0 ||
    target.height < 0
  )
    return cloneCanvasDocumentContent(content);
  const source = selectedBounds(content, ids);
  if (!source) return cloneCanvasDocumentContent(content);
  let sx = target.width / (source.width || 1),
    sy = target.height / (source.height || 1);
  // Preserve stroke and glyph proportions; rotated boxes cannot represent skew.
  const requiresUniformScale = getSelectableCanvasObjects(content).some(
    ({ object }) =>
      ids.includes(object.id) &&
      (isCanvasPathObject(object) ||
        object.type === "text" ||
        object.rotation % 360 !== 0),
  );
  if (requiresUniformScale) sx = sy = Math.min(sx, sy);
  return mapSelected(content, ids, (o) => {
    if (isCanvasPathObject(o))
      return {
        ...o,
        size: o.size * sx,
        ...(o.type === "arrow"
          ? { headSize: (o.headSize ?? Math.max(12, o.size * 3)) * sx }
          : {}),
        points: o.points.map((v, i) =>
          i % 2 === 0
            ? target.x + (v - source.x) * sx
            : target.y + (v - source.y) * sy,
        ),
      };
    return {
      ...o,
      x: target.x + (o.x - source.x) * sx,
      y: target.y + (o.y - source.y) * sy,
      width: Math.max(1, o.width * sx),
      height: Math.max(1, o.height * sy),
      ...(o.type === "text"
        ? { fontSize: Math.max(1, o.fontSize * Math.min(sx, sy)) }
        : {}),
    };
  });
}
export function updateCanvasObjectStyle(
  content: CanvasDocumentContent,
  ids: string[],
  style: CanvasObjectStyle,
): CanvasDocumentContent {
  return mapSelected(content, ids, (o) => {
    const next = { ...o };
    for (const [key, value] of Object.entries(style)) {
      if (key in next && value !== undefined)
        Object.assign(next, {
          [key]:
            key === "opacity"
              ? Math.max(0, Math.min(1, Number(value)))
              : key === "size" || key === "fontSize"
                ? Math.max(1, Number(value))
                : value,
        });
    }
    if (
      o.type === "text" &&
      next.type === "text" &&
      next.fontSize !== o.fontSize
    ) {
      const factor = next.fontSize / Math.max(1, o.fontSize);
      next.width = o.width * factor;
      next.height = o.height * factor;
    }
    return next;
  });
}
export function alignCanvasObjects(
  content: CanvasDocumentContent,
  ids: string[],
  alignment: CanvasAlignment,
): CanvasDocumentContent {
  const all = selectedBounds(content, ids);
  if (!all) return cloneCanvasDocumentContent(content);
  let next = content;
  for (const { object } of getSelectableCanvasObjects(content)) {
    if (!ids.includes(object.id)) continue;
    const b = getCanvasObjectBounds(object);
    if (!b) continue;
    const dx =
      alignment === "left"
        ? all.x - b.x
        : alignment === "center"
          ? all.x + all.width / 2 - b.x - b.width / 2
          : alignment === "right"
            ? all.x + all.width - b.x - b.width
            : 0;
    const dy =
      alignment === "top"
        ? all.y - b.y
        : alignment === "middle"
          ? all.y + all.height / 2 - b.y - b.height / 2
          : alignment === "bottom"
            ? all.y + all.height - b.y - b.height
            : 0;
    next = translateCanvasObjects(next, [object.id], { dx, dy });
  }
  return next;
}
export function distributeCanvasObjects(
  content: CanvasDocumentContent,
  ids: string[],
  axis: "horizontal" | "vertical",
): CanvasDocumentContent {
  const entries = getSelectableCanvasObjects(content)
    .filter((e) => ids.includes(e.object.id))
    .map((e) => ({ object: e.object, b: getCanvasObjectBounds(e.object) }))
    .filter((e): e is { object: CanvasObject; b: CanvasRect } => !!e.b);
  const pos = axis === "horizontal" ? "x" : "y",
    size = axis === "horizontal" ? "width" : "height";
  entries.sort((a, b) => a.b[pos] - b.b[pos]);
  if (entries.length < 3) return cloneCanvasDocumentContent(content);
  const first = entries[0].b,
    last = entries[entries.length - 1].b;
  const gap =
    (last[pos] +
      last[size] -
      first[pos] -
      entries.reduce((sum, e) => sum + e.b[size], 0)) /
    (entries.length - 1);
  let cursor = first[pos],
    next = content;
  for (const e of entries) {
    next = translateCanvasObjects(next, [e.object.id], {
      dx: pos === "x" ? cursor - e.b.x : 0,
      dy: pos === "y" ? cursor - e.b.y : 0,
    });
    cursor += e.b[size] + gap;
  }
  return next;
}
export interface CanvasSnapGuide {
  axis: "x" | "y";
  position: number;
}
export function snapCanvasSelection(
  content: CanvasDocumentContent,
  ids: string[],
  delta: { dx: number; dy: number },
  thresholdScreen = 6,
): { delta: { dx: number; dy: number }; guides: CanvasSnapGuide[] } {
  const b = selectedBounds(content, ids);
  if (!b) return { delta, guides: [] };
  const threshold = thresholdScreen / content.viewport.scale;
  const targets = [
    content.frame,
    ...content.layers
      .filter((l) => l.visible)
      .flatMap((l) =>
        l.objects
          .filter((o) => !ids.includes(o.id))
          .map(getCanvasObjectBounds)
          .filter((v): v is CanvasRect => !!v),
      ),
  ];
  const next = { ...delta },
    guides: CanvasSnapGuide[] = [];
  for (const axis of ["x", "y"] as const) {
    const size = axis === "x" ? "width" : "height",
      key = axis === "x" ? "dx" : "dy";
    let best = threshold + Number.EPSILON,
      adjustment = 0,
      position: number | undefined;
    for (const target of targets)
      for (const a of [0, 0.5, 1])
        for (const z of [0, 0.5, 1]) {
          const edge = target[axis] + target[size] * z;
          const d = edge - (b[axis] + b[size] * a + delta[key]);
          if (Math.abs(d) < best) {
            best = Math.abs(d);
            adjustment = d;
            position = edge;
          }
        }
    if (position !== undefined) {
      next[key] += adjustment;
      guides.push({ axis, position });
    }
  }
  return { delta: next, guides };
}
