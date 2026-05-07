import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import Lightbox from "./Lightbox";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) =>
      ({
        "lightbox.preview": "Preview",
        "lightbox.zoomIn": "Zoom In",
        "lightbox.zoomOut": "Zoom Out",
        "lightbox.reset": "Reset",
        "lightbox.copy": "Copy",
        "lightbox.download": "Download",
        "lightbox.edit": "Edit",
        "lightbox.delete": "Delete",
        "favorites.addToFolder": "Add to folder",
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

vi.mock("../favorites/FolderSelector", () => ({
  default: () => <div data-testid="folder-selector" />,
}));

describe("Lightbox", () => {
  it("deletes the currently displayed image by generation id", () => {
    const onDelete = vi.fn();

    render(
      <Lightbox
        images={[
          {
            imageId: "image-1",
            generationId: "generation-1",
            path: "/tmp/image.png",
            thumbnailPath: "/tmp/thumb.png",
          },
        ]}
        initialIndex={0}
        onClose={vi.fn()}
        onDelete={onDelete}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    expect(onDelete).toHaveBeenCalledWith("generation-1");
  });

  it("renders a direct image preview without a lightbox frame", () => {
    render(
      <Lightbox
        images={[
          {
            imageId: "image-1",
            generationId: "generation-1",
            path: "/tmp/image.png",
            thumbnailPath: "/tmp/thumb.png",
          },
        ]}
        initialIndex={0}
        onClose={vi.fn()}
      />,
    );

    const image = screen.getByAltText("Preview");
    const viewport = screen.getByTestId("image-preview-viewport");

    expect(viewport).toHaveClass("overflow-hidden");
    expect(image.parentElement).toHaveClass("h-full", "w-full");
    expect(image.parentElement).not.toHaveClass(
      "rounded-[28px]",
      "border",
      "shadow-[0_24px_80px_rgba(0,0,0,0.42)]",
    );

    fireEvent.wheel(viewport, { deltaY: -1000 });

    expect(image).toHaveStyle({
      transform: "scale(2) translate(0px, 0px)",
    });
  });

  it("falls back to the thumbnail when the full-resolution lightbox image fails", () => {
    render(
      <Lightbox
        images={[
          {
            imageId: "image-1",
            generationId: "generation-1",
            path: "/tmp/image.jpeg",
            thumbnailPath: "/tmp/thumb.png",
          },
        ]}
        initialIndex={0}
        onClose={vi.fn()}
      />,
    );

    const image = screen.getByAltText("Preview");
    fireEvent.error(image);

    expect(image).toHaveAttribute("src", "/tmp/thumb.png");
  });

  it("closes when clicking the empty preview area but not the image itself", () => {
    const onClose = vi.fn();

    render(
      <Lightbox
        images={[
          {
            imageId: "image-1",
            generationId: "generation-1",
            path: "/tmp/image.png",
            thumbnailPath: "/tmp/thumb.png",
          },
        ]}
        initialIndex={0}
        onClose={onClose}
      />,
    );

    fireEvent.click(screen.getByAltText("Preview"));
    expect(onClose).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTestId("image-preview-viewport"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
