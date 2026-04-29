import { useEffect, useState } from "react";
import type { MouseEvent } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Plus, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { usePromptFolders } from "../../hooks/usePromptFolders";
import {
  addPromptFavoriteToFolders,
  getPromptFavoriteFolders,
  removePromptFavoriteFromFolders,
} from "../../lib/api";
import { getPromptFolderDisplayName } from "../../lib/promptFolders";
import { cn } from "../../lib/utils";

interface PromptFolderSelectorProps {
  favoriteId: string;
  onClose: () => void;
}

export default function PromptFolderSelector({
  favoriteId,
  onClose,
}: PromptFolderSelectorProps) {
  const { t } = useTranslation();
  const { folders, create } = usePromptFolders();
  const [originalIds, setOriginalIds] = useState<string[]>([]);
  const [pendingIds, setPendingIds] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [newFolderName, setNewFolderName] = useState("");
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    getPromptFavoriteFolders(favoriteId)
      .then((ids) => {
        setOriginalIds(ids);
        setPendingIds(ids);
      })
      .finally(() => {
        setLoading(false);
      });
  }, [favoriteId]);

  const handleToggle = (folderId: string, checked: boolean) => {
    setPendingIds((current) =>
      checked
        ? [...current, folderId]
        : current.filter((id) => id !== folderId),
    );
  };

  const handleConfirm = async () => {
    const original = new Set(originalIds);
    const next = new Set(pendingIds);
    const toAdd = [...next].filter((id) => !original.has(id));
    const toRemove = [...original].filter((id) => !next.has(id));

    if (toAdd.length > 0) {
      await addPromptFavoriteToFolders(favoriteId, toAdd);
    }
    if (toRemove.length > 0) {
      await removePromptFavoriteFromFolders(favoriteId, toRemove);
    }
    onClose();
  };

  const handleCreate = async () => {
    const name = newFolderName.trim();
    if (!name) return;

    setCreating(true);
    try {
      const folder = await create(name);
      setPendingIds((current) => [...current, folder.id]);
      setNewFolderName("");
    } finally {
      setCreating(false);
    }
  };

  const handleBackdropClick = (event: MouseEvent<HTMLDivElement>) => {
    if (event.target === event.currentTarget) {
      onClose();
    }
  };

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
          className="w-72 overflow-hidden rounded-[16px] border border-border bg-surface shadow-float"
        >
          <div className="flex items-center justify-between border-b border-border-subtle px-4 py-3">
            <span className="text-[13px] font-semibold text-foreground">
              {t("favorites.addPromptToFolder")}
            </span>
            <button
              onClick={onClose}
              className="flex h-6 w-6 items-center justify-center rounded-[6px] text-muted transition-colors hover:bg-subtle hover:text-foreground"
              aria-label={t("favorites.cancel")}
            >
              <X size={14} />
            </button>
          </div>

          {loading ? (
            <div className="flex justify-center p-8">
              <div className="h-5 w-5 rounded-full bg-subtle animate-pulse" />
            </div>
          ) : (
            <div className="max-h-60 overflow-y-auto py-1">
              {folders.map((folder) => (
                <label
                  key={folder.id}
                  className="flex cursor-pointer items-center gap-2.5 px-4 py-2 transition-colors hover:bg-subtle"
                >
                  <input
                    type="checkbox"
                    checked={pendingIds.includes(folder.id)}
                    onChange={(event) =>
                      handleToggle(folder.id, event.target.checked)
                    }
                    className={cn(
                      "h-4 w-4 rounded-[4px] border border-border-subtle",
                      "bg-transparent checked:border-primary checked:bg-primary",
                      "transition-colors focus:outline-none focus:ring-2 focus:ring-primary/20",
                    )}
                  />
                  <span className="min-w-0 truncate text-[13px] text-foreground">
                    {getPromptFolderDisplayName(folder)}
                  </span>
                </label>
              ))}
            </div>
          )}

          <div className="flex items-center gap-2 border-t border-border-subtle px-4 py-3">
            <input
              value={newFolderName}
              onChange={(event) => setNewFolderName(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") void handleCreate();
              }}
              placeholder={t("favorites.newFolder")}
              className="min-w-0 flex-1 rounded-[8px] border border-border-subtle bg-subtle px-3 py-1.5 text-[12px] text-foreground transition-colors placeholder:text-muted/50 focus:border-border focus:outline-none"
            />
            <button
              onClick={() => void handleCreate()}
              disabled={!newFolderName.trim() || creating}
              className="flex h-7 w-7 items-center justify-center rounded-[8px] gradient-primary text-white transition-opacity disabled:opacity-40"
              aria-label={t("favorites.newFolder")}
            >
              <Plus size={14} />
            </button>
          </div>

          <div className="flex gap-2 px-4 pb-4">
            <button
              onClick={onClose}
              className="flex-1 rounded-[10px] border border-border-subtle py-2 text-[13px] font-medium text-muted transition-colors hover:bg-subtle hover:text-foreground"
            >
              {t("favorites.cancel")}
            </button>
            <button
              onClick={() => void handleConfirm()}
              className="flex-1 rounded-[10px] gradient-primary py-2 text-[13px] font-semibold text-white shadow-[0_4px_12px_rgba(79,106,255,0.3)] transition-shadow hover:shadow-[0_6px_16px_rgba(79,106,255,0.4)]"
            >
              {t("favorites.confirm")}
            </button>
          </div>
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}
