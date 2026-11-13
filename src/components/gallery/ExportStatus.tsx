import { useTranslation } from "react-i18next";
import { useExportStore } from "../../lib/exportStore";
export default function ExportStatus() {
  const { t } = useTranslation();
  const state = useExportStore();
  if (!state.busy && !state.result && !state.error) return null;
  const button =
    "studio-control focus-ring min-h-9 rounded-lg px-3 py-2 text-xs font-medium";
  return (
    <aside
      aria-label={t("export.status", { defaultValue: "Image export" })}
      className="studio-floating-panel fixed bottom-4 right-4 z-[80] max-h-[50vh] w-[min(360px,calc(100vw-2rem))] overflow-y-auto rounded-xl p-4"
    >
      <p role="status" className="text-sm font-medium text-foreground">
        {state.busy
          ? state.total === 0
            ? t("export.preparing", { defaultValue: "Preparing export…" })
            : t("export.progress", {
                completed: state.completed,
                total: state.total,
                defaultValue: "Exporting {{completed}} / {{total}} images…",
              })
          : t("export.completed", {
              completed: state.completed,
              total: state.total,
              defaultValue: "Exported {{completed}} / {{total}} images",
            })}
      </p>
      {state.result?.cancelled && (
        <p className="mt-2 text-xs text-muted">
          {t("export.cancelled", {
            defaultValue: "Export stopped. Completed files are kept.",
          })}
        </p>
      )}
      {state.error && (
        <p role="alert" className="mt-2 text-xs text-error">
          {t(`export.${state.error}`, {
            defaultValue: "Export could not finish. Completed files are kept.",
          })}
        </p>
      )}
      {!!state.result?.failed.length && (
        <details className="mt-3 text-xs text-muted">
          <summary className="focus-ring cursor-pointer">
            {t("export.failed", {
              count: state.result.failed.length,
              defaultValue: "{{count}} image(s) need attention",
            })}
          </summary>
          <ul className="mt-2 space-y-2">
            {state.result.failed.map((failure) => (
              <li key={failure.image_id} className="break-all">
                {failure.image_id} ·{" "}
                {t(`export.codes.${failure.code}`, {
                  defaultValue: "Could not export this image",
                })}
              </li>
            ))}
          </ul>
        </details>
      )}
      {!!state.result?.cancelled_image_ids.length && (
        <details className="mt-3 text-xs text-muted">
          <summary className="focus-ring cursor-pointer">
            {t("export.cancelledItems", {
              count: state.result.cancelled_image_ids.length,
              defaultValue: "{{count}} image(s) cancelled",
            })}
          </summary>
          <ul className="mt-2 space-y-2">
            {state.result.cancelled_image_ids.map((id) => (
              <li key={id} className="break-all">
                {id}
              </li>
            ))}
          </ul>
        </details>
      )}
      <div className="mt-3 flex flex-wrap justify-end gap-2">
        {state.busy ? (
          <button className={button} onClick={() => void state.cancel()}>
            {t("export.stop", { defaultValue: "Stop export" })}
          </button>
        ) : (
          <>
            {state.result?.retry_token && (
              <button className={button} onClick={() => void state.retry()}>
                {t("export.retry", { defaultValue: "Retry remaining images" })}
              </button>
            )}
            <button className={button} onClick={state.dismiss}>
              {t("export.dismiss", { defaultValue: "Dismiss" })}
            </button>
          </>
        )}
      </div>
    </aside>
  );
}
