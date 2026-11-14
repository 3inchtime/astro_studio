import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import { describe, it, expect, vi, afterEach } from "vitest";
import CanvasPropertiesPanel from "./CanvasPropertiesPanel";
import { createImageObject } from "../../lib/canvas/document";
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
afterEach(cleanup);
describe("selection properties", () => {
  it("does not clamp a scaled spacing value when focus leaves without edits", () => {
    const change = vi.fn();
    render(
      <CanvasPropertiesPanel
        objects={[
          {
            type: "text",
            id: "t",
            x: 0,
            y: 0,
            width: 500,
            height: 30,
            rotation: 0,
            text: "AB",
            fontSize: 24,
            letterSpacing: 400,
            color: "#000000",
            opacity: 1,
          },
        ]}
        onChange={change}
      />,
    );
    const input = screen.getByLabelText("canvas.letterSpacing");
    fireEvent.focus(input);
    fireEvent.blur(input);
    expect(change).not.toHaveBeenCalled();
  });
  it("keeps numeric drafts until blur and accepts negative and fractional values", () => {
    const change = vi.fn();
    render(
      <CanvasPropertiesPanel
        objects={[
          {
            type: "text",
            id: "t",
            x: 0,
            y: 0,
            width: 100,
            height: 30,
            rotation: 0,
            text: "AB",
            fontSize: 24,
            color: "#000000",
            opacity: 1,
          },
        ]}
        onChange={change}
      />,
    );
    const spacing = screen.getByLabelText("canvas.letterSpacing");
    fireEvent.change(spacing, { target: { value: "" } });
    fireEvent.change(spacing, { target: { value: "-5" } });
    expect(change).not.toHaveBeenCalled();
    fireEvent.blur(spacing);
    expect(change).toHaveBeenLastCalledWith({ letterSpacing: -5 });
    const height = screen.getByLabelText("canvas.lineHeight");
    fireEvent.change(height, { target: { value: "0" } });
    fireEvent.change(height, { target: { value: "0.8" } });
    fireEvent.blur(height);
    expect(change).toHaveBeenLastCalledWith({ lineHeight: 0.8 });
  });
  it("exposes typography and rotation controls for legacy text objects", () => {
    const change = vi.fn();
    render(
      <CanvasPropertiesPanel
        objects={[
          {
            type: "text",
            id: "t",
            x: 0,
            y: 0,
            width: 100,
            height: 30,
            rotation: 0,
            text: "中文",
            fontSize: 24,
            color: "#000000",
            opacity: 1,
          },
        ]}
        onChange={change}
      />,
    );
    fireEvent.change(screen.getByLabelText("canvas.fontFamily"), {
      target: { value: "serif" },
    });
    expect(change).toHaveBeenLastCalledWith({ fontFamily: "serif" });
    fireEvent.click(screen.getByRole("button", { name: "canvas.bold" }));
    expect(change).toHaveBeenLastCalledWith({ fontWeight: "bold" });
    fireEvent.click(screen.getByRole("button", { name: "canvas.italic" }));
    expect(change).toHaveBeenLastCalledWith({ fontStyle: "italic" });
    fireEvent.click(
      screen.getByRole("button", { name: "canvas.textAlignCenter" }),
    );
    expect(change).toHaveBeenLastCalledWith({ textAlign: "center" });
    for (const [key, value] of [
      ["letterSpacing", 3],
      ["lineHeight", 1.8],
      ["rotation", 45],
    ] as const) {
      fireEvent.change(screen.getByLabelText(`canvas.${key}`), {
        target: { value: String(value) },
      });
      fireEvent.blur(screen.getByLabelText(`canvas.${key}`));
      expect(change).toHaveBeenLastCalledWith({ [key]: value });
    }
  });
  it("shows only opacity for selected images", () => {
    const change = vi.fn();
    render(
      <CanvasPropertiesPanel
        objects={[
          createImageObject({ image_path: "/a", width: 100, height: 100 }),
        ]}
        onChange={change}
      />,
    );
    expect(screen.queryByLabelText("canvas.fillColor")).not.toBeInTheDocument();
    expect(
      screen.queryByLabelText("canvas.strokeColor"),
    ).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("canvas.opacity"), {
      target: { value: "0.5" },
    });
    expect(change).toHaveBeenCalledWith({ opacity: 0.5 });
  });
  it("shows fill and stroke for shapes, font size and text color for text", () => {
    const change = vi.fn();
    const common = {
      id: "r",
      x: 0,
      y: 0,
      width: 100,
      height: 100,
      opacity: 1,
      rotation: 0,
      color: "#000000",
    };
    const view = render(
      <CanvasPropertiesPanel
        objects={[{ ...common, type: "rectangle", fill: "#ff0000", size: 2 }]}
        onChange={change}
      />,
    );
    expect(screen.getByLabelText("canvas.fillColor")).toBeInTheDocument();
    expect(screen.queryByLabelText("canvas.fontSize")).not.toBeInTheDocument();
    view.rerender(
      <CanvasPropertiesPanel
        objects={[{ ...common, type: "text", text: "hello", fontSize: 24 }]}
        onChange={change}
      />,
    );
    expect(screen.queryByLabelText("canvas.fillColor")).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("canvas.fontSize"), {
      target: { value: "48" },
    });
    expect(change).toHaveBeenCalledWith({ fontSize: 48 });
  });
});
