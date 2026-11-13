import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import GenerationDeleteDialog from "./GenerationDeleteDialog";
import { deleteGeneration } from "../../lib/api";
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string, options?: { defaultValue?: string }) => options?.defaultValue ?? key }) }));
vi.mock("../../lib/api", () => ({ deleteGeneration: vi.fn() }));
it("waits for confirmation and retains the dialog on failure", async () => {
  vi.mocked(deleteGeneration).mockRejectedValueOnce(new Error("locked")).mockResolvedValueOnce(undefined);
  const deleted = vi.fn();
  render(<GenerationDeleteDialog generationId="g" onClose={vi.fn()} onDeleted={deleted} />);
  expect(deleteGeneration).not.toHaveBeenCalled();
  expect(screen.getByText("All images in this generation record will move to trash.")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Move to trash" }));
  await screen.findByRole("alert");
  expect(deleted).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Move to trash" }));
  await waitFor(() => expect(deleted).toHaveBeenCalledWith("g"));
});
