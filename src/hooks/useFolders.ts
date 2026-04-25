import { useState, useCallback, useEffect } from "react";
import { getFolders, createFolder, renameFolder, deleteFolder } from "../lib/api";
import type { Folder } from "../types";

export function useFolders() {
  const [folders, setFolders] = useState<Folder[]>([]);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const data = await getFolders();
      setFolders(data);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { load(); }, [load]);

  const create = useCallback(async (name: string) => {
    const folder = await createFolder(name);
    setFolders((prev) => [...prev, folder]);
    return folder;
  }, []);

  const rename = useCallback(async (id: string, name: string) => {
    await renameFolder(id, name);
    setFolders((prev) => prev.map((f) => f.id === id ? { ...f, name } : f));
  }, []);

  const remove = useCallback(async (id: string) => {
    await deleteFolder(id);
    setFolders((prev) => prev.filter((f) => f.id !== id));
  }, []);

  return { folders, loading, create, rename, remove, reload: load };
}