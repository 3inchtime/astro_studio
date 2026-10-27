import { describe, it, expect } from "vitest";
import {
  createCanvasDocumentContent,
  sanitizeCanvasDocumentContent,
} from "./document";
import {
  alignCanvasObjects,
  distributeCanvasObjects,
  scaleCanvasObjects,
  snapCanvasSelection,
  updateCanvasObjectStyle,
} from "./editor";
import { getCanvasObjectBounds } from "./bounds";
import { hitTestCanvasObjectId } from "./selection";
import { viewportFromWheel, isSecondaryButtonPan } from "./frame";
import {
  createHistory,
  pushHistory,
  replaceHistory,
  undoHistory,
  redoHistory,
} from "./history";
import type { CanvasObject } from "../../types";
const box = (id: string, x: number) =>
  ({
    type: "rectangle",
    id,
    x,
    y: 10,
    width: 20,
    height: 30,
    rotation: 0,
    color: "#123",
    fill: "transparent",
    size: 2,
    opacity: 1,
  }) as CanvasObject;
function content() {
  const c = createCanvasDocumentContent();
  c.layers[0].objects = [box("a", 10), box("b", 60), box("c", 110)];
  return c;
}
describe("canvas v2 editing", () => {
  it("upgrades documents and preserves shapes and requests", () => {
    const c = content();
    c.version = 1;
    expect(sanitizeCanvasDocumentContent(c)).toMatchObject({
      version: 2,
      generation_requests: [],
      layers: [{ objects: [box("a", 10), box("b", 60), box("c", 110)] }],
    });
  });
  it("bounds boxes and line hits use distance at screen scale", () => {
    expect(getCanvasObjectBounds(box("a", 10))).toEqual({
      x: 10,
      y: 10,
      width: 20,
      height: 30,
    });
    const c = content();
    c.layers[0].objects = [
      {
        type: "line",
        id: "l",
        points: [0, 0, 100, 100],
        size: 2,
        color: "#000",
        opacity: 1,
      } as CanvasObject,
    ];
    expect(hitTestCanvasObjectId(c, { x: 50, y: 10 })).toBeNull();
    expect(hitTestCanvasObjectId(c, { x: 50, y: 53 })).toBe("l");
  });
  it("scales all selected objects to bounds", () => {
    const c = scaleCanvasObjects(content(), ["a", "b"], {
      x: 0,
      y: 0,
      width: 140,
      height: 60,
    });
    expect(getCanvasObjectBounds(c.layers[0].objects[0])).toEqual({
      x: 0,
      y: 0,
      width: 40,
      height: 60,
    });
    expect(getCanvasObjectBounds(c.layers[0].objects[1])?.x).toBe(100);
  });
  it("aligns and distributes while preserving locked objects", () => {
    expect(
      alignCanvasObjects(content(), ["a", "b"], "right").layers[0].objects[0],
    ).toMatchObject({ x: 60 });
    const c = content();
    (c.layers[0].objects[1] as any).x = 40;
    expect(
      distributeCanvasObjects(c, ["a", "b", "c"], "horizontal").layers[0]
        .objects[1],
    ).toMatchObject({ x: 60 });
  });
  it("styles selected objects and snaps at zoom adjusted threshold", () => {
    expect(
      updateCanvasObjectStyle(content(), ["a"], { fill: "#f00" }).layers[0]
        .objects[0],
    ).toMatchObject({ fill: "#f00" });
    expect(
      snapCanvasSelection(content(), ["a"], { dx: 27, dy: 0 }).delta.dx,
    ).toBe(30);
  });
  it("pans wheels and anchors modifier zoom", () => {
    expect(
      viewportFromWheel(
        { x: 10, y: 20, scale: 1 },
        { x: 50, y: 50 },
        { deltaX: 4, deltaY: 8 },
      ),
    ).toEqual({ x: 6, y: 12, scale: 1 });
    expect(
      viewportFromWheel(
        { x: 0, y: 0, scale: 1 },
        { x: 50, y: 50 },
        { deltaX: 0, deltaY: -100, ctrlKey: true },
      ).scale,
    ).toBeGreaterThan(1);
    expect(isSecondaryButtonPan(1)).toBe(true);
  });
  it("undo keeps the latest viewport and generation metadata", () => {
    const first = content();
    const next = alignCanvasObjects(first, ["a", "b"], "left");
    let h = pushHistory(createHistory(first), next);
    h = replaceHistory(h, {
      ...next,
      viewport: { x: 123, y: 0, scale: 2 },
      generation_requests: [],
    });
    expect(undoHistory(h).present.viewport.x).toBe(123);
    expect(redoHistory(undoHistory(h)).present.viewport.x).toBe(123);
  });
});

it("sanitizes new objects with defaults instead of dropping shape/text data", () => {
  const c = content();
  c.layers[0].objects = [
    { type: "text", id: "t", x: 1, y: 2, text: "Hello" } as CanvasObject,
    {
      type: "ellipse",
      id: "e",
      x: 3,
      y: 4,
      width: 50,
      height: 40,
    } as CanvasObject,
  ];
  expect(sanitizeCanvasDocumentContent(c).layers[0].objects).toMatchObject([
    { type: "text", text: "Hello", fontSize: 24, opacity: 1, rotation: 0 },
    {
      type: "ellipse",
      fill: "transparent",
      size: 6,
      color: "#1f2937",
      rotation: 0,
    },
  ]);
});
it("ignores malformed non-finite geometry in bounds and transformations", () => {
  const c = content();
  (c.layers[0].objects[0] as any).width = NaN;
  expect(getCanvasObjectBounds(c.layers[0].objects[0])).toBeNull();
  expect(
    scaleCanvasObjects(content(), ["a"], { x: 0, y: 0, width: NaN, height: 1 }),
  ).toEqual(content());
});
it("undo preserves viewport and request state after a real edit", () => {
  const first = content(),
    next = content();
  next.frame.x = 100;
  let h = pushHistory(createHistory(first), next);
  h = replaceHistory(h, {
    ...next,
    viewport: { x: 91, y: 72, scale: 2 },
    generation_requests: [
      {
        request_id: "r",
        project_id: null,
        frame: first.frame,
        prompt: "p",
        status: "completed",
        image_ids: ["i"],
        created_at: "now",
      },
    ],
  });
  const undone = undoHistory(h);
  expect(undone.present.frame.x).toBe(0);
  expect(undone.present.viewport.x).toBe(91);
  expect(undone.present.generation_requests?.[0].status).toBe("completed");
});

it("roundtrips Rust-shaped boxes and text without treating default points as paths", async () => {
  const { translateCanvasObjects } = await import("./transforms");
  const { copyCanvasObjects, pasteCanvasObjects } = await import("./clipboard");
  const raw = content();
  raw.layers[0].objects = [
    {
      ...box("r", 10),
      points: [],
      tool: null,
      image_path: null,
      text: null,
      fontSize: null,
    },
    {
      type: "text",
      id: "t",
      x: 60,
      y: 10,
      width: 40,
      height: 30,
      rotation: 0,
      text: "Saved",
      fontSize: 24,
      color: "#000",
      opacity: 1,
      points: [],
      tool: null,
      image_path: null,
      fill: null,
      size: null,
    },
  ] as unknown as CanvasObject[];
  const clean = sanitizeCanvasDocumentContent(raw);
  for (const c of [raw, clean]) {
    expect(getCanvasObjectBounds(c.layers[0].objects[0])).toEqual({
      x: 10,
      y: 10,
      width: 20,
      height: 30,
    });
    expect(hitTestCanvasObjectId(c, { x: 65, y: 15 })).toBe("t");
    expect(
      translateCanvasObjects(c, ["r", "t"], { dx: 5, dy: 7 }).layers[0].objects,
    ).toMatchObject([
      { x: 15, y: 17 },
      { x: 65, y: 17 },
    ]);
    const pasted = pasteCanvasObjects(
      c,
      copyCanvasObjects(c, ["r", "t"]),
      "layer-1",
      24,
    );
    expect(pasted.content.layers[0].objects.slice(-2)).toMatchObject([
      { x: 34, y: 34 },
      { x: 84, y: 34 },
    ]);
    expect(
      scaleCanvasObjects(c, ["r"], { x: 0, y: 0, width: 40, height: 60 })
        .layers[0].objects[0],
    ).toMatchObject({ x: 0, y: 0, width: 40, height: 60 });
  }
  expect(clean.layers[0].objects[0]).not.toHaveProperty("points");
  expect(clean.layers[0].objects[1]).not.toHaveProperty("points");
  expect(clean.layers[0].objects[0]).not.toHaveProperty("text");
});

it("includes stroked arrow heads in bounds and hit tests", () => {
  const c = content();
  c.viewport.scale = 4;
  c.layers[0].objects = [
    {
      type: "arrow",
      id: "a",
      points: [0, 0, 100, 0],
      size: 20,
      color: "#000",
      opacity: 1,
    },
  ];
  const bounds = getCanvasObjectBounds(c.layers[0].objects[0])!;
  expect(bounds.x).toBe(-10);
  expect(bounds.width).toBe(120);
  expect(bounds.y).toBeCloseTo(-40);
  expect(bounds.height).toBeCloseTo(80);
  expect(hitTestCanvasObjectId(c, { x: 50, y: 29 })).toBe("a");
});
it("uniformly scales pre-existing rotated images into a target box", async () => {
  const { createImageObject } = await import("./document");
  const c = content();
  c.layers[0].objects = [
    createImageObject({
      id: "r",
      image_path: "/p",
      x: 100,
      y: 20,
      width: 80,
      height: 40,
      rotation: 90,
    }),
  ];
  const next = scaleCanvasObjects(c, ["r"], {
    x: 0,
    y: 0,
    width: 80,
    height: 80,
  });
  expect(getCanvasObjectBounds(next.layers[0].objects[0])).toEqual({
    x: 0,
    y: 0,
    width: 40,
    height: 80,
  });
});
it("skips locked selected layers during alignment and distribution", () => {
  const c = content();
  c.layers.push({
    ...c.layers[0],
    id: "locked",
    locked: true,
    objects: [box("locked", 200)],
  });
  expect(
    alignCanvasObjects(c, ["a", "b", "locked"], "right").layers[1].objects[0],
  ).toMatchObject({ x: 200 });
  expect(
    distributeCanvasObjects(c, ["a", "b", "c", "locked"], "horizontal")
      .layers[1].objects[0],
  ).toMatchObject({ x: 200 });
});
it("uses a fixed screen snap tolerance across zoom levels", () => {
  const c = content();
  c.viewport.scale = 4;
  expect(snapCanvasSelection(c, ["a"], { dx: 27, dy: 0 }).delta.dx).toBe(27);
  c.viewport.scale = 1;
  expect(snapCanvasSelection(c, ["a"], { dx: 27, dy: 0 }).delta.dx).toBe(30);
});

it.each(["stroke", "line", "arrow"] as const)(
  "resizes %s bounds exactly with proportional stroke and arrowhead scaling",
  (type) => {
    const c = content();
    c.layers[0].objects = [
      {
        type,
        id: "p",
        points: [0, 0, 100, 100],
        size: 2,
        color: "#000",
        opacity: 1,
        ...(type === "stroke" ? { tool: "brush" as const } : {}),
      },
    ];
    const source = getCanvasObjectBounds(c.layers[0].objects[0])!;
    const target = {
      x: 20,
      y: 30,
      width: source.width * 2,
      height: source.height * 2,
    };
    const scaled = scaleCanvasObjects(c, ["p"], target);
    const bounds = getCanvasObjectBounds(scaled.layers[0].objects[0])!;
    expect(bounds.x).toBeCloseTo(target.x);
    expect(bounds.y).toBeCloseTo(target.y);
    expect(bounds.width).toBeCloseTo(target.width);
    expect(bounds.height).toBeCloseTo(target.height);
    expect(scaled.layers[0].objects[0]).toMatchObject({ size: 4 });
  },
);
it("preserves aspect for paths in a nonuniform target and scales arrowheads below the default minimum", () => {
  const c = content();
  c.layers[0].objects = [
    {
      type: "arrow",
      id: "p",
      points: [0, 0, 100, 0],
      size: 2,
      color: "#000",
      opacity: 1,
    },
  ];
  const source = getCanvasObjectBounds(c.layers[0].objects[0])!;
  const scaled = scaleCanvasObjects(c, ["p"], {
    x: 0,
    y: 0,
    width: source.width * 0.5,
    height: source.height * 2,
  });
  expect(scaled.layers[0].objects[0]).toMatchObject({ size: 1, headSize: 6 });
  const b = getCanvasObjectBounds(scaled.layers[0].objects[0])!;
  expect(b.width).toBeCloseTo(source.width * 0.5);
  expect(b.height).toBeCloseTo(source.height * 0.5);
  const saved = sanitizeCanvasDocumentContent(scaled);
  expect(saved.layers[0].objects[0]).toMatchObject({ headSize: 6 });
});

it("resizes text proportionally without compressing natural glyph metrics", () => {
  const c = content();
  c.layers[0].objects = [
    {
      type: "text",
      id: "text",
      x: 0,
      y: 0,
      width: 100,
      height: 30,
      rotation: 0,
      text: "中文内容",
      fontSize: 25,
      color: "#000",
      opacity: 1,
    },
  ];
  const scaled = scaleCanvasObjects(c, ["text"], {
    x: 0,
    y: 0,
    width: 200,
    height: 90,
  });
  expect(scaled.layers[0].objects[0]).toMatchObject({
    width: 200,
    height: 60,
    fontSize: 50,
  });
});

it("updates text bounds along with font size so selection and framing contain glyphs", () => {
  const c = content();
  c.layers[0].objects = [
    {
      type: "text",
      id: "text",
      x: 10,
      y: 20,
      width: 100,
      height: 30,
      rotation: 0,
      text: "中文内容",
      fontSize: 25,
      color: "#000",
      opacity: 1,
    },
  ];
  const styled = updateCanvasObjectStyle(c, ["text"], { fontSize: 50 });
  expect(styled.layers[0].objects[0]).toMatchObject({
    x: 10,
    y: 20,
    width: 200,
    height: 60,
    fontSize: 50,
  });
});
