import { describe, expect, it } from "vitest";
import {
  clampZoom,
  frameToScreenRect,
  isSecondaryButtonPan,
  panViewportFromPointerDelta,
  resizeFrameByAspect,
  screenPointToCanvasPoint,
  zoomViewportAtScreenPoint,
} from "./frame";

describe("canvas frame helpers", () => {
  it("projects frame coordinates into screen space", () => {
    expect(
      frameToScreenRect(
        { x: 20, y: 30, width: 100, height: 80, aspect: "5:4" },
        { x: 10, y: 5, scale: 2 },
      ),
    ).toEqual({
      x: 50,
      y: 65,
      width: 200,
      height: 160,
    });
  });

  it("maps screen coordinates back into canvas space", () => {
    expect(screenPointToCanvasPoint({ x: 70, y: 85 }, { x: 10, y: 5, scale: 2 })).toEqual({
      x: 30,
      y: 40,
    });
  });

  it("clamps zoom to supported bounds", () => {
    expect(clampZoom(10)).toBe(4);
    expect(clampZoom(0.01)).toBe(0.2);
  });

  it("resizes frame height to the requested aspect ratio", () => {
    expect(
      resizeFrameByAspect(
        { x: 0, y: 0, width: 1200, height: 400, aspect: "3:1" },
        "16:9",
      ),
    ).toMatchObject({
      width: 1200,
      height: 675,
      aspect: "16:9",
    });
  });

  it("pans viewport from the original pointer anchor without compounding delta", () => {
    expect(
      panViewportFromPointerDelta(
        { x: 100, y: 80, scale: 1.5 },
        { x: 20, y: 30 },
        { x: 65, y: 10 },
      ),
    ).toEqual({
      x: 145,
      y: 60,
      scale: 1.5,
    });
  });

  it("treats only the secondary mouse button as temporary canvas pan", () => {
    expect(isSecondaryButtonPan(2)).toBe(true);
    expect(isSecondaryButtonPan(0)).toBe(false);
    expect(isSecondaryButtonPan(1)).toBe(false);
  });

  it("zooms viewport around the current pointer position", () => {
    expect(
      zoomViewportAtScreenPoint(
        { x: 100, y: 80, scale: 1 },
        { x: 300, y: 280 },
        2,
      ),
    ).toEqual({
      x: -100,
      y: -120,
      scale: 2,
    });
  });
});
