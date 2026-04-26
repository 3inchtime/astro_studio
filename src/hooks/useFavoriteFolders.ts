import { useState, useCallback, useEffect } from "react";
import { getImageFolders, addImageToFolders, removeImageFromFolders } from "../lib/api";
import { listen } from "@tauri-apps/api/event";

export function useFavoriteFolders(imageId: string) {
  const [folderIds, setFolderIds] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const data = await getImageFolders(imageId);
      setFolderIds(data);
    } finally {
      setLoading(false);
    }
  }, [imageId]);

  useEffect(() => { load(); }, [load]);

  // Listen for global favorites changed event and reload
  useEffect(() => {
    const unlisten = listen("favorites:changed", () => {
      load();
    });
    return () => { unlisten.then((fn) => fn()); };
  }, [load]);

  const toggle = useCallback(async (folderId: string, selected: boolean) => {
    if (selected) {
      await addImageToFolders(imageId, [folderId]);
      setFolderIds((prev) => [...prev, folderId]);
    } else {
      await removeImageFromFolders(imageId, [folderId]);
      setFolderIds((prev) => prev.filter((id) => id !== folderId));
    }
  }, [imageId]);

  const setFolders = useCallback(async (ids: string[]) => {
    const current = new Set(folderIds);
    const next = new Set(ids);
    const toAdd = [...next].filter((id) => !current.has(id));
    const toRemove = [...current].filter((id) => !next.has(id));
    if (toAdd.length > 0) await addImageToFolders(imageId, toAdd);
    if (toRemove.length > 0) await removeImageFromFolders(imageId, toRemove);
    setFolderIds(ids);
  }, [imageId, folderIds]);

  return { folderIds, loading, toggle, setFolders, reload: load };
}