import { invoke } from "@tauri-apps/api/core";
import { flushCanvasSaves } from "./canvas/persistence";
export interface BackupInspection {
  token: string;
  manifest: {
    version: number;
    created_at: string;
    generations: number;
    images: number;
    projects: number;
    canvases: number;
  };
}
export interface LibraryResult {
  status: "completed" | "cancelled" | "restart_required";
  path: string | null;
}
export async function createBackup(operationId: string) {
  await flushCanvasSaves();
  return invoke<LibraryResult>("create_library_backup", { operationId });
}
export function inspectBackup(operationId: string) {
  return invoke<BackupInspection | null>("inspect_library_backup", {
    operationId,
  });
}
export async function restoreBackup(token: string, operationId: string) {
  await flushCanvasSaves();
  return invoke<LibraryResult>("restore_library_backup", {
    token,
    operationId,
  });
}
export async function rollbackLibrary() {
  await flushCanvasSaves();
  return invoke<LibraryResult>("rollback_library_restore");
}
export function canRollbackLibrary() {
  return invoke<boolean>("can_rollback_library_restore");
}
export function cancelLibraryOperation(operationId: string) {
  return invoke<void>("cancel_operation", { operationId });
}
