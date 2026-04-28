import { useState, useCallback, useEffect } from "react";
import {
  createPromptFolder,
  deletePromptFolder,
  getPromptFolders,
  renamePromptFolder,
} from "../lib/api";
import type { Folder } from "../types";

export function usePromptFolders() {
  const [folders, setFolders] = useState<Folder[]>([]);
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const data = await getPromptFolders();
      setFolders(data);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const create = useCallback(async (name: string) => {
    const folder = await createPromptFolder(name);
    setFolders((prev) => [...prev, folder]);
    return folder;
  }, []);

  const rename = useCallback(async (id: string, name: string) => {
    await renamePromptFolder(id, name);
    setFolders((prev) =>
      prev.map((folder) => (folder.id === id ? { ...folder, name } : folder)),
    );
  }, []);

  const remove = useCallback(async (id: string) => {
    await deletePromptFolder(id);
    setFolders((prev) => prev.filter((folder) => folder.id !== id));
  }, []);

  return { folders, loading, create, rename, remove, reload: load };
}
