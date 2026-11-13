import { useTranslation } from "react-i18next";
import { cancelLibraryOperation } from "../../lib/libraryApi";
import { useBackupState } from "./LibraryBackupPanel";

/** The running operation belongs to the app, not the settings route. */
export default function BackupOperationStatus() {
  const { t } = useTranslation();
  const state = useBackupState();
  if (!state.busy || !state.id) return null;
  return (
    <aside
      aria-label={t("backup.title")}
      className="studio-floating-panel fixed bottom-4 left-[max(6rem,calc(env(safe-area-inset-left)+1rem))] z-[80] w-[min(340px,calc(100vw-7rem))] rounded-xl p-4"
    >
      <p role="status" className="text-sm font-medium text-foreground">
        {t("backup.working")}
        {state.total > 0 && (
          <span className="ml-2 tabular-nums">
            {state.completed} / {state.total}
          </span>
        )}
      </p>
      {state.error && (
        <p role="alert" className="mt-2 break-words text-xs text-error">
          {state.error}
        </p>
      )}
      <div className="mt-3 flex justify-end">
        <button
          type="button"
          className="studio-control focus-ring min-h-9 rounded-lg px-3 py-2 text-xs font-medium"
          onClick={() =>
            void cancelLibraryOperation(state.id!).catch(() =>
              useBackupState.setState({
                error: t("backup.cancelFailed", {
                  defaultValue:
                    "Could not cancel yet. Check progress and try again.",
                }),
              }),
            )
          }
        >
          {t("backup.cancel")}
        </button>
      </div>
    </aside>
  );
}
