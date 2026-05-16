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
  it("uses the original image for single-image previews so large renders stay sharp", () => {
    render(
      <ImageGrid
        images={[
          {
            path: "/tmp/full-resolution.jpeg",
            thumbnail: "/tmp/thumbnail.png",
            imageId: "image-sharp",
            generationId: "generation-sharp",
          },
        ]}
        onImageClick={vi.fn()}
      />,
    );

    expect(screen.getByAltText("Generated")).toHaveAttribute(
      "src",
      "/tmp/full-resolution.jpeg",
    );
  });

  it("falls back to the thumbnail when a single-image preview fails to load", () => {
    render(
      <ImageGrid
        images={[
          {
            path: "/tmp/full-resolution.jpeg",
            thumbnail: "/tmp/thumbnail.png",
            imageId: "image-fallback",
            generationId: "generation-fallback",
          },
        ]}
        onImageClick={vi.fn()}
      />,
    );

    const image = screen.getByAltText("Generated");
    fireEvent.error(image);

    expect(image).toHaveAttribute("src", "/tmp/thumbnail.png");
  });

  it("resets a single-image preview when the rendered image changes", () => {
    const { rerender } = render(
      <ImageGrid
        images={[
          {
            path: "/tmp/full-resolution-a.jpeg",
            thumbnail: "/tmp/thumbnail-a.png",
            imageId: "image-a",
            generationId: "generation-a",
          },
        ]}
        onImageClick={vi.fn()}
      />,
    );

    fireEvent.error(screen.getByAltText("Generated"));
    expect(screen.getByAltText("Generated")).toHaveAttribute(
      "src",
      "/tmp/thumbnail-a.png",
    );

    rerender(
      <ImageGrid
        images={[
          {
            path: "/tmp/full-resolution-b.jpeg",
            thumbnail: "/tmp/thumbnail-b.png",
            imageId: "image-b",
            generationId: "generation-b",
          },
        ]}
        onImageClick={vi.fn()}
      />,
    );

    expect(screen.getByAltText("Generated")).toHaveAttribute(
      "src",
      "/tmp/full-resolution-b.jpeg",
    );
  });

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

  it("renders image actions as an overlay tool cluster", () => {
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
      />,
    );

    const controls = screen
      .getByRole("button", { name: "Delete" })
      .parentElement;

    expect(controls).toHaveClass("absolute", "bottom-3", "opacity-0");
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

  it("keeps using thumbnails for multi-image grids", () => {
    render(
      <ImageGrid
        images={[
          {
            path: "/tmp/full-image-1.jpeg",
            thumbnail: "/tmp/thumb-1.png",
            imageId: "image-1",
            generationId: "generation-1",
          },
          {
            path: "/tmp/full-image-2.jpeg",
            thumbnail: "/tmp/thumb-2.png",
            imageId: "image-2",
            generationId: "generation-2",
          },
        ]}
        onImageClick={vi.fn()}
      />,
    );

    const renderedImages = screen.getAllByAltText("Generated");

    expect(renderedImages[0]).toHaveAttribute("src", "/tmp/thumb-1.png");
    expect(renderedImages[1]).toHaveAttribute("src", "/tmp/thumb-2.png");
  });
});
