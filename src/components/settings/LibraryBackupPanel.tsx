import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import {
  createBackup,
  inspectBackup,
  restoreBackup,
  rollbackLibrary,
  canRollbackLibrary,
  cancelLibraryOperation,
  type BackupInspection,
  type LibraryResult,
} from "../../lib/libraryApi";
import { relaunchApp } from "../../lib/api";
import ConfirmDialog from "../common/ConfirmDialog";
interface BackupState {
  inspection: BackupInspection | null;
  inspecting: boolean;
  busy: boolean;
  id: string | null;
  status: string | null;
  error: string | null;
  completed: number;
  total: number;
}
export const useBackupState = create<BackupState>(() => ({
  inspection: null,
  inspecting: false,
  busy: false,
  id: null,
  status: null,
  error: null,
  completed: 0,
  total: 0,
}));
function errorText(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string") return error;
  return JSON.stringify(error);
}
async function runBackupOperation(run: (id: string) => Promise<LibraryResult>) {
  if (useBackupState.getState().busy) return;
  const id = crypto.randomUUID();
  useBackupState.setState({
    busy: true,
    id,
    error: null,
    status: null,
    completed: 0,
    total: 0,
  });
  let unlisten: (() => void) | undefined;
  try {
    unlisten = await listen<{
      operation_id: string;
      completed: number;
      total: number;
    }>("library:operation-progress", ({ payload }) => {
      if (payload.operation_id === id)
        useBackupState.setState({
          completed: payload.completed,
          total: payload.total,
        });
    });
    const result = await run(id);
    useBackupState.setState({ status: result.status });
    if (result.status === "restart_required") await relaunchApp();
  } catch (error) {
    if (errorText(error) === "operation_cancelled")
      useBackupState.setState({ status: "cancelled" });
    else useBackupState.setState({ error: errorText(error) });
  } finally {
    unlisten?.();
    useBackupState.setState({ busy: false, id: null });
  }
}
export async function runBackupInspection() {
  if (useBackupState.getState().busy) return;
  const id = crypto.randomUUID();
  useBackupState.setState({
    busy: true,
    inspecting: true,
    id,
    error: null,
    status: null,
    inspection: null,
    completed: 0,
    total: 0,
  });
  try {
    const inspection = await inspectBackup(id);
    useBackupState.setState({ inspection });
  } catch (error) {
    if (errorText(error) === "operation_cancelled")
      useBackupState.setState({ status: "cancelled" });
    else useBackupState.setState({ error: errorText(error) });
  } finally {
    useBackupState.setState({ busy: false, inspecting: false, id: null });
  }
}
export default function LibraryBackupPanel() {
  const { t } = useTranslation();
  const state = useBackupState();
  const { inspection, inspecting } = state;
  const setInspection = (inspection: BackupInspection | null) =>
    useBackupState.setState({ inspection });
  const [rollback, setRollback] = useState(false);
  const [canRollback, setCanRollback] = useState(false);
  useEffect(() => {
    let active = true;
    void canRollbackLibrary()
      .then((value) => {
        if (active) setCanRollback(value);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  const disabled =
    state.busy || inspecting || state.status === "restart_required";
  return (
    <section
      className="border-t border-border-subtle pt-5"
      aria-labelledby="backup-title"
    >
      <h2
        id="backup-title"
        className="mb-2 text-base font-semibold text-foreground"
      >
        {t("backup.title")}
      </h2>
      <p className="mb-4 text-sm leading-relaxed text-muted">
        {t("backup.description")}
      </p>
      <div className="flex flex-wrap gap-2">
        <button
          className="studio-control focus-ring rounded-lg px-3 py-2 text-sm"
          disabled={disabled}
          onClick={() => void runBackupOperation(createBackup)}
        >
          {t("backup.create")}
        </button>
        <button
          className="studio-control focus-ring rounded-lg px-3 py-2 text-sm"
          disabled={disabled}
          onClick={() => void runBackupInspection()}
        >
          {inspecting ? t("backup.checking") : t("backup.restore")}
        </button>
        {canRollback && (
          <button
            className="studio-control focus-ring rounded-lg px-3 py-2 text-sm"
            disabled={disabled}
            onClick={() => setRollback(true)}
          >
            {t("backup.rollback")}
          </button>
        )}
        {state.id && (
          <button
            className="studio-control focus-ring rounded-lg px-3 py-2 text-sm"
            onClick={() =>
              void cancelLibraryOperation(state.id!).catch((error) =>
                useBackupState.setState({ error: errorText(error) }),
              )
            }
          >
            {t("backup.cancel")}
          </button>
        )}
      </div>
      {state.busy && (
        <div role="status" className="mt-3 text-sm text-muted">
          {t("backup.working")}
          {state.total > 0 && (
            <>
              <span className="ml-2 tabular-nums">
                {state.completed} / {state.total}
              </span>
              <progress
                className="mt-2 block w-full"
                max={state.total}
                value={state.completed}
                aria-label={t("backup.progress")}
              />
            </>
          )}
        </div>
      )}
      {state.status && (
        <p role="status" className="mt-3 text-sm text-muted">
          {t(`backup.${state.status}`)}
        </p>
      )}
      {state.error && (
        <p role="alert" className="mt-3 break-words text-sm text-error">
          {state.error}
        </p>
      )}
      {state.status === "restart_required" && (
        <button
          className="studio-control focus-ring mt-2 rounded-lg px-3 py-2 text-sm"
          onClick={() =>
            void relaunchApp().catch((error) =>
              useBackupState.setState({ error: errorText(error) }),
            )
          }
        >
          {t("backup.restart")}
        </button>
      )}
      <ConfirmDialog
        open={inspection !== null}
        title={t("backup.confirmRestore")}
        description={
          inspection
            ? `${t("backup.restoreWarning")} ${t("backup.counts", { projects: inspection.manifest.projects, images: inspection.manifest.images, canvases: inspection.manifest.canvases })}`
            : ""
        }
        confirmLabel={t("backup.confirmRestore")}
        cancelLabel={t("backup.cancel")}
        loading={state.busy}
        onCancel={() => setInspection(null)}
        onConfirm={() => {
          if (inspection) {
            const token = inspection.token;
            setInspection(null);
            void runBackupOperation((id) => restoreBackup(token, id));
          }
        }}
      />
      <ConfirmDialog
        open={rollback}
        title={t("backup.rollback")}
        description={t("backup.rollbackWarning")}
        confirmLabel={t("backup.rollback")}
        cancelLabel={t("backup.cancel")}
        onCancel={() => setRollback(false)}
        onConfirm={() => {
          setRollback(false);
          void runBackupOperation(rollbackLibrary);
        }}
      />
    </section>
  );
}
