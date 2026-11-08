import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import ImageAdvancedControls from "./ImageAdvancedControls";
const pickSourceImages = vi.fn();
vi.mock("../../lib/api", () => ({
  pickSourceImages: () => pickSourceImages(),
}));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
beforeEach(() => vi.clearAllMocks());
it("offers GIF controls while hiding unsupported streaming and masks", () => {
  const onChange = vi.fn();
  render(
    <ImageAdvancedControls
      value={{}}
      onChange={onChange}
      format="gif"
      editing
    />,
  );
  const frames = screen.getByLabelText("generate.advanced.frameCount");
  fireEvent.change(frames, { target: { value: "12" } });
  expect(onChange).toHaveBeenCalledWith({ frameCount: 12 });
  expect(
    screen.queryByLabelText("generate.advanced.stream"),
  ).not.toBeInTheDocument();
  expect(screen.queryByText("generate.advanced.mask")).not.toBeInTheDocument();
});
it("keeps SVG author selection optional and enables reasoning only with an author", () => {
  const onChange = vi.fn();
  render(
    <ImageAdvancedControls
      value={{}}
      onChange={onChange}
      format="svg"
      editing={false}
    />,
  );
  expect(
    screen.getByLabelText("generate.advanced.svgReasoning"),
  ).toBeDisabled();
  fireEvent.change(screen.getByLabelText("generate.advanced.svgModel"), {
    target: { value: "my-author" },
  });
  expect(onChange).toHaveBeenCalledWith({ svg: { model: "my-author" } });
});
it("replaces a mask with a picked markup rather than combining overlays", async () => {
  pickSourceImages.mockResolvedValue(["/tmp/markup.png"]);
  const onChange = vi.fn();
  render(
    <ImageAdvancedControls
      value={{ maskPath: "/tmp/mask.png" }}
      onChange={onChange}
      format="png"
      editing
    />,
  );
  fireEvent.click(screen.getByText("generate.advanced.markup"));
  await vi.waitFor(() =>
    expect(onChange).toHaveBeenCalledWith({
      markupPath: "/tmp/markup.png",
      maskPath: undefined,
    }),
  );
});
