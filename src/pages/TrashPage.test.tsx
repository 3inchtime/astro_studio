import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import TrashPage from "./TrashPage";

const restoreGeneration = vi.fn();
const permanentlyDeleteGeneration = vi.fn();
const searchGenerations = vi.fn();
const getTrashSettings = vi.fn();
const refreshConversations = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { days?: number }) =>
      key === "trash.autoDeleteNotice"
        ? `Deleted images are permanently deleted after ${options?.days} days`
        : key,
  }),
}));

vi.mock("../lib/api", () => ({
  getTrashSettings: (...args: unknown[]) => getTrashSettings(...args),
  permanentlyDeleteGeneration: (...args: unknown[]) =>
    permanentlyDeleteGeneration(...args),
  restoreGeneration: (...args: unknown[]) => restoreGeneration(...args),
  searchGenerations: (...args: unknown[]) => searchGenerations(...args),
}));

vi.mock("../components/layout/AppLayout", () => ({
  useLayoutContext: () => ({
    refreshConversations,
  }),
}));

vi.mock("../components/gallery/EmptyCollectionState", () => ({
  default: () => <div data-testid="empty-state" />,
}));

vi.mock("../components/gallery/GenerationGrid", () => ({
  default: ({ onSelect }: { onSelect: (value: unknown) => void }) => (
    <button
      onClick={() =>
        onSelect({
          generation: {
            id: "generation-1",
            prompt: "Restorable prompt",
            created_at: "2026-04-26T00:00:00Z",
            deleted_at: "2026-04-26T01:00:00Z",
          },
          images: [],
        })
      }
    >
      Select generation
    </button>
  ),
}));

vi.mock("../components/gallery/PaginationControls", () => ({
  default: () => null,
}));

vi.mock("../components/gallery/GenerationDetailPanel", () => ({
  default: ({
    onRestore,
    onDelete,
  }: {
    onRestore: (generationId: string) => void;
    onDelete: (generationId: string) => void;
  }) => (
    <>
      <button onClick={() => onRestore("generation-1")}>Restore image</button>
      <button onClick={() => onDelete("generation-1")}>Delete image</button>
    </>
  ),
}));

describe("TrashPage", () => {
  beforeEach(() => {
    restoreGeneration.mockReset();
    searchGenerations.mockReset();
    getTrashSettings.mockReset();
    refreshConversations.mockReset();

    restoreGeneration.mockResolvedValue(undefined);
    getTrashSettings.mockResolvedValue({ retention_days: 30 });
    searchGenerations.mockResolvedValue({
      generations: [
        {
          generation: {
            id: "generation-1",
            prompt: "Restorable prompt",
            created_at: "2026-04-26T00:00:00Z",
            deleted_at: "2026-04-26T01:00:00Z",
          },
          images: [],
        },
      ],
      total: 1,
      page: 1,
      page_size: 20,
    });
  });

  it("requires confirmation before permanently deleting and prevents duplicate requests", async () => {
    permanentlyDeleteGeneration.mockReturnValue(new Promise(() => {}));
    render(<TrashPage />);
    fireEvent.click(await screen.findByText("Select generation"));
    fireEvent.click(screen.getByText("Delete image"));
    expect(permanentlyDeleteGeneration).not.toHaveBeenCalled();
    const confirm = screen.getByRole("button", { name: "trash.deleteNow" });
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    expect(permanentlyDeleteGeneration).toHaveBeenCalledTimes(1);
    expect(confirm).toBeDisabled();
  });

  it("ignores a stale search that finishes after the latest result", async () => {
    let resolveOld!: (value: unknown) => void;
    searchGenerations.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveOld = resolve;
      }),
    );
    render(<TrashPage />);
    fireEvent.change(screen.getByPlaceholderText("trash.search"), {
      target: { value: "latest" },
    });
    fireEvent.keyDown(screen.getByPlaceholderText("trash.search"), {
      key: "Enter",
    });
    await screen.findByText("Select generation");
    await act(async () =>
      resolveOld({ generations: [], total: 0, page: 1, page_size: 20 }),
    );
    expect(screen.getByText("Select generation")).toBeInTheDocument();
  });

  it("refreshes the latest applied search after a pending restore", async () => {
    let resolveRestore!: () => void;
    restoreGeneration.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolveRestore = resolve;
      }),
    );
    render(<TrashPage />);
    fireEvent.click(await screen.findByText("Select generation"));
    fireEvent.click(screen.getByText("Restore image"));
    const search = screen.getByPlaceholderText("trash.search");
    fireEvent.change(search, { target: { value: "latest" } });
    fireEvent.keyDown(search, { key: "Enter" });
    await waitFor(() =>
      expect(searchGenerations).toHaveBeenLastCalledWith("latest", 1, true),
    );
    fireEvent.change(search, { target: { value: "not submitted" } });
    await act(async () => resolveRestore());
    expect(searchGenerations).toHaveBeenLastCalledWith("latest", 1, true);
  });

  it("refreshes the conversation history immediately after restoring", async () => {
    render(<TrashPage />);

    fireEvent.click(await screen.findByText("Select generation"));
    fireEvent.click(screen.getByText("Restore image"));

    await waitFor(() => {
      expect(restoreGeneration).toHaveBeenCalledWith("generation-1");
      expect(refreshConversations).toHaveBeenCalledTimes(1);
    });
  });
});
