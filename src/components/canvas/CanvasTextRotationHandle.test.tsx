import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import CanvasTextRotationHandle from "./CanvasTextRotationHandle";
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
afterEach(cleanup);
const object = {
  type: "text" as const,
  id: "t",
  text: "Hello",
  x: 0,
  y: 0,
  width: 100,
  height: 40,
  fontSize: 20,
  color: "#000",
  opacity: 1,
  rotation: 0,
};
it("previews around the center at zoom and commits once, with Shift snapping", () => {
  const onPreview = vi.fn(),
    onCommit = vi.fn();
  render(
    <CanvasTextRotationHandle
      object={object}
      viewport={{ x: 10, y: 20, scale: 2 }}
      identity="doc"
      onPreview={onPreview}
      onCommit={onCommit}
    />,
  );
  fireEvent.mouseDown(screen.getByRole("button"), { clientX: 110, clientY: 0 });
  fireEvent.mouseMove(window, { clientX: 170, clientY: 60, shiftKey: true });
  expect(onPreview).toHaveBeenLastCalledWith(90);
  expect(onCommit).not.toHaveBeenCalled();
  fireEvent.mouseUp(window);
  expect(onCommit).toHaveBeenCalledExactlyOnceWith(90);
});
it.each(["escape", "blur", "switch"])("cancels rotation on %s", (reason) => {
  const onPreview = vi.fn(),
    onCommit = vi.fn();
  const props = {
    object,
    viewport: { x: 0, y: 0, scale: 1 },
    identity: "a",
    onPreview,
    onCommit,
  };
  const view = render(<CanvasTextRotationHandle {...props} />);
  fireEvent.mouseDown(screen.getByRole("button"), {
    clientX: 50,
    clientY: -20,
  });
  fireEvent.mouseMove(window, { clientX: 100, clientY: 20 });
  if (reason === "escape") fireEvent.keyDown(window, { key: "Escape" });
  if (reason === "blur") fireEvent.blur(window);
  if (reason === "switch")
    view.rerender(<CanvasTextRotationHandle {...props} identity="b" />);
  fireEvent.mouseUp(window);
  expect(onCommit).not.toHaveBeenCalled();
  expect(onPreview).toHaveBeenLastCalledWith(null);
});
