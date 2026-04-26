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
