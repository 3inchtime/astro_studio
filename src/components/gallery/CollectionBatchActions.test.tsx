import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import CollectionBatchActions from "./CollectionBatchActions";
import type { GenerationResult } from "../../types";
import { deleteGeneration, permanentlyDeleteGeneration, restoreGeneration } from "../../lib/api";
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string, options?: { defaultValue?: string; count?: number }) => options?.defaultValue?.replace("{{count}}", String(options.count)) ?? key }) }));
vi.mock("../../lib/api", () => ({ permanentlyDeleteGeneration: vi.fn(), restoreGeneration: vi.fn(), deleteGeneration: vi.fn(), addImageToFolders: vi.fn(), getFolders: vi.fn().mockResolvedValue([]) }));
const rows = ["a", "b"].map((id) => ({ generation: { id }, images: [{ id: `${id}1` }, { id: `${id}2` }] })) as GenerationResult[];
describe("CollectionBatchActions", () => {
  it("keeps bulk controls absent until selection begins", () => {
    render(<CollectionBatchActions rows={rows} selectedIds={new Set()} onSelectLoaded={vi.fn()} onClear={vi.fn()} onSucceeded={vi.fn()} />);
    expect(screen.queryByText(/generation record.*selected/)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Select all visible" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Select all loaded" })).not.toBeInTheDocument();
  });

  it.each([["Restore selected", "restore", restoreGeneration], ["Permanently delete selected", "purge", permanentlyDeleteGeneration]] as const)(
    "confirms %s for the selected trash records", async (label, action, command) => {
      const completed = vi.fn();
      render(<CollectionBatchActions scope="trash" rows={rows} selectedIds={new Set(["a"])} onSelectLoaded={vi.fn()} onClear={vi.fn()} onSucceeded={completed} />);
      expect(screen.queryByRole("button", { name: "Move to trash" })).toBeNull();
      fireEvent.click(screen.getByRole("button", { name: label }));
      expect(command).not.toHaveBeenCalled();
      fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
      await waitFor(() => expect(completed).toHaveBeenCalledWith(["a"], action));
      expect(command).toHaveBeenCalledExactlyOnceWith("a");
    }
  );

  it("confirms deleting all images and retains only failed generations for retry", async () => {
    vi.mocked(deleteGeneration).mockResolvedValueOnce(undefined).mockRejectedValueOnce(new Error("locked")).mockResolvedValueOnce(undefined);
    const removed = vi.fn();
    const { rerender } = render(<CollectionBatchActions rows={rows} selectedIds={new Set(["a", "b"])} onSelectLoaded={vi.fn()} onClear={vi.fn()} onSucceeded={removed} />);
    fireEvent.click(screen.getByRole("button", { name: "Move to trash" }));
    expect(screen.getByText("All images in these generation records will move to trash.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));
    await waitFor(() => expect(removed).toHaveBeenCalledWith(["a"], "delete"));
    expect(screen.getByRole("alert")).toHaveTextContent("1 generation record(s) failed");
    rerender(<CollectionBatchActions rows={[rows[1]]} selectedIds={new Set(["b"])} onSelectLoaded={vi.fn()} onClear={vi.fn()} onSucceeded={removed} />);
    fireEvent.click(screen.getByRole("button", { name: "Retry failed" }));
    await waitFor(() => expect(removed).toHaveBeenLastCalledWith(["b"], "delete"));
    expect(vi.mocked(deleteGeneration).mock.calls).toEqual([["a"], ["b"], ["b"]]);
  });
});
