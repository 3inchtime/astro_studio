import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  layoutCanvasText,
  normalizeTextStyle,
  drawCanvasText,
  rotateCanvasText,
} from "./text";
import {
  createCanvasDocumentContent,
  sanitizeCanvasDocumentContent,
} from "./document";
import { updateCanvasObjectStyle, scaleCanvasObjects } from "./editor";
import type { CanvasTextObject } from "../../types";

const text: CanvasTextObject = {
  type: "text",
  id: "t",
  x: 10,
  y: 20,
  width: 40,
  height: 48,
  rotation: 0,
  text: "中文\n字",
  fontSize: 20,
  color: "#000000",
  opacity: 1,
};
const measure = (value: string) => [...value].length * 20;
beforeEach(() => {
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
});
afterEach(() => {
  vi.restoreAllMocks();
});

describe("canvas typography", () => {
  it("includes the last line's glyphs at tight line heights", () => {
    expect(layoutCanvasText({ ...text, lineHeight: 0.5 }, measure).height).toBe(
      30,
    );
  });
  it("bounds every glyph when negative tracking reverses the pen", () => {
    const layout = layoutCanvasText(
      { ...text, text: "ABC", letterSpacing: -20 },
      () => 12,
    );
    expect(layout.width).toBe(28);
    expect(layout.runs.map((run) => run.x)).toEqual([16, 8, 0]);
  });
  it("preserves scaled tracking beyond the inspector input range", () => {
    expect(normalizeTextStyle({ letterSpacing: 400 }).letterSpacing).toBe(400);
    expect(
      layoutCanvasText(
        { ...text, text: "AB", fontSize: 24, letterSpacing: 400 },
        () => 24,
      ).width,
    ).toBe(448);
  });
  it("preserves legacy defaults and normalizes invalid style values", () => {
    expect(normalizeTextStyle(text)).toEqual({
      fontFamily: "sans-serif",
      fontWeight: "normal",
      fontStyle: "normal",
      textAlign: "left",
      letterSpacing: 0,
      lineHeight: 1.2,
    });
    expect(
      normalizeTextStyle({
        lineHeight: NaN,
        letterSpacing: Infinity,
        fontFamily: "unknown",
      }),
    ).toMatchObject({
      lineHeight: 1.2,
      letterSpacing: 0,
      fontFamily: "sans-serif",
    });
  });
  it("lays out tracked CJK text with explicit multiline alignment and line height", () => {
    const layout = layoutCanvasText(
      {
        ...text,
        letterSpacing: 4,
        lineHeight: 1.8,
        textAlign: "center",
        width: 80,
      },
      measure,
    );
    expect(layout.width).toBe(44);
    expect(layout.height).toBe(72);
    expect(layout.runs).toEqual([
      { text: "中", x: 18, y: 0 },
      { text: "文", x: 42, y: 0 },
      { text: "字", x: 30, y: 36 },
    ]);
  });
  it("shares font and layout across scaled stage drawing and export", () => {
    const object = {
      ...text,
      fontFamily: "serif",
      fontWeight: "bold",
      fontStyle: "italic",
      textAlign: "right",
      lineHeight: 2,
    } as CanvasTextObject;
    const context = { setAttr: vi.fn(), fillText: vi.fn() };
    drawCanvasText(context, object, 2, measure);
    expect(context.setAttr).toHaveBeenCalledWith(
      "font",
      "italic bold 40px serif",
    );
    expect(context.fillText).toHaveBeenNthCalledWith(1, "中文", 0, 0);
    expect(context.fillText).toHaveBeenNthCalledWith(2, "字", 40, 80);
  });
  it("uses native tracking to preserve kerning and contextual shaping", () => {
    const measureText = vi.fn(() => ({ width: 69.186 }));
    vi.mocked(HTMLCanvasElement.prototype.getContext).mockReturnValue({
      font: "",
      letterSpacing: "0px",
      measureText,
    } as unknown as CanvasRenderingContext2D);
    const context = { setAttr: vi.fn(), fillText: vi.fn() };
    drawCanvasText(context, { ...text, text: "سلام", letterSpacing: 3 }, 2);
    expect(context.setAttr).toHaveBeenCalledWith("letterSpacing", "6px");
    expect(context.fillText).toHaveBeenCalledExactlyOnceWith("سلام", 0, 0);
    expect(measureText).toHaveBeenCalledWith("سلام");
  });
  it("contains native negative-tracking ink even when the advance is negative", () => {
    vi.mocked(HTMLCanvasElement.prototype.getContext).mockReturnValue({
      font: "",
      letterSpacing: "0px",
      measureText: () => ({
        width: -35,
        actualBoundingBoxLeft: 35,
        actualBoundingBoxRight: 8,
      }),
    } as unknown as CanvasRenderingContext2D);
    const layout = layoutCanvasText({
      ...text,
      text: "ABC",
      letterSpacing: -20,
    });
    expect(layout.width).toBe(43);
    expect(layout.runs[0].x).toBe(35);
  });
  it("aligns negative advances like the textarea, treating their inline width as zero", () => {
    vi.mocked(HTMLCanvasElement.prototype.getContext).mockReturnValue({
      font: "",
      letterSpacing: "0px",
      measureText: (value: string) =>
        value === "ABC"
          ? { width: -35, actualBoundingBoxLeft: 35, actualBoundingBoxRight: 8 }
          : { width: 20, actualBoundingBoxLeft: 0, actualBoundingBoxRight: 20 },
    } as unknown as CanvasRenderingContext2D);
    const layout = layoutCanvasText({
      ...text,
      text: "ABC\nD",
      textAlign: "center",
      letterSpacing: -20,
    });
    expect(layout.runs.map((run) => run.x)).toEqual([35, 25]);
    expect(layout.editor).toEqual({
      width: 20,
      paddingLeft: 25,
      paddingRight: 0,
    });
    expect(layout.width).toBe(45);
  });
  it("retains formatting through document normalization and applies new styles to old objects", () => {
    const content = createCanvasDocumentContent({
      layers: [
        { id: "l", name: "l", visible: true, locked: false, objects: [text] },
      ],
    });
    const styled = updateCanvasObjectStyle(content, ["t"], {
      fontFamily: "serif",
      fontWeight: "bold",
      fontStyle: "italic",
      textAlign: "right",
      letterSpacing: 3,
      lineHeight: 2,
    });
    expect(
      sanitizeCanvasDocumentContent(styled).layers[0].objects[0],
    ).toMatchObject({
      fontFamily: "serif",
      fontWeight: "bold",
      fontStyle: "italic",
      textAlign: "right",
      letterSpacing: 3,
      lineHeight: 2,
      height: 80,
    });
    const scaled = scaleCanvasObjects(styled, ["t"], {
      x: 0,
      y: 0,
      width: 1000,
      height: 160,
    });
    expect(scaled.layers[0].objects[0]).toMatchObject({
      fontSize: 40,
      letterSpacing: 6,
      lineHeight: 2,
    });
  });
  it("rotates around the existing visual center without losing typography", () => {
    const rotated = rotateCanvasText({ ...text, fontWeight: "bold" }, 90);
    expect(rotated).toMatchObject({
      x: 54,
      y: 24,
      rotation: 90,
      fontWeight: "bold",
    });
    const restored = rotateCanvasText(rotated, 360);
    expect(restored.x).toBeCloseTo(text.x);
    expect(restored.y).toBeCloseTo(text.y);
    expect(restored.rotation).toBe(0);
  });
});
