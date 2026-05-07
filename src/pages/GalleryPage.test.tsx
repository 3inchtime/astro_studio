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
});
