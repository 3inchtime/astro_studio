import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import BackupOperationStatus from "./BackupOperationStatus";
import { runBackupInspection, useBackupState } from "./LibraryBackupPanel";
import { cancelLibraryOperation, inspectBackup } from "../../lib/libraryApi";
vi.mock("../../lib/libraryApi", () => ({
  cancelLibraryOperation: vi.fn().mockResolvedValue(undefined),
  createBackup: vi.fn(),
  inspectBackup: vi.fn(),
  restoreBackup: vi.fn(),
  rollbackLibrary: vi.fn(),
  canRollbackLibrary: vi.fn(),
}));
afterEach(() => {
  cleanup();
  useBackupState.setState({ busy: false, id: null, error: null, status: null });
});
it("keeps cancellation available after the settings panel unmounts", async () => {
  useBackupState.setState({
    busy: true,
    id: "operation",
    completed: 2,
    total: 4,
  });
  render(<BackupOperationStatus />);
  expect(screen.getByRole("status")).toHaveTextContent("2 / 4");
  fireEvent.click(screen.getByRole("button", { name: /cancel/i }));
  await waitFor(() =>
    expect(cancelLibraryOperation).toHaveBeenCalledWith("operation"),
  );
});

it("inspection cancellation survives navigation without opening confirmation", async () => {
  let reject!: (reason: string) => void;
  vi.mocked(inspectBackup).mockReturnValue(
    new Promise((_, r) => {
      reject = r;
    }),
  );
  let work!: Promise<void>;
  act(() => {
    work = runBackupInspection();
  });
  expect(useBackupState.getState().busy).toBe(true);
  render(<BackupOperationStatus />);
  fireEvent.click(screen.getByRole("button", { name: /cancel/i }));
  await act(async () => {
    reject("operation_cancelled");
    await work;
  });
  expect(useBackupState.getState().inspection).toBeNull();
  expect(useBackupState.getState().error).toBeNull();
  expect(useBackupState.getState().status).toBe("cancelled");
});
