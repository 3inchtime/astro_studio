import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import { describe, it, expect, vi, afterEach } from "vitest";
import CanvasPropertiesPanel from "./CanvasPropertiesPanel";
import { createImageObject } from "../../lib/canvas/document";
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
afterEach(cleanup);
describe("selection properties", () => {
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
