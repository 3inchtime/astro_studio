import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import FavoritesPage from "./FavoritesPage";
import type { GenerationResult } from "../types";

const deleteGeneration = vi.fn();
const deletePromptFavorite = vi.fn();
const getFavoriteImages = vi.fn();
const getPromptFavorites = vi.fn();
const navigate = vi.fn();
const reloadFolders = vi.fn();
const reloadPromptFolders = vi.fn();
const savePendingEditSources = vi.fn();
const setActiveConversationId = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock("../lib/api", () => ({
  deleteGeneration: (...args: unknown[]) => deleteGeneration(...args),
  deletePromptFavorite: (...args: unknown[]) => deletePromptFavorite(...args),
  getFavoriteImages: (...args: unknown[]) => getFavoriteImages(...args),
  getPromptFavorites: (...args: unknown[]) => getPromptFavorites(...args),
}));

vi.mock("react-router-dom", () => ({
  useNavigate: () => navigate,
}));

vi.mock("../lib/editSources", () => ({
  savePendingEditSources: (...args: unknown[]) => savePendingEditSources(...args),
}));

vi.mock("../components/layout/AppLayout", () => ({
  useLayoutContext: () => ({
    setActiveConversationId,
  }),
}));

vi.mock("../hooks/useFolders", () => ({
  useFolders: () => ({
    folders: [],
    reload: reloadFolders,
  }),
}));

vi.mock("../hooks/usePromptFolders", () => ({
  usePromptFolders: () => ({
    folders: [],
    reload: reloadPromptFolders,
  }),
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
    onEditImage,
  }: {
    images: Array<{
      imageId: string;
      generationId: string;
      path: string;
      thumbnailPath?: string;
    }>;
    initialIndex: number;
    onEditImage?: (image: {
      imageId: string;
      generationId: string;
      path: string;
      thumbnailPath?: string;
    }) => void;
  }) => (
    <div data-testid="lightbox">
      <span data-testid="lightbox-index">{initialIndex}</span>
      {images.map((image) => (
        <span key={image.imageId} data-testid="lightbox-image">
          {image.imageId}:{image.generationId}:{image.path}:{image.thumbnailPath}
        </span>
      ))}
      {images[initialIndex] && (
        <button onClick={() => onEditImage?.(images[initialIndex])}>
          Edit lightbox image
        </button>
      )}
    </div>
  ),
}));

vi.mock("../components/gallery/PaginationControls", () => ({
  default: () => null,
}));

vi.mock("../components/favorites/FolderSelector", () => ({
  default: () => null,
}));

vi.mock("../components/favorites/PromptFolderSelector", () => ({
  default: () => null,
}));

function buildResult(): GenerationResult {
  return {
    generation: {
      id: "generation-1",
      prompt: "Favorite observatory",
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
        id: "image-1",
        generation_id: "generation-1",
        file_path: "/tmp/favorite-full-1.png",
        thumbnail_path: "/tmp/favorite-thumb-1.png",
        width: 1024,
        height: 1024,
        file_size: 1024,
      },
      {
        id: "image-2",
        generation_id: "generation-1",
        file_path: "/tmp/favorite-full-2.png",
        thumbnail_path: "/tmp/favorite-thumb-2.png",
        width: 1536,
        height: 1024,
        file_size: 2048,
      },
    ],
  };
}

describe("FavoritesPage", () => {
  beforeEach(() => {
    deleteGeneration.mockReset();
    deletePromptFavorite.mockReset();
    getFavoriteImages.mockReset();
    getPromptFavorites.mockReset();
    navigate.mockReset();
    reloadFolders.mockReset();
    reloadPromptFolders.mockReset();
    savePendingEditSources.mockReset();
    setActiveConversationId.mockReset();

    getFavoriteImages.mockResolvedValue({
      generations: [buildResult()],
      total: 1,
      page: 1,
      page_size: 20,
    });
    getPromptFavorites.mockResolvedValue([]);
  });

  it("opens the shared lightbox from a favorite image with all result images", async () => {
    render(<FavoritesPage />);

    await waitFor(() => {
      expect(screen.getByTestId("grid")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", {
      name: "Open preview generation-1",
    }));

    expect(screen.getByTestId("lightbox-index")).toHaveTextContent("1");
    expect(screen.getAllByTestId("lightbox-image")).toHaveLength(2);
    expect(screen.getAllByTestId("lightbox-image")[0]).toHaveTextContent(
      "image-1:generation-1:/tmp/favorite-full-1.png:/tmp/favorite-thumb-1.png",
    );
  });

  it("opens the shared lightbox from the favorite detail panel image", async () => {
    render(<FavoritesPage />);

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

  it("uses a favorite lightbox image as an edit source", async () => {
    render(<FavoritesPage />);

    await waitFor(() => {
      expect(screen.getByTestId("grid")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", {
      name: "Open preview generation-1",
    }));
    fireEvent.click(screen.getByRole("button", { name: "Edit lightbox image" }));

    expect(savePendingEditSources).toHaveBeenCalledWith([
      {
        id: "image-2:/tmp/favorite-full-2.png",
        path: "/tmp/favorite-full-2.png",
        label: "favorite-full-2.png",
        imageId: "image-2",
        generationId: "generation-1",
      },
    ]);
    expect(setActiveConversationId).toHaveBeenCalledWith(null);
    expect(navigate).toHaveBeenCalledWith("/generate");
  });
});
