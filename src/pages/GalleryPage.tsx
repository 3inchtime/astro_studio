import { useEffect, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { useNavigate } from "react-router-dom";
import { searchGenerations, deleteGeneration } from "../lib/api";
import { savePendingEditSources } from "../lib/editSources";
import { useLayoutContext } from "../components/layout/AppLayout";
import type { GenerationResult } from "../types";
import { Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import FolderSelector from "../components/favorites/FolderSelector";
import EmptyCollectionState from "../components/gallery/EmptyCollectionState";
import GenerationDetailPanel from "../components/gallery/GenerationDetailPanel";
import GenerationGrid from "../components/gallery/GenerationGrid";
import PaginationControls from "../components/gallery/PaginationControls";

export default function GalleryPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { setActiveConversationId } = useLayoutContext();
  const [results, setResults] = useState<GenerationResult[]>([]);
  const [query, setQuery] = useState("");
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [selected, setSelected] = useState<GenerationResult | null>(null);
  const [folderSelectorImageId, setFolderSelectorImageId] = useState<
    string | null
  >(null);

  async function loadGenerations(p: number, q?: string) {
    const result = await searchGenerations(q || query || undefined, p);
    setResults(result.generations);
    setTotal(result.total);
    setPage(result.page);
    setPageSize(result.page_size);
  }

  useEffect(() => {
    loadGenerations(1);
  }, []);

  function handleSearch() {
    loadGenerations(1, query);
  }

  async function handleDelete(id: string) {
    await deleteGeneration(id);
    loadGenerations(page, query);
    if (selected?.generation.id === id) setSelected(null);
  }

  function handleEditImage(
    imagePath: string,
    imageId: string,
    generationId: string,
  ) {
    const normalizedPath = imagePath.replace(/\\/g, "/");
    const fileName = normalizedPath.split("/").pop() || "source-image";

    savePendingEditSources([
      {
        id: `${imageId}:${normalizedPath}`,
        path: imagePath,
        label: fileName,
        imageId,
        generationId,
      },
    ]);
    setActiveConversationId(null);
    navigate("/generate");
  }

  const totalPages = Math.ceil(total / pageSize);

  return (
    <div className="flex h-full">
      <div className="flex flex-1 flex-col">
        <div className="flex items-center justify-between border-b border-border-subtle px-6 py-4">
          <div className="flex items-center gap-3">
            <h2 className="text-[15px] font-semibold text-foreground tracking-tight">
              {t("gallery.title")}
            </h2>
            {total > 0 && (
              <span className="rounded-[6px] bg-subtle px-2 py-0.5 text-[10px] font-medium text-muted tabular-nums">
                {total}
              </span>
            )}
          </div>

          <div className="relative">
            <Search
              size={13}
              className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted/60"
              strokeWidth={2}
            />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleSearch()}
              placeholder={t("gallery.search")}
              className="h-[30px] w-52 rounded-[8px] border border-border-subtle bg-subtle/40 pl-7 pr-3 text-[12px] text-foreground placeholder:text-muted/50 focus:outline-none focus:border-border focus:bg-surface transition-colors"
            />
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-5">
          {results.length === 0 ? (
            <EmptyCollectionState
              title={t("gallery.noImages")}
              subtitle={t("gallery.emptyHint")}
            />
          ) : (
            <GenerationGrid
              results={results}
              favoriteMode="manage"
              onSelect={setSelected}
              onManageFolders={setFolderSelectorImageId}
            />
          )}

          <PaginationControls
            page={page}
            totalPages={totalPages}
            onPageChange={(p) => loadGenerations(p)}
          />
        </div>
      </div>

      <AnimatePresence>
        {selected && (
          <GenerationDetailPanel
            result={selected}
            title={t("gallery.detail")}
            showSaveButton
            onClose={() => setSelected(null)}
            onDelete={(id) => void handleDelete(id)}
            onEditImage={handleEditImage}
            onManageFolders={setFolderSelectorImageId}
          />
        )}
      </AnimatePresence>

      {folderSelectorImageId && (
        <FolderSelector
          imageId={folderSelectorImageId}
          onClose={() => setFolderSelectorImageId(null)}
        />
      )}
    </div>
  );
}
