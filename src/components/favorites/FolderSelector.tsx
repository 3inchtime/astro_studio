import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { X, Plus } from "lucide-react";
import { useFolders } from "../../hooks/useFolders";
import { getImageFolders, addImageToFolders, removeImageFromFolders } from "../../lib/api";
import { cn } from "../../lib/utils";
import { emit } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";

interface FolderSelectorProps {
  imageId: string;
  onClose: () => void;
}

const EVENT_NAME = "favorites:changed";

export function emitFavoritesChanged() {
  emit(EVENT_NAME, {});
}

export default function FolderSelector({ imageId, onClose }: FolderSelectorProps) {
  const { t } = useTranslation();
  const { folders, create } = useFolders();
  const [originalIds, setOriginalIds] = useState<string[]>([]);
  const [pendingIds, setPendingIds] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [newFolderName, setNewFolderName] = useState("");
  const [creating, setCreating] = useState(false);

  // Load current folderIds once on mount
  useEffect(() => {
    getImageFolders(imageId).then((ids) => {
      setOriginalIds(ids);
      setPendingIds(ids);
      setLoading(false);
    }).catch(() => {
      setLoading(false);
    });
  }, [imageId]);

  const handleToggle = (folderId: string, checked: boolean) => {
    setPendingIds((prev) =>
      checked ? [...prev, folderId] : prev.filter((id) => id !== folderId)
    );
  };

  const handleConfirm = async () => {
    const orig = new Set(originalIds);
    const next = new Set(pendingIds);
    const toAdd = [...next].filter((id) => !orig.has(id));
    const toRemove = [...orig].filter((id) => !next.has(id));
    if (toAdd.length > 0) await addImageToFolders(imageId, toAdd);
    if (toRemove.length > 0) await removeImageFromFolders(imageId, toRemove);
    emitFavoritesChanged();
    onClose();
  };

  const handleCreate = async () => {
    if (!newFolderName.trim()) return;
    setCreating(true);
    try {
      const folder = await create(newFolderName.trim());
      setPendingIds((prev) => [...prev, folder.id]);
      setNewFolderName("");
    } finally {
      setCreating(false);
    }
  };

  const handleBackdropClick = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget) onClose();
  };

  if (loading) {
    return (
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm"
        onClick={handleBackdropClick}
      >
        <div className="w-72 rounded-[16px] border border-border bg-surface shadow-float p-8 flex items-center justify-center">
          <div className="h-5 w-5 rounded-full bg-subtle animate-pulse" />
        </div>
      </motion.div>
    );
  }

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm"
        onClick={handleBackdropClick}
      >
        <motion.div
          initial={{ opacity: 0, scale: 0.95, y: 4 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={{ opacity: 0, scale: 0.95, y: 4 }}
          transition={{ duration: 0.15 }}
          className="w-72 rounded-[16px] border border-border bg-surface shadow-float overflow-hidden"
        >
          <div className="flex items-center justify-between px-4 py-3 border-b border-border-subtle">
            <span className="text-[13px] font-semibold text-foreground">{t("favorites.addToFolder")}</span>
            <button
              onClick={onClose}
              className="flex h-6 w-6 items-center justify-center rounded-[6px] text-muted hover:bg-subtle hover:text-foreground transition-colors"
            >
              <X size={14} />
            </button>
          </div>

          <div className="max-h-60 overflow-y-auto py-1">
            {folders.map((folder) => (
              <label
                key={folder.id}
                className="flex items-center gap-2.5 px-4 py-2 cursor-pointer hover:bg-subtle transition-colors"
              >
                <input
                  type="checkbox"
                  checked={pendingIds.includes(folder.id)}
                  onChange={(e) => handleToggle(folder.id, e.target.checked)}
                  className={cn(
                    "h-4 w-4 rounded-[4px] border border-border-subtle",
                    "bg-transparent",
                    "checked:bg-primary checked:border-primary",
                    "focus:outline-none focus:ring-2 focus:ring-primary/20",
                    "transition-colors"
                  )}
                />
                <span className="text-[13px] text-foreground">{folder.name}</span>
              </label>
            ))}
          </div>

          <div className="flex items-center gap-2 px-4 py-3 border-t border-border-subtle">
            <input
              value={newFolderName}
              onChange={(e) => setNewFolderName(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") handleCreate(); }}
              placeholder={t("favorites.newFolder")}
              className="flex-1 rounded-[8px] border border-border-subtle bg-subtle px-3 py-1.5 text-[12px] text-foreground placeholder:text-muted/50 focus:outline-none focus:border-border transition-colors"
            />
            <button
              onClick={handleCreate}
              disabled={!newFolderName.trim() || creating}
              className="flex h-7 w-7 items-center justify-center rounded-[8px] gradient-primary text-white disabled:opacity-40 transition-opacity"
            >
              <Plus size={14} />
            </button>
          </div>

          <div className="flex gap-2 px-4 pb-4">
            <button
              onClick={onClose}
              className="flex-1 rounded-[10px] border border-border-subtle py-2 text-[13px] font-medium text-muted hover:bg-subtle hover:text-foreground transition-colors"
            >
              {t("favorites.cancel")}
            </button>
            <button
              onClick={handleConfirm}
              className="flex-1 rounded-[10px] gradient-primary py-2 text-[13px] font-semibold text-white shadow-[0_4px_12px_rgba(79,106,255,0.3)] hover:shadow-[0_6px_16px_rgba(79,106,255,0.4)] transition-shadow"
            >
              {t("favorites.confirm")}
            </button>
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}
