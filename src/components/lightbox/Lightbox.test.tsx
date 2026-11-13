import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Lightbox from "./Lightbox";
import { copyImageToClipboard, saveImageToFile } from "../../lib/api";

vi.mock("react-i18next", () => {
  const t = (key: string) =>
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
    })[key] ?? key;
  return { useTranslation: () => ({ t }) };
});

vi.mock("../../lib/api", () => ({
  toAssetUrl: (path: string) => path,
  copyImageToClipboard: vi.fn(),
  saveImageToFile: vi.fn(),
  getFolders: vi.fn().mockResolvedValue([]),
  getImageFolders: vi.fn().mockResolvedValue([]),
  createFolder: vi.fn(),
  addImageToFolders: vi.fn(),
  removeImageFromFolders: vi.fn(),
}));

vi.mock("../favorites/FavoriteButton", () => ({
  default: ({ onClick }: { onClick: () => void }) => (
    <button onClick={onClick}>Choose folder</button>
  ),
}));

const images = [1, 2].map((number) => ({
  imageId: `image-${number}`,
  generationId: `generation-${number}`,
  path: `/tmp/image-${number}.png`,
}));

describe("Lightbox", () => {
  beforeEach(() => {
    vi.mocked(copyImageToClipboard).mockReset().mockResolvedValue(undefined);
    vi.mocked(saveImageToFile).mockReset().mockResolvedValue(undefined);
  });

  it("handles empty images before and after rendering without breaking hooks or navigation", () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const onClose = vi.fn();
    const { rerender } = render(
      <Lightbox images={[]} initialIndex={0} onClose={onClose} />,
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    fireEvent.keyDown(document, { key: "ArrowLeft" });
    rerender(<Lightbox images={images} initialIndex={0} onClose={onClose} />);
    expect(screen.getByRole("dialog", { name: "Preview" })).toBeInTheDocument();
    fireEvent.keyDown(document, { key: "ArrowRight" });
    rerender(<Lightbox images={[]} initialIndex={0} onClose={onClose} />);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    rerender(
      <Lightbox images={[images[0]]} initialIndex={0} onClose={onClose} />,
    );
    expect(screen.getByText("1 / 1")).toBeInTheDocument();
    expect(errorSpy).not.toHaveBeenCalled();
    errorSpy.mockRestore();
  });

  it("navigates with arrow keys and wraps at either end", () => {
    render(<Lightbox images={images} initialIndex={0} onClose={vi.fn()} />);
    fireEvent.keyDown(document.activeElement!, { key: "ArrowLeft" });
    expect(screen.getByText("2 / 2")).toBeInTheDocument();
    fireEvent.keyDown(document.activeElement!, { key: "ArrowRight" });
    expect(screen.getByText("1 / 2")).toBeInTheDocument();
  });

  it("clears copied and failure feedback when changing images", async () => {
    render(<Lightbox images={images} initialIndex={0} onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "studio.copied",
    );
    fireEvent.keyDown(document, { key: "ArrowRight" });
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    vi.mocked(saveImageToFile).mockRejectedValueOnce(new Error("No access"));
    fireEvent.click(screen.getByRole("button", { name: "Download" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "studio.saveImageFailed",
    );
    fireEvent.keyDown(document, { key: "ArrowLeft" });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it.each(["copy success", "copy failure", "save failure"])(
    "ignores a delayed %s after navigating to a different image",
    async (action) => {
      let resolve!: () => void;
      let reject!: (error: Error) => void;
      const pending = new Promise<void>((res, rej) => {
        resolve = res;
        reject = rej;
      });
      if (action.startsWith("copy"))
        vi.mocked(copyImageToClipboard).mockReturnValueOnce(pending);
      else vi.mocked(saveImageToFile).mockReturnValueOnce(pending);
      render(<Lightbox images={images} initialIndex={0} onClose={vi.fn()} />);
      fireEvent.click(
        screen.getByRole("button", {
          name: action.startsWith("copy") ? "Copy" : "Download",
        }),
      );
      fireEvent.keyDown(document, { key: "ArrowRight" });
      await act(async () => {
        if (action === "copy success") resolve();
        else reject(new Error("Old image failed"));
      });
      expect(screen.getByText("2 / 2")).toBeInTheDocument();
      expect(screen.queryByRole("status")).not.toBeInTheDocument();
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    },
  );

  it("closes only the nested folder dialog on Escape and suspends image navigation", async () => {
    const onClose = vi.fn();
    render(<Lightbox images={images} initialIndex={0} onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: "Choose folder" }));
    const folderDialog = await screen.findByRole("dialog", {
      name: "Add to folder",
    });
    await waitFor(() =>
      expect(
        within(folderDialog).queryByRole("status"),
      ).not.toBeInTheDocument(),
    );
    fireEvent.keyDown(document.activeElement!, { key: "ArrowRight" });
    expect(screen.getByText("1 / 2")).toBeInTheDocument();
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(
      screen.queryByRole("dialog", { name: "Add to folder" }),
    ).not.toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

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
    expect(image.parentElement).toHaveClass("absolute", "inset-0");
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

  it("retains a thumbnail until the full image has loaded", () => {
    render(<Lightbox images={[{ ...images[0], thumbnailPath: "/tmp/thumb.png" }]} initialIndex={0} onClose={vi.fn()} />);
    const full = screen.getByAltText("Preview");
    expect(screen.getByTestId("preview-thumbnail")).toBeInTheDocument();
    expect(full).toHaveStyle({ opacity: "0" });
    fireEvent.load(full);
    expect(full).toHaveStyle({ opacity: "1" });
    expect(screen.queryByTestId("preview-thumbnail")).not.toBeInTheDocument();
  });

  it("keeps the full-resolution source for the lightbox image", () => {
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

    expect(image).toHaveAttribute("src", "/tmp/image.jpeg");
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
