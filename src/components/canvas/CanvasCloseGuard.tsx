import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { useDialogFocus } from "../../hooks/useDialogFocus";
import { flushCanvasSaves } from "../../lib/canvas/persistence";

type CloseRequest = { request_id: string; action: "close" | "exit" | "restart" };

// Lives above routing: saves queued by a page that just unmounted still block exit.
export default function CanvasCloseGuard() {
  const { t } = useTranslation();
  const pending = useRef<CloseRequest | null>(null);
  const working = useRef(false);
  const [request, setRequest] = useState<CloseRequest | null>(null);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const [guardError, setGuardError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const dialogRef = useDialogFocus({ open: Boolean(request), busy, onClose: () => void cancel() });

  async function saveAndClose(next: CloseRequest, retry = false) {
    if (working.current) return;
    pending.current = next;
    working.current = true;
    setRequest(next);
    setBusy(true);
    setFailed(false);
    try {
      await flushCanvasSaves(retry);
      await invoke("finish_canvas_close", { requestId: next.request_id });
      pending.current = null;
      setRequest(null);
    } catch {
      setFailed(true);
    } finally {
      working.current = false;
      setBusy(false);
    }
  }

  async function cancel() {
    if (working.current || !pending.current) return;
    try {
      await invoke("cancel_canvas_close", { requestId: pending.current.request_id });
      pending.current = null;
      setRequest(null);
    } catch { setFailed(true); }
  }

  useEffect(() => {
    if (!(window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void (async () => {
      const cleanup = await listen<CloseRequest>("canvas:close-requested", ({ payload }) => {
        if (!disposed) void saveAndClose(payload);
      });
      if (disposed) { cleanup(); return; }
      unlisten = cleanup;
      await invoke("enable_canvas_close_guard");
      if (!disposed) setGuardError(false);
    })().catch(() => { if (!disposed) setGuardError(true); });
    return () => { disposed = true; unlisten?.(); };
  }, [attempt]);

  return <>
    {guardError && <div role="alert" className="fixed bottom-4 right-4 z-[100] max-w-sm rounded-xl border border-border bg-surface p-4 text-body-sm text-foreground">
      <p>{t("canvas.closeGuardError")}</p>
      <button type="button" className="focus-ring mt-3 rounded-lg border border-control-border px-3 py-2" onClick={() => setAttempt((value) => value + 1)}>{t("canvas.retrySave")}</button>
    </div>}
    {request && <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/40 p-6">
      <div ref={dialogRef} role="dialog" aria-modal="true" aria-labelledby="canvas-close-title" aria-describedby="canvas-close-description" aria-busy={busy} tabIndex={-1} className="w-full max-w-md rounded-xl bg-surface p-6 text-foreground shadow-float">
        <h2 id="canvas-close-title" className="text-title font-semibold">{t("canvas.closeTitle")}</h2>
        <p id="canvas-close-description" role={failed ? "alert" : "status"} className="mt-3 text-body leading-6 text-muted">{t(failed ? "canvas.closeSaveError" : "canvas.closeSaving")}</p>
        {failed && <div className="mt-6 flex flex-wrap justify-end gap-3">
          <button type="button" disabled={busy} className="focus-ring rounded-lg border border-control-border px-4 py-2 text-body-sm" onClick={() => void cancel()}>{t("canvas.keepEditing")}</button>
          <button type="button" disabled={busy} className="focus-ring rounded-lg bg-primary px-4 py-2 text-body-sm text-primary-foreground" onClick={() => void saveAndClose(request, true)}>{t("canvas.retrySaveAndClose")}</button>
        </div>}
      </div>
    </div>}
  </>;
}
