import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import TrashPage from "./TrashPage";

const restoreGeneration = vi.fn();
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
  permanentlyDeleteGeneration: vi.fn(),
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
  }: {
    onRestore: (generationId: string) => void;
  }) => (
    <button onClick={() => onRestore("generation-1")}>Restore image</button>
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

  it("refreshes the conversation history immediately after restoring", async () => {
    render(<TrashPage />);

    await waitFor(() => {
      expect(searchGenerations).toHaveBeenCalledWith(undefined, 1, true);
    });

    fireEvent.click(screen.getByText("Select generation"));
    fireEvent.click(screen.getByText("Restore image"));

    await waitFor(() => {
      expect(restoreGeneration).toHaveBeenCalledWith("generation-1");
      expect(refreshConversations).toHaveBeenCalledTimes(1);
    });
  });
});
