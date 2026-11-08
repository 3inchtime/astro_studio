import { useEffect, useRef, useState } from "react";
import { Plus, X } from "lucide-react";
import { emit } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import {
  addImageToFolders,
  createFolder,
  getFolders,
  getImageFolders,
  removeImageFromFolders,
} from "../../lib/api";
import { useDialogFocus } from "../../hooks/useDialogFocus";
import type { Folder } from "../../types";

const EVENT_NAME = "favorites:changed";

export function emitFavoritesChanged() {
  void emit(EVENT_NAME, {}).catch(() => {});
}

export interface FolderDialogService {
  list: () => Promise<Folder[]>;
  selected: (itemId: string) => Promise<string[]>;
  create: (name: string) => Promise<Folder>;
  add: (itemId: string, folderIds: string[]) => Promise<void>;
  remove: (itemId: string, folderIds: string[]) => Promise<void>;
}

const imageFolders: FolderDialogService = {
  list: getFolders,
  selected: getImageFolders,
  create: createFolder,
  add: addImageToFolders,
  remove: removeImageFromFolders,
};

export default function FolderSelector({ imageId, onClose }: {
  imageId: string;
  onClose: () => void;
}) {
  return <FolderPickerDialog itemId={imageId} titleKey="favorites.addToFolder" service={imageFolders} onSaved={emitFavoritesChanged} onClose={onClose} />;
}

export function FolderPickerDialog({ itemId, titleKey, service, displayName, onSaved, onClose }: {
  itemId: string;
  titleKey: string;
  service: FolderDialogService;
  displayName?: (folder: Folder) => string;
  onSaved?: () => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const [folders, setFolders] = useState<Folder[]>([]);
  const originalIdsRef = useRef<string[]>([]);
  const [pendingIds, setPendingIds] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadAttempt, setLoadAttempt] = useState(0);
  const [operation, setOperation] = useState<"save" | "create" | null>(null);
  const [error, setError] = useState<"load" | "save" | "create" | null>(null);
  const [newFolderName, setNewFolderName] = useState("");
  const operationRef = useRef(false);
  const requestRef = useRef(0);
  const busy = operation !== null;
  const dialogRef = useDialogFocus({ open: true, onClose, busy });
  const editingDisabled = loading || error === "load" || busy;

  useEffect(() => {
    const request = ++requestRef.current;
    setLoading(true);
    setError(null);
    setOperation(null);
    operationRef.current = false;
    Promise.all([service.list(), service.selected(itemId)])
      .then(([availableFolders, selectedIds]) => {
        if (request !== requestRef.current) return;
        setFolders(availableFolders);
        originalIdsRef.current = selectedIds;
        setPendingIds(selectedIds);
      })
      .catch(() => {
        if (request === requestRef.current) setError("load");
      })
      .finally(() => {
        if (request === requestRef.current) setLoading(false);
      });
    return () => { requestRef.current += 1; };
  }, [itemId, service, loadAttempt]);

  async function handleConfirm() {
    if (editingDisabled || operationRef.current) return;
    operationRef.current = true;
    setOperation("save");
    setError(null);
    const request = requestRef.current;
    const original = new Set(originalIdsRef.current);
    const next = new Set(pendingIds);
    const toAdd = [...next].filter((id) => !original.has(id));
    const toRemove = [...original].filter((id) => !next.has(id));
    try {
      if (toAdd.length) {
        await service.add(itemId, toAdd);
        if (request !== requestRef.current) return;
        // Keep confirmed writes so a retry only submits unfinished changes.
        originalIdsRef.current = [...new Set([...originalIdsRef.current, ...toAdd])];
      }
      if (toRemove.length) {
        await service.remove(itemId, toRemove);
        if (request !== requestRef.current) return;
        originalIdsRef.current = originalIdsRef.current.filter((id) => !toRemove.includes(id));
      }
      if (request === requestRef.current) {
        onSaved?.();
        onClose();
      }
    } catch {
      if (request === requestRef.current) setError("save");
    } finally {
      if (request === requestRef.current) {
        operationRef.current = false;
        setOperation(null);
      }
    }
  }

  async function handleCreate() {
    const name = newFolderName.trim();
    if (!name || editingDisabled || operationRef.current) return;
    operationRef.current = true;
    setOperation("create");
    setError(null);
    const request = requestRef.current;
    try {
      const folder = await service.create(name);
      if (request !== requestRef.current) return;
      setFolders((current) => [...current.filter((entry) => entry.id !== folder.id), folder]);
      setPendingIds((current) => [...new Set([...current, folder.id])]);
      setNewFolderName("");
    } catch {
      if (request === requestRef.current) setError("create");
    } finally {
      if (request === requestRef.current) {
        operationRef.current = false;
        setOperation(null);
      }
    }
  }

  function retry() {
    if (error === "load") setLoadAttempt((current) => current + 1);
    else if (error === "create") void handleCreate();
    else void handleConfirm();
  }

  const errorMessage = error === "load"
    ? t("studio.folder.loadFailed", { defaultValue: "Could not load folders. Retry to make changes." })
    : error === "create"
      ? t("studio.folder.createFailed", { defaultValue: "Could not create folder. Your name has been kept; try again." })
      : t("studio.folder.saveFailed", { defaultValue: "Could not save folder changes. Your selection has been kept; try again." });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4" onClick={(event) => {
      if (event.target === event.currentTarget && !operationRef.current) onClose();
    }}>
      <div ref={dialogRef} role="dialog" aria-modal="true" aria-label={t(titleKey)} aria-busy={loading || busy} tabIndex={-1} className="flex max-h-[calc(100dvh-32px)] w-full max-w-[360px] flex-col overflow-hidden rounded-xl border border-border bg-surface text-sm text-foreground shadow-float focus:outline-none">
        <header className="flex shrink-0 items-center justify-between gap-3 border-b border-border-subtle px-4 py-3">
          <h2 className="font-semibold">{t(titleKey)}</h2>
          <button type="button" data-autofocus aria-label={t("studio.folder.close", { defaultValue: "Close" })} onClick={onClose} disabled={busy} className="focus-ring flex h-9 w-9 shrink-0 items-center justify-center rounded-lg text-muted transition-colors hover:bg-subtle hover:text-foreground disabled:opacity-40"><X size={16} /></button>
        </header>
        <div className="min-h-0 overflow-y-auto">
          {(loading || busy) ? <p role="status" className="px-4 py-3 text-muted">{loading ? t("studio.folder.loading", { defaultValue: "Loading folders…" }) : operation === "create" ? t("studio.folder.creating", { defaultValue: "Creating folder…" }) : t("studio.folder.saving", { defaultValue: "Saving folder changes…" })}</p> : null}
          {error ? <div role="alert" className="m-3 rounded-lg border border-error/25 bg-error/5 p-3 text-error"><p>{errorMessage}</p><button type="button" onClick={retry} disabled={busy} className="focus-ring mt-2 h-9 rounded-lg border border-error/25 px-3 font-medium hover:bg-error/10">{t("studio.folder.retry", { defaultValue: "Retry" })}</button></div> : null}
          <fieldset disabled={editingDisabled} className="min-w-0 border-0 p-0">
            {!loading && error !== "load" ? <div className="max-h-60 overflow-y-auto py-1">
              {folders.length ? folders.map((folder) => <label key={folder.id} className="flex min-h-9 cursor-pointer items-center gap-2.5 px-4 py-2 transition-colors hover:bg-subtle">
                <input type="checkbox" checked={pendingIds.includes(folder.id)} onChange={(event) => {
                  const checked = event.target.checked;
                  setPendingIds((current) => checked ? [...new Set([...current, folder.id])] : current.filter((id) => id !== folder.id));
                  setError(null);
                }} className="focus-ring h-4 w-4 shrink-0 accent-primary" />
                <span className="min-w-0 break-words">{displayName ? displayName(folder) : folder.name}</span>
              </label>) : <p className="px-4 py-4 text-muted">{t("studio.folder.empty", { defaultValue: "No folders yet. Create one below." })}</p>}
            </div> : null}
            <div className="flex items-center gap-2 border-t border-border-subtle px-4 py-3">
              <input aria-label={t("studio.folder.name", { defaultValue: "Folder name" })} value={newFolderName} onChange={(event) => setNewFolderName(event.target.value)} onKeyDown={(event) => {
                if (event.key === "Enter") { event.preventDefault(); void handleCreate(); }
              }} placeholder={t("favorites.newFolder")} className="studio-input focus-ring h-9 min-w-0 flex-1 rounded-lg px-3 text-sm placeholder:text-muted" />
              <button type="button" aria-label={t("studio.folder.create", { defaultValue: "Create folder" })} onClick={() => void handleCreate()} disabled={!newFolderName.trim() || editingDisabled} className="studio-control-primary focus-ring flex h-9 w-9 shrink-0 items-center justify-center rounded-lg disabled:opacity-40"><Plus size={16} /></button>
            </div>
          </fieldset>
        </div>
        <footer className="flex shrink-0 gap-2 border-t border-border-subtle px-4 py-3">
          <button type="button" onClick={onClose} disabled={busy} className="focus-ring min-h-9 flex-1 rounded-lg border border-border-subtle px-3 font-medium text-muted hover:bg-subtle disabled:opacity-40">{t("favorites.cancel")}</button>
          <button type="button" onClick={() => void handleConfirm()} disabled={editingDisabled} className="studio-control-primary focus-ring min-h-9 flex-1 rounded-lg px-3 font-semibold disabled:opacity-40">{t("favorites.confirm")}</button>
        </footer>
      </div>
    </div>
  );
}
