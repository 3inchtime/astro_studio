import { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { X, Plus } from "lucide-react";
import { useFolders } from "../../hooks/useFolders";
import { useFavoriteFolders } from "../../hooks/useFavoriteFolders";
import { cn } from "../../lib/utils";

interface FolderSelectorProps {
  imageId: string;
  onClose: () => void;
}

export default function FolderSelector({ imageId, onClose }: FolderSelectorProps) {
  const { folders, create } = useFolders();
  const { folderIds, setFolders } = useFavoriteFolders(imageId);
  const [newFolderName, setNewFolderName] = useState("");
  const [creating, setCreating] = useState(false);

  const handleToggle = async (folderId: string, checked: boolean) => {
    const next = checked
      ? [...folderIds, folderId]
      : folderIds.filter((id) => id !== folderId);
    await setFolders(next);
  };

  const handleCreate = async () => {
    if (!newFolderName.trim()) return;
    setCreating(true);
    try {
      const folder = await create(newFolderName.trim());
      await setFolders([...folderIds, folder.id]);
      setNewFolderName("");
    } finally {
      setCreating(false);
    }
  };

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 backdrop-blur-sm"
        onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
      >
        <motion.div
          initial={{ opacity: 0, scale: 0.95, y: 4 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={{ opacity: 0, scale: 0.95, y: 4 }}
          transition={{ duration: 0.15 }}
          className="w-72 rounded-[16px] border border-border bg-surface shadow-float overflow-hidden"
        >
          <div className="flex items-center justify-between px-4 py-3 border-b border-border-subtle">
            <span className="text-[13px] font-semibold text-foreground">加入收藏夹</span>
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
                  checked={folderIds.includes(folder.id)}
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
              placeholder="新建文件夹"
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
        </motion.div>
      </motion.div>
    </AnimatePresence>
  );
}