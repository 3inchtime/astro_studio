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
});
