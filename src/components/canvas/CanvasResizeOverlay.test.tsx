import { fireEvent, render, screen, cleanup } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import CanvasResizeOverlay from "./CanvasResizeOverlay";
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (k: string) => k }),
}));
afterEach(cleanup);
describe("transactional resizing", () => {
  it("preserves ratios for selections requiring uniform scaling", () => {
    const onCommit = vi.fn();
    render(
      <CanvasResizeOverlay
        kind="selection"
        bounds={{ x: 10, y: 20, width: 80, height: 40 }}
        viewport={{ x: 0, y: 0, scale: 2 }}
        keepRatio
        onPreview={vi.fn()}
        onCommit={onCommit}
        identity="doc"
      />,
    );
    fireEvent.mouseDown(
      screen.getByRole("button", {
        name: "canvas.resizeSelection bottom-right",
      }),
      { clientX: 180, clientY: 120 },
    );
    fireEvent.mouseMove(window, { clientX: 260, clientY: 130 });
    fireEvent.mouseUp(window);
    expect(onCommit).toHaveBeenCalledWith({
      x: 10,
      y: 20,
      width: 120,
      height: 60,
    });
  });
  it("moves frame in canvas units and commits once", () => {
    const onCommit = vi.fn();
    render(
      <CanvasResizeOverlay
        kind="frame"
        bounds={{ x: 10, y: 20, width: 80, height: 40 }}
        viewport={{ x: 0, y: 0, scale: 2 }}
        onPreview={vi.fn()}
        onCommit={onCommit}
        identity="doc"
      />,
    );
    fireEvent.mouseDown(
      screen.getByRole("button", { name: "canvas.moveFrame" }),
      { clientX: 100, clientY: 100 },
    );
    fireEvent.mouseMove(window, { clientX: 140, clientY: 160 });
    expect(onCommit).not.toHaveBeenCalled();
    fireEvent.mouseUp(window);
    fireEvent.mouseUp(window);
    expect(onCommit).toHaveBeenCalledExactlyOnceWith({
      x: 30,
      y: 50,
      width: 80,
      height: 40,
    });
  });
  it.each([1, 2])(
    "ignores non-primary mouse button %s for resize and frame movement",
    (button) => {
      const onPreview = vi.fn(),
        onCommit = vi.fn();
      render(
        <CanvasResizeOverlay
          kind="frame"
          bounds={{ x: 10, y: 20, width: 80, height: 40 }}
          viewport={{ x: 0, y: 0, scale: 1 }}
          onPreview={onPreview}
          onCommit={onCommit}
          identity="doc"
        />,
      );
      onPreview.mockClear();
      for (const name of [
        "canvas.resizeFrame bottom-right",
        "canvas.moveFrame",
      ]) {
        fireEvent.mouseDown(screen.getByRole("button", { name }), {
          button,
          clientX: 80,
          clientY: 40,
        });
        fireEvent.mouseMove(window, { clientX: 120, clientY: 80 });
        fireEvent.mouseUp(window, { button });
      }
      expect(onPreview).not.toHaveBeenCalled();
      expect(onCommit).not.toHaveBeenCalled();
    },
  );
});
