import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ImageGrid from "./ImageGrid";
import { copyImageToClipboard, saveImageToFile } from "../../lib/api";

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
  it("offers a native keyboard-accessible image preview button", () => {
    const onImageClick = vi.fn();
    const images = [
      {
        path: "/tmp/image.png",
        imageId: "image-1",
        generationId: "generation-1",
      },
    ];
    render(<ImageGrid images={images} onImageClick={onImageClick} />);
    const preview = screen.getByRole("button", { name: "studio.openImage" });
    expect(preview.tagName).toBe("BUTTON");
    fireEvent.click(preview);
    expect(onImageClick).toHaveBeenCalledWith(images, 0);
  });

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
    const controls = screen.getByRole("button", {
      name: "Delete",
    }).parentElement;
    const imageFrame = screen.getByAltText("Generated").parentElement;

    expect(root).toHaveClass("inline-flex", "flex-col", "items-start");
    expect(item).toHaveClass("w-fit");
    expect(controls).toHaveClass("mx-auto", "w-fit");
    expect(imageFrame).toHaveClass("h-[min(52cqh,520px)]");
  });

  it("keeps image actions visible below the preview", () => {
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

    const controls = screen.getByRole("button", {
      name: "Delete",
    }).parentElement;

    expect(controls).not.toHaveClass("absolute", "opacity-0");
    expect(controls).toHaveClass("border-t");
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
  it.each(["copy", "save"])(
    "shows %s failures and lets the user retry",
    async (kind) => {
      const action = vi.mocked(
        kind === "copy" ? copyImageToClipboard : saveImageToFile,
      );
      action
        .mockReset()
        .mockRejectedValueOnce(new Error("Not available"))
        .mockResolvedValueOnce(undefined);
      render(
        <ImageGrid
          images={[
            {
              path: "/tmp/action.png",
              imageId: "action",
              generationId: "generation-action",
            },
          ]}
          onImageClick={vi.fn()}
        />,
      );
      const button = screen.getByRole("button", {
        name: kind === "copy" ? "lightbox.copy" : "lightbox.download",
      });
      fireEvent.click(button);
      expect(await screen.findByRole("alert")).toHaveTextContent(
        kind === "copy" ? "studio.copyFailed" : "studio.saveImageFailed",
      );
      fireEvent.click(button);
      await waitFor(() =>
        expect(screen.queryByRole("alert")).not.toBeInTheDocument(),
      );
      expect(action).toHaveBeenCalledTimes(2);
    },
  );
  it("fits a result to the available feed height budget", () => {
    render(
      <ImageGrid
        images={[{ path: "/tmp/fit.png", imageId: "fit", generationId: "fit" }]}
        maxImageHeight={220}
        onImageClick={vi.fn()}
      />,
    );
    expect(
      screen.getByRole("button", { name: "studio.openImage" }),
    ).toHaveStyle({ height: "220px" });
  });
});
