import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ImageGrid from "./ImageGrid";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) =>
      ({
        "lightbox.edit": "Edit",
        "lightbox.delete": "Delete",
      })[key] ?? key,
  }),
}));

vi.mock("../../lib/api", () => ({
  toAssetUrl: (path: string) => path,
  copyImageToClipboard: vi.fn(),
  saveImageToFile: vi.fn(),
}));

vi.mock("../favorites/FavoriteButton", () => ({
  default: () => <div data-testid="favorite-button" />,
}));

describe("ImageGrid", () => {
  it("renders single-image cards larger so portrait images stay wider than the action row", () => {
    const { container } = render(
      <ImageGrid
        images={[
          {
            path: "/tmp/portrait-image.png",
            thumbnail: "/tmp/portrait-thumb.png",
            imageId: "image-portrait",
            generationId: "generation-portrait",
          },
        ]}
        onImageClick={vi.fn()}
      />,
    );

    const root = container.firstElementChild;
    const item = container.querySelector('div[class*="w-fit"]');
    const controls = screen
      .getByRole("button", { name: "Delete" })
      .parentElement;
    const imageFrame = screen.getByAltText("Generated").parentElement;

    expect(root).toHaveClass("inline-flex", "flex-col", "items-start");
    expect(item).toHaveClass("w-fit");
    expect(controls).toHaveClass("mx-auto", "w-fit");
    expect(imageFrame).toHaveClass("h-72");
  });

  it("passes the generation id to delete without relying on native confirm", () => {
    const onDelete = vi.fn();

    render(
      <ImageGrid
        images={[
          {
            path: "/tmp/image.png",
            thumbnail: "/tmp/thumb.png",
            imageId: "image-1",
            generationId: "generation-1",
          },
        ]}
        onImageClick={vi.fn()}
        onDelete={onDelete}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    expect(onDelete).toHaveBeenCalledWith("generation-1");
  });
});
