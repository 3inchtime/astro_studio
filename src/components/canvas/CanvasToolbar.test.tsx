import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import CanvasToolbar from "./CanvasToolbar";
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
const props = {
  activeTool: "select" as const,
  strokeColor: "#000000",
  strokeSize: 3,
  canUndo: false,
  canRedo: false,
  selectedObjectCount: 2,
  zoomPercent: 100,
  canPaste: true,
  onToolChange: vi.fn(),
  onColorChange: vi.fn(),
  onSizeChange: vi.fn(),
  onUndo: vi.fn(),
  onRedo: vi.fn(),
  onZoomIn: vi.fn(),
  onZoomOut: vi.fn(),
  onImportImage: vi.fn(),
  onDeleteSelection: vi.fn(),
  onCopySelection: vi.fn(),
  onPasteSelection: vi.fn(),
  onReorderSelection: vi.fn(),
  onFitFrame: vi.fn(),
  onFitSelection: vi.fn(),
};
describe("canvas tool panel", () => {
  it("provides shape and text tools with selected state and shortcut hints", () => {
    render(<CanvasToolbar {...props} />);
    fireEvent.click(
      screen.getByRole("button", { name: "canvas.tool.rectangle" }),
    );
    expect(props.onToolChange).toHaveBeenCalledWith("rectangle");
    expect(
      screen.getByRole("button", { name: "canvas.tool.text" }),
    ).toHaveAttribute("title", expect.stringContaining("T"));
    expect(
      screen.getByRole("button", { name: "canvas.tool.select" }),
    ).toHaveAttribute("aria-pressed", "true");
  });
  it("keeps object actions in a compact menu with alignment controls", () => {
    const align = vi.fn();
    render(<CanvasToolbar {...props} onAlignSelection={align} />);
    fireEvent.click(screen.getByRole("button", { name: "canvas.actions" }));
    fireEvent.click(
      screen.getByRole("menuitem", { name: "canvas.align.left" }),
    );
    expect(align).toHaveBeenCalledWith("left");
  });
});
