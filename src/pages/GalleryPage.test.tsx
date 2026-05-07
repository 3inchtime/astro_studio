import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import GalleryPage from "./GalleryPage";
import type { GenerationResult } from "../types";

const searchGenerations = vi.fn();
const deleteGeneration = vi.fn();
const setActiveConversationId = vi.fn();
const navigate = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock("react-router-dom", () => ({
  useNavigate: () => navigate,
}));

vi.mock("../lib/api", () => ({
  deleteGeneration: (...args: unknown[]) => deleteGeneration(...args),
  searchGenerations: (...args: unknown[]) => searchGenerations(...args),
}));

vi.mock("../lib/editSources", () => ({
  savePendingEditSources: vi.fn(),
}));

vi.mock("../components/layout/AppLayout", () => ({
  useLayoutContext: () => ({
    setActiveConversationId,
  }),
}));

vi.mock("../components/gallery/EmptyCollectionState", () => ({
  default: () => <div data-testid="empty-state" />,
}));

vi.mock("../components/gallery/GenerationGrid", () => ({
  default: ({
    results,
    onPreview,
    onSelect,
  }: {
    results: GenerationResult[];
    onPreview?: (result: GenerationResult, imageIndex: number) => void;
    onSelect: (result: GenerationResult) => void;
  }) => (
    <div data-testid="grid">
      {results.map((result) => (
        <div key={result.generation.id}>
          <button onClick={() => onPreview?.(result, 1)}>
            Open preview {result.generation.id}
          </button>
          <button onClick={() => onSelect(result)}>
            Open detail {result.generation.id}
          </button>
        </div>
      ))}
    </div>
  ),
}));

vi.mock("../components/gallery/GenerationDetailPanel", () => ({
  default: ({
    result,
    onPreview,
  }: {
    result: GenerationResult;
    onPreview?: (imageIndex: number) => void;
  }) => (
    <div data-testid="detail-panel">
      <button onClick={() => onPreview?.(0)}>
        Preview detail {result.generation.id}
      </button>
    </div>
  ),
}));

vi.mock("../components/lightbox/Lightbox", () => ({
  default: ({
    images,
    initialIndex,
  }: {
    images: Array<{
      imageId: string;
      generationId: string;
      path: string;
      thumbnailPath?: string;
    }>;
    initialIndex: number;
  }) => (
    <div data-testid="lightbox">
      <span data-testid="lightbox-index">{initialIndex}</span>
      {images.map((image) => (
        <span key={image.imageId} data-testid="lightbox-image">
          {image.imageId}:{image.generationId}:{image.path}:{image.thumbnailPath}
        </span>
      ))}
    </div>
  ),
}));

vi.mock("../components/gallery/PaginationControls", () => ({
  default: () => null,
}));

vi.mock("../components/favorites/FolderSelector", () => ({
  default: () => null,
}));

let intersectionCallback: IntersectionObserverCallback | null = null;

function triggerIntersection(isIntersecting = true) {
  if (!intersectionCallback) {
    throw new Error("IntersectionObserver was not initialized");
  }

  intersectionCallback(
    [
      {
        isIntersecting,
        target: document.createElement("div"),
        intersectionRatio: isIntersecting ? 1 : 0,
        time: 0,
        boundingClientRect: {} as DOMRectReadOnly,
        intersectionRect: {} as DOMRectReadOnly,
        rootBounds: null,
      } as IntersectionObserverEntry,
    ],
    {} as IntersectionObserver,
  );
}

function buildResult(id = "1"): GenerationResult {
  const imageIdPrefix = id === "1" ? "image" : `image-${id}`;
  const fullPrefix = id === "1" ? "/tmp/full" : `/tmp/full-${id}`;
  const thumbPrefix = id === "1" ? "/tmp/thumb" : `/tmp/thumb-${id}`;

  return {
    generation: {
      id: `generation-${id}`,
      prompt: `Moonlit observatory ${id}`,
      engine: "gpt-image-2",
      request_kind: "generate",
      size: "1024x1024",
      quality: "auto",
      background: "auto",
      output_format: "png",
      output_compression: 100,
      moderation: "auto",
      input_fidelity: "high",
      image_count: 2,
      source_image_count: 0,
      source_image_paths: [],
      status: "completed",
      created_at: "2026-05-07T00:00:00Z",
      deleted_at: null,
    },
    images: [
      {
        id: `${imageIdPrefix}-1`,
        generation_id: `generation-${id}`,
        file_path: `${fullPrefix}-1.png`,
        thumbnail_path: `${thumbPrefix}-1.png`,
        width: 1024,
        height: 1024,
        file_size: 1024,
      },
      {
        id: `${imageIdPrefix}-2`,
        generation_id: `generation-${id}`,
        file_path: `${fullPrefix}-2.png`,
        thumbnail_path: `${thumbPrefix}-2.png`,
        width: 1536,
        height: 1024,
        file_size: 2048,
      },
    ],
  };
}

describe("GalleryPage", () => {
  beforeEach(() => {
    searchGenerations.mockReset();
    deleteGeneration.mockReset();
    setActiveConversationId.mockReset();
    navigate.mockReset();
    intersectionCallback = null;

    vi.stubGlobal(
      "IntersectionObserver",
      vi.fn((callback: IntersectionObserverCallback) => {
        intersectionCallback = callback;
        return {
          observe: vi.fn(),
          unobserve: vi.fn(),
          disconnect: vi.fn(),
          root: null,
          rootMargin: "",
          thresholds: [],
          takeRecords: vi.fn(),
        };
      }),
    );

    searchGenerations.mockResolvedValue({
      generations: [],
      total: 0,
      page: 1,
      page_size: 20,
    });
  });

  it("searches with prompt, model, and date only", async () => {
    render(<GalleryPage />);

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenCalledWith(undefined, 1, false, {}, undefined);
    });

    fireEvent.change(screen.getByLabelText("gallery.filterModel"), {
      target: { value: "gpt-image-2" },
    });
    fireEvent.change(screen.getByLabelText("gallery.filterCreatedFrom"), {
      target: { value: "2026-05-01" },
    });
    fireEvent.change(screen.getByPlaceholderText("gallery.search"), {
      target: { value: "sunrise" },
    });
    fireEvent.click(screen.getByRole("button", { name: "gallery.applyFilters" }));

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenLastCalledWith("sunrise", 1, false, {
        model: "gpt-image-2",
        created_from: "2026-05-01",
      }, undefined);
    });

    expect(screen.queryByLabelText("gallery.filterStatus")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("gallery.filterQuality")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("gallery.filterSources")).not.toBeInTheDocument();
  });

  it("searches with the current query and filters when pressing Enter", async () => {
    render(<GalleryPage />);

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenCalledWith(undefined, 1, false, {}, undefined);
    });

    fireEvent.change(screen.getByLabelText("gallery.filterCreatedTo"), {
      target: { value: "2026-05-31" },
    });
    fireEvent.change(screen.getByPlaceholderText("gallery.search"), {
      target: { value: "nebula" },
    });
    fireEvent.keyDown(screen.getByPlaceholderText("gallery.search"), {
      key: "Enter",
    });

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenLastCalledWith("nebula", 1, false, {
        created_to: "2026-05-31",
      }, undefined);
    });
  });

  it("compacts date filters through the gallery search action", async () => {
    render(<GalleryPage />);

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenCalledWith(undefined, 1, false, {}, undefined);
    });

    fireEvent.change(screen.getByLabelText("gallery.filterCreatedFrom"), {
      target: { value: "2026-05-01" },
    });
    fireEvent.change(screen.getByLabelText("gallery.filterCreatedTo"), {
      target: { value: "" },
    });
    fireEvent.click(screen.getByRole("button", { name: "gallery.applyFilters" }));

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenLastCalledWith(undefined, 1, false, {
        created_from: "2026-05-01",
      }, undefined);
    });
  });

  it("enables reset when filters are active and searches with cleared filters", async () => {
    render(<GalleryPage />);

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenCalledWith(undefined, 1, false, {}, undefined);
    });

    const resetButton = screen.getByRole("button", {
      name: "gallery.resetFilters",
    });
    expect(resetButton).toBeDisabled();

    fireEvent.change(screen.getByLabelText("gallery.filterCreatedFrom"), {
      target: { value: "2026-05-01" },
    });
    expect(resetButton).toBeEnabled();

    fireEvent.click(resetButton);

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenLastCalledWith(undefined, 1, false, {}, undefined);
    });
    expect(screen.getByLabelText("gallery.filterCreatedFrom")).toHaveValue("");
  });

  it("reset clears an active text query", async () => {
    render(<GalleryPage />);

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenCalledWith(undefined, 1, false, {}, undefined);
    });

    const searchInput = screen.getByPlaceholderText("gallery.search");
    const resetButton = screen.getByRole("button", {
      name: "gallery.resetFilters",
    });

    fireEvent.change(searchInput, {
      target: { value: "portrait" },
    });
    expect(resetButton).toBeEnabled();

    fireEvent.click(resetButton);

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenLastCalledWith(undefined, 1, false, {}, undefined);
    });
    expect(searchInput).toHaveValue("");
  });

  it("opens the shared lightbox from a gallery image with all result images", async () => {
    searchGenerations.mockResolvedValueOnce({
      generations: [buildResult()],
      total: 1,
      page: 1,
      page_size: 20,
    });

    render(<GalleryPage />);

    await waitFor(() => {
      expect(screen.getByTestId("grid")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", {
      name: "Open preview generation-1",
    }));

    expect(screen.getByTestId("lightbox-index")).toHaveTextContent("1");
    expect(screen.getAllByTestId("lightbox-image")).toHaveLength(2);
    expect(screen.getAllByTestId("lightbox-image")[0]).toHaveTextContent(
      "image-1:generation-1:/tmp/full-1.png:/tmp/thumb-1.png",
    );
    expect(screen.getAllByTestId("lightbox-image")[1]).toHaveTextContent(
      "image-2:generation-1:/tmp/full-2.png:/tmp/thumb-2.png",
    );
  });

  it("opens the shared lightbox from the detail panel image", async () => {
    searchGenerations.mockResolvedValueOnce({
      generations: [buildResult()],
      total: 1,
      page: 1,
      page_size: 20,
    });

    render(<GalleryPage />);

    await waitFor(() => {
      expect(screen.getByTestId("grid")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", {
      name: "Open detail generation-1",
    }));
    fireEvent.click(screen.getByRole("button", {
      name: "Preview detail generation-1",
    }));

    expect(screen.getByTestId("lightbox-index")).toHaveTextContent("0");
    expect(screen.getAllByTestId("lightbox-image")).toHaveLength(2);
  });

  it("loads the next gallery page when the waterfall sentinel becomes visible", async () => {
    searchGenerations
      .mockResolvedValueOnce({
        generations: [buildResult("1")],
        total: 2,
        page: 1,
        page_size: 1,
      })
      .mockResolvedValueOnce({
        generations: [buildResult("2")],
        total: 2,
        page: 2,
        page_size: 1,
      });

    render(<GalleryPage />);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Open preview generation-1" })).toBeInTheDocument();
    });

    await act(async () => {
      triggerIntersection();
    });

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenLastCalledWith(undefined, 2, false, {}, undefined);
    });

    expect(screen.getByRole("button", { name: "Open preview generation-1" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open preview generation-2" })).toBeInTheDocument();
  });
});
