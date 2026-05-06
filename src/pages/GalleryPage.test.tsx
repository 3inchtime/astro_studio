import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import GalleryPage from "./GalleryPage";

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
  default: () => <div data-testid="grid" />,
}));

vi.mock("../components/gallery/GenerationDetailPanel", () => ({
  default: () => null,
}));

vi.mock("../components/gallery/PaginationControls", () => ({
  default: () => null,
}));

vi.mock("../components/favorites/FolderSelector", () => ({
  default: () => null,
}));

describe("GalleryPage", () => {
  beforeEach(() => {
    searchGenerations.mockReset();
    deleteGeneration.mockReset();
    setActiveConversationId.mockReset();
    navigate.mockReset();

    searchGenerations.mockResolvedValue({
      generations: [],
      total: 0,
      page: 1,
      page_size: 20,
    });
  });

  it("applies advanced search filters through the gallery search action", async () => {
    render(<GalleryPage />);

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenCalledWith(undefined, 1, false, {});
    });

    fireEvent.change(screen.getByLabelText("gallery.filterModel"), {
      target: { value: "gpt-image-2" },
    });
    fireEvent.change(screen.getByLabelText("gallery.filterSources"), {
      target: { value: "2" },
    });
    fireEvent.change(screen.getByPlaceholderText("gallery.search"), {
      target: { value: "sunrise" },
    });
    fireEvent.click(screen.getByRole("button", { name: "gallery.applyFilters" }));

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenLastCalledWith("sunrise", 1, false, {
        model: "gpt-image-2",
        source_image_count: "2",
      });
    });
  });
});
