import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import GenerationGrid from "./GenerationGrid";
import type { GenerationResult } from "../../types";

vi.mock("../../lib/api", () => ({
  toAssetUrl: (path: string) => path,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock("../favorites/FavoriteButton", () => ({
  default: ({ imageId }: { imageId: string }) => (
    <div data-testid={`favorite-${imageId}`} />
  ),
}));

function buildResult(
  id: string,
  width: number,
  height: number,
): GenerationResult {
  return {
    generation: {
      id: `generation-${id}`,
      prompt: `Prompt ${id}`,
      engine: "gpt-image-2",
      request_kind: "generate",
      size: `${width}x${height}`,
      quality: "auto",
      background: "auto",
      output_format: "png",
      output_compression: 100,
      moderation: "auto",
      input_fidelity: "high",
      image_count: 1,
      source_image_count: 0,
      source_image_paths: [],
      status: "completed",
      created_at: "2026-05-07T00:00:00Z",
      deleted_at: null,
    },
    images: [
      {
        id: `image-${id}`,
        generation_id: `generation-${id}`,
        file_path: `/tmp/${id}.png`,
        thumbnail_path: `/tmp/${id}-thumb.png`,
        width,
        height,
        file_size: 1024,
      },
    ],
  };
}

describe("GenerationGrid", () => {
  it("uses a masonry column layout so mixed aspect ratios do not leave stretched row gaps", () => {
    const { container } = render(
      <GenerationGrid
        results={[
          buildResult("wide", 1536, 1024),
          buildResult("square", 1024, 1024),
          buildResult("portrait", 1024, 1536),
        ]}
        onSelect={vi.fn()}
      />,
    );

    const grid = container.firstElementChild;
    const firstCard = grid?.firstElementChild;

    expect(grid).toHaveClass("columns-2", "sm:columns-3", "lg:columns-4");
    expect(grid).not.toHaveClass("grid");
    expect(firstCard).toHaveClass("mb-3", "break-inside-avoid");
  });

  it("sizes gallery previews from each image aspect ratio instead of forcing squares", () => {
    render(
      <GenerationGrid
        results={[
          buildResult("wide", 1536, 1024),
          buildResult("portrait", 1024, 1536),
        ]}
        onSelect={vi.fn()}
      />,
    );

    const wideFrame = screen.getByAltText("Prompt wide").parentElement;
    const portraitFrame = screen.getByAltText("Prompt portrait").parentElement;

    expect(wideFrame).toHaveStyle({ aspectRatio: "1536 / 1024" });
    expect(portraitFrame).toHaveStyle({ aspectRatio: "1024 / 1536" });
    expect(screen.getByAltText("Prompt wide")).not.toHaveClass("aspect-square");
    expect(screen.getByAltText("Prompt portrait")).not.toHaveClass(
      "aspect-square",
    );
  });

  it("keeps masonry previews out of hover transforms so images stay painted in columns", () => {
    const { container } = render(
      <GenerationGrid
        results={[buildResult("wide", 1536, 1024)]}
        onSelect={vi.fn()}
      />,
    );

    const card = container.firstElementChild?.firstElementChild;
    const image = screen.getByAltText("Prompt wide");

    expect(card?.className).not.toContain("hover:-translate-y");
    expect(image.className).not.toContain("group-hover:scale");
    expect(image.className).not.toContain("transition-transform");
  });

  it("opens the preview from the image area and opens details from a dedicated button", () => {
    const onPreview = vi.fn();
    const onSelect = vi.fn();

    render(
      <GenerationGrid
        results={[buildResult("wide", 1536, 1024)]}
        onPreview={onPreview}
        onSelect={onSelect}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Preview Prompt wide" }));

    expect(onPreview).toHaveBeenCalledWith(expect.objectContaining({
      generation: expect.objectContaining({ id: "generation-wide" }),
    }), 0);
    expect(onSelect).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "gallery.detail" }));

    expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({
      generation: expect.objectContaining({ id: "generation-wide" }),
    }));
  });

  it("renders results as a compact list when list view is selected", () => {
    const { container } = render(
      <GenerationGrid
        results={[buildResult("wide", 1536, 1024)]}
        viewMode="list"
        onSelect={vi.fn()}
      />,
    );

    const list = container.firstElementChild;
    const row = list?.firstElementChild;

    expect(list).toHaveClass("space-y-2");
    expect(list).not.toHaveClass("columns-2");
    expect(row).toHaveClass("grid", "grid-cols-[104px_minmax(0,1fr)_auto]");
    expect(screen.getByText("Prompt wide")).toBeInTheDocument();
    expect(screen.getByText("gpt-image-2")).toBeInTheDocument();
    expect(screen.getByText("1536x1024")).toBeInTheDocument();
  });
});
