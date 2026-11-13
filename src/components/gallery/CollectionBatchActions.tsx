import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { addImageToFolders, deleteGeneration, getFolders } from "../../lib/api";
import { runGenerationBatch, selectLoadedGenerations } from "../../lib/collectionBatch";
import { useDialogFocus } from "../../hooks/useDialogFocus";
import { emit } from "@tauri-apps/api/event";
import type { Folder, GenerationResult } from "../../types";

type Action = "delete" | "favorite";
interface Props {
  rows: GenerationResult[];
  selectedIds: Set<string>;
  onSelectLoaded: () => void;
  onClear: () => void;
  onSucceeded: (ids: string[], kind: Action) => void;
}

export default function CollectionBatchActions({ rows, selectedIds, onSelectLoaded, onClear, onSucceeded }: Props) {
  const { t } = useTranslation();
  const [action, setAction] = useState<Action | null>(null);
  const [targets, setTargets] = useState<GenerationResult[]>([]);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [folderIds, setFolderIds] = useState<string[]>([]);
  const [loadingFolders, setLoadingFolders] = useState(false);
  const [folderError, setFolderError] = useState(false);
  const [busy, setBusy] = useState(false);
  const [failedCount, setFailedCount] = useState(0);
  const busyRef = useRef(false);
  const checkpoints = useRef(new Set<string>());
  const close = () => { if (!busyRef.current) setAction(null); };
  const dialogRef = useDialogFocus({ open: action !== null, onClose: close, busy });

  async function loadFolders() {
    setLoadingFolders(true);
    setFolderError(false);
    try { setFolders(await getFolders()); }
    catch { setFolderError(true); }
    finally { setLoadingFolders(false); }
  }
  useEffect(() => { if (action === "favorite") void loadFolders(); }, [action]);

  function begin(kind: Action) {
    checkpoints.current = new Set();
    setTargets(selectLoadedGenerations(rows, selectedIds));
    setFolderIds([]);
    setFailedCount(0);
    setAction(kind);
  }
  async function confirm() {
    if (!action || busyRef.current || (action === "favorite" && (!folderIds.length || loadingFolders || folderError))) return;
    busyRef.current = true;
    setBusy(true);
    const outcome = await runGenerationBatch(targets, action, action === "delete" ? deleteGeneration : (id) => addImageToFolders(id, folderIds), checkpoints.current);
    if (action === "favorite" && checkpoints.current.size > 0) void emit("favorites:changed", {}).catch(() => {});
    onSucceeded(outcome.succeeded, action);
    setTargets((current) => current.filter((row) => outcome.failed.includes(row.generation.id)));
    setFailedCount(outcome.failed.length);
    busyRef.current = false;
    setBusy(false);
    if (!outcome.failed.length) setAction(null);
  }

  const buttonClass = "studio-control focus-ring min-h-9 rounded-lg px-3 text-xs font-medium disabled:opacity-40";
  return <>
    <div className="flex min-h-12 shrink-0 flex-wrap items-center gap-2 border-b border-border-subtle px-5 py-2">
      <button type="button" disabled={!rows.length} onClick={onSelectLoaded} className={buttonClass}>{t("studio.collection.selectLoaded", { defaultValue: "Select all loaded" })}</button>
      <span aria-live="polite" className="text-xs text-muted">{t("studio.collection.selectedCount", { count: selectedIds.size, defaultValue: "{{count}} generation record(s) selected" })}</span>
      {selectedIds.size > 0 && <>
        <button type="button" onClick={() => begin("favorite")} className={buttonClass}>{t("studio.collection.bulkFavorite", { defaultValue: "Add to folders" })}</button>
        <button type="button" onClick={() => begin("delete")} className={`${buttonClass} text-error`}>{t("studio.collection.bulkDelete", { defaultValue: "Move to trash" })}</button>
        <button type="button" onClick={onClear} className={buttonClass}>{t("studio.collection.clearSelection", { defaultValue: "Clear selection" })}</button>
      </>}
    </div>
    {action && <div className="fixed inset-0 z-[70] flex items-center justify-center bg-black/45 p-4" onClick={(event) => { if (event.target === event.currentTarget) close(); }}>
      <div ref={dialogRef} role="dialog" aria-modal="true" aria-label={action === "delete" ? t("studio.collection.bulkDelete", { defaultValue: "Move to trash" }) : t("studio.collection.bulkFavorite", { defaultValue: "Add to folders" })} aria-busy={busy} tabIndex={-1} className="studio-floating-panel flex max-h-[calc(100dvh-32px)] w-full max-w-sm flex-col rounded-xl p-5 outline-none">
        <h2 className="text-base font-semibold text-foreground">{t("studio.collection.batchCount", { count: targets.length, defaultValue: "{{count}} generation record(s)" })}</h2>
        <p className="mt-2 text-sm text-muted">{action === "delete" ? t("studio.collection.deleteAllImages", { defaultValue: "All images in these generation records will move to trash." }) : t("studio.collection.favoriteAllImages", { defaultValue: "Add every image in these generation records to the selected folders." })}</p>
        {action === "favorite" && <div className="mt-4 min-h-0 overflow-y-auto">
          {loadingFolders && <p role="status" className="text-sm text-muted">{t("studio.folder.loading", { defaultValue: "Loading folders…" })}</p>}
          {folderError && <div role="alert"><p className="text-sm text-error">{t("studio.folder.loadFailed", { defaultValue: "Could not load folders. Retry to make changes." })}</p><button className={buttonClass} onClick={() => void loadFolders()}>{t("studio.folder.retry", { defaultValue: "Retry" })}</button></div>}
          {!loadingFolders && !folderError && folders.length === 0 && <p className="text-sm text-muted">{t("studio.collection.createFolderFirst", { defaultValue: "Create a folder in Favorites before adding images." })}</p>}
          {folders.map((folder) => <label key={folder.id} className="flex min-h-9 items-center gap-2 text-sm text-foreground"><input type="checkbox" disabled={busy || failedCount > 0} checked={folderIds.includes(folder.id)} onChange={(event) => setFolderIds((current) => event.target.checked ? [...current, folder.id] : current.filter((id) => id !== folder.id))} className="focus-ring h-4 w-4 accent-primary" />{folder.name}</label>)}
        </div>}
        {failedCount > 0 && <p role="alert" className="mt-4 text-sm text-error">{t("studio.collection.batchFailed", { count: failedCount, defaultValue: "{{count}} generation record(s) failed. Successful changes are saved; retry the remaining records." })}</p>}
        {busy && <p role="status" className="mt-4 text-sm text-muted">{t("studio.collection.batchSaving", { defaultValue: "Saving changes…" })}</p>}
        <div className="mt-5 flex shrink-0 justify-end gap-2">
          <button className={buttonClass} disabled={busy} onClick={close}>{t("favorites.cancel")}</button>
          <button className={`${buttonClass} studio-control-primary`} disabled={busy || (action === "favorite" && (!folderIds.length || loadingFolders || folderError))} onClick={() => void confirm()}>{failedCount ? t("studio.collection.retryFailed", { defaultValue: "Retry failed" }) : t("studio.collection.confirmBatch", { defaultValue: "Confirm" })}</button>
        </div>
      </div>
    </div>}
  </>;
}
