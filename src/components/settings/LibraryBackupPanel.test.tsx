import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { vi, describe, it, expect, beforeEach } from "vitest";
const mocks = vi.hoisted(() => ({
  createBackup: vi.fn(),
  inspectBackup: vi.fn(),
  restoreBackup: vi.fn(),
  rollbackLibrary: vi.fn(),
  canRollbackLibrary: vi.fn(),
  cancelLibraryOperation: vi.fn(),
}));
vi.mock("../../lib/libraryApi", () => mocks);
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock("../../lib/api", () => ({ relaunchApp: vi.fn() }));
import LibraryBackupPanel from "./LibraryBackupPanel";
describe("library backup", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.canRollbackLibrary.mockResolvedValue(false);
  });
  it("treats save dialog cancellation as a normal result", async () => {
    mocks.createBackup.mockResolvedValue({ status: "cancelled", path: null });
    render(<LibraryBackupPanel />);
    fireEvent.click(screen.getByRole("button", { name: "backup.create" }));
    await screen.findByText("backup.cancelled");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
  it("inspects a backup but does not restore until confirmed", async () => {
    mocks.inspectBackup.mockResolvedValue({
      token: "token",
      manifest: {
        version: 1,
        created_at: "2026-11-13",
        generations: 2,
        images: 3,
        projects: 1,
        canvases: 1,
      },
    });
    mocks.restoreBackup.mockResolvedValue({
      status: "restart_required",
      path: null,
    });
    render(<LibraryBackupPanel />);
    fireEvent.click(screen.getByRole("button", { name: "backup.restore" }));
    await screen.findByRole("dialog");
    expect(mocks.restoreBackup).not.toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole("button", { name: "backup.confirmRestore" }),
    );
    await waitFor(() =>
      expect(mocks.restoreBackup).toHaveBeenCalledWith(
        "token",
        expect.any(String),
      ),
    );
  });
});
