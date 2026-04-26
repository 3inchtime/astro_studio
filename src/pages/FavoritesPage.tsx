import { useEffect, useState, useCallback } from "react";
import { AnimatePresence } from "framer-motion";
import { deleteGeneration, getFavoriteImages } from "../lib/api";
import type { GenerationResult } from "../types";
import { Folder, Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useFolders } from "../hooks/useFolders";
import FolderSelector from "../components/favorites/FolderSelector";
import EmptyCollectionState from "../components/gallery/EmptyCollectionState";
import GenerationDetailPanel from "../components/gallery/GenerationDetailPanel";
import GenerationGrid from "../components/gallery/GenerationGrid";
import PaginationControls from "../components/gallery/PaginationControls";

export default function FavoritesPage() {
  const { t } = useTranslation();
  const { folders, reload: reloadFolders } = useFolders();
  const [results, setResults] = useState<GenerationResult[]>([]);
  const [query, setQuery] = useState("");
  const [selectedFolderId, setSelectedFolderId] = useState("");
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize] = useState(20);
  const [selected, setSelected] = useState<GenerationResult | null>(null);
  const [folderSelectorImageId, setFolderSelectorImageId] = useState<string | null>(null);

  const loadFavorites = useCallback(async (p: number, q?: string, folderId?: string) => {
    const result = await getFavoriteImages(
      folderId || selectedFolderId || undefined,
      q?.trim() || query.trim() || undefined,
      p,
    );
    setResults(result.generations);
    setTotal(result.total);
    setPage(result.page);
  }, [query, selectedFolderId]);

  useEffect(() => {
    loadFavorites(1);
  }, []);

  function handleSearch() {
    loadFavorites(1, query, selectedFolderId);
  }

  function handleFolderFilterChange(folderId: string) {
    setSelectedFolderId(folderId);
    setSelected(null);
    loadFavorites(1, query, folderId);
  }

  async function handleDelete(id: string) {
    await deleteGeneration(id);
    loadFavorites(page, query, selectedFolderId);
    if (selected?.generation.id === id) setSelected(null);
  }

  function handleFolderSelectorClose() {
    setFolderSelectorImageId(null);
    reloadFolders();
    loadFavorites(page, query, selectedFolderId);
  }

  const totalPages = Math.ceil(total / pageSize);

  return (
    <div className="flex h-full">
      <div className="flex flex-1 flex-col">
        <div className="flex flex-col gap-3 border-b border-border-subtle px-6 py-4 lg:flex-row lg:items-center lg:justify-between">
          <div className="flex items-center gap-3">
            <h2 className="text-[15px] font-semibold text-foreground tracking-tight">
              {t("favorites.title")}
            </h2>
            {total > 0 && (
              <span className="rounded-[6px] bg-subtle px-2 py-0.5 text-[10px] font-medium text-muted tabular-nums">
                {total}
              </span>
            )}
          </div>

          <div className="flex w-full min-w-0 items-center gap-2 lg:w-auto">
            <div className="relative min-w-0 flex-1 lg:flex-none">
              <Folder size={13} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted/60" strokeWidth={2} />
              <select
                value={selectedFolderId}
                onChange={(e) => handleFolderFilterChange(e.target.value)}
                className="h-[30px] w-full appearance-none rounded-[8px] border border-border-subtle bg-subtle/40 pl-7 pr-7 text-[12px] text-foreground transition-colors focus:border-border focus:bg-surface focus:outline-none lg:w-40"
                title={t("favorites.folderFilter")}
                aria-label={t("favorites.folderFilter")}
              >
                <option value="">{t("favorites.allFolders")}</option>
                {folders.map((folder) => (
                  <option key={folder.id} value={folder.id}>
                    {folder.name}
                  </option>
                ))}
              </select>
            </div>

            <div className="relative min-w-0 flex-1 lg:flex-none">
              <Search size={13} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted/60" strokeWidth={2} />
              <input
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleSearch()}
                placeholder={t("favorites.search")}
                className="h-[30px] w-full rounded-[8px] border border-border-subtle bg-subtle/40 pl-7 pr-3 text-[12px] text-foreground placeholder:text-muted/50 focus:outline-none focus:border-border focus:bg-surface transition-colors lg:w-52"
              />
            </div>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-5">
          {results.length === 0 ? (
            <EmptyCollectionState title={t("favorites.noImages")} subtitle={t("favorites.emptyHint")} />
          ) : (
            <GenerationGrid results={results} onSelect={setSelected} />
          )}

          <PaginationControls page={page} totalPages={totalPages} onPageChange={(p) => loadFavorites(p, query, selectedFolderId)} />
        </div>
      </div>

      <AnimatePresence>
        {selected && (
          <GenerationDetailPanel
            result={selected}
            title={t("favorites.detail")}
            onClose={() => setSelected(null)}
            onDelete={(id) => void handleDelete(id)}
            onManageFolders={setFolderSelectorImageId}
          />
        )}
      </AnimatePresence>

      {folderSelectorImageId && (
        <FolderSelector imageId={folderSelectorImageId} onClose={handleFolderSelectorClose} />
      )}
    </div>
  );
}
