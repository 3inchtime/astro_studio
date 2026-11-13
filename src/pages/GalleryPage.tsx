import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { useNavigate } from "react-router-dom";
import { searchGenerations } from "../lib/api";
import { buildEditSource, savePendingEditSources } from "../lib/editSources";
import { useUIStore, type PreviewSource } from "../lib/store";
import { useLayoutContext } from "../components/layout/AppLayout";
import type { GenerationResult, GenerationSearchFilters } from "../types";
import { useTranslation } from "react-i18next";
import FolderSelector from "../components/favorites/FolderSelector";
import CollectionStatus from "../components/gallery/CollectionStatus";
import EmptyCollectionState from "../components/gallery/EmptyCollectionState";
import GenerationDetailPanel from "../components/gallery/GenerationDetailPanel";
import CollectionBatchActions from "../components/gallery/CollectionBatchActions";
import GenerationDeleteDialog from "../components/gallery/GenerationDeleteDialog";
import { useCollectionSelection } from "../components/gallery/useCollectionSelection";
import GenerationGrid from "../components/gallery/GenerationGrid";
import GallerySearchBar from "../components/gallery/GallerySearchBar";
import ViewModeToggle from "../components/gallery/ViewModeToggle";
import Lightbox from "../components/lightbox/Lightbox";
import { generationResultToLightboxImages } from "../lib/lightboxImages";
import { createGallerySearchConfig } from "../lib/galleryFilterConfig";
import { useInfiniteScroll } from "../hooks/useInfiniteScroll";
import { useStoredGalleryViewMode } from "../hooks/useStoredGalleryViewMode";
import {
  compactFilters,
  isFilterActive,
  updateFilterValue,
} from "../lib/galleryFilters";
import { ROUTES } from "../app/routes/contracts";

export default function GalleryPage() {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const { setActiveConversationId } = useLayoutContext();
  const {
    lightbox,
    openLightbox,
    closeLightbox,
    folderSelectorImageId,
    openFolderSelector,
    closeFolderSelector,
  } = useUIStore();
  const [results, setResults] = useState<GenerationResult[]>([]);
  const selection = useCollectionSelection(results.map((row) => row.generation.id));
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [filters, setFilters] = useState<GenerationSearchFilters>({});
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState(false);
  const [selected, setSelected] = useState<GenerationResult | null>(null);
  const searchRequestIdRef = useRef(0);
  const appliedSearch = useRef<{ query: string; filters: GenerationSearchFilters }>({ query: "", filters: {} });
  const [viewMode, setViewMode] = useStoredGalleryViewMode(
    "astro-gallery-view-mode",
  );

  const hasActiveFilters = useMemo(
    () => isFilterActive(filters, query),
    [filters, query],
  );

  const handleFilterChange = useCallback(
    <K extends keyof GenerationSearchFilters>(
      key: K,
      value: GenerationSearchFilters[K],
    ) => {
      setFilters((current) => updateFilterValue(current, key, value));
    },
    [],
  );

  const searchConfig = useMemo(
    () =>
      createGallerySearchConfig(
        t,
        filters,
        handleFilterChange,
        i18n.resolvedLanguage ?? i18n.language,
      ),
    [filters, handleFilterChange, i18n.language, i18n.resolvedLanguage, t],
  );

  const performSearch = useCallback(
    async (
      pageToLoad: number,
      nextQuery: string,
      nextFilters: GenerationSearchFilters,
      mode: "replace" | "append" = "replace",
    ) => {
      const requestId = ++searchRequestIdRef.current;
      if (mode === "replace") appliedSearch.current = { query: nextQuery, filters: nextFilters };
      setIsLoading(true);
      setLoadError(false);
      try {
        const result = await searchGenerations(
          nextQuery.trim() || undefined,
          pageToLoad,
          false,
          compactFilters(nextFilters),
          undefined,
        );
        if (requestId !== searchRequestIdRef.current) return;
        setResults((current) =>
          mode === "append"
            ? [...current, ...result.generations.filter((row) => !current.some((entry) => entry.generation.id === row.generation.id))]
            : result.generations,
        );
        setTotal(result.total);
        setPage(result.page);
        setPageSize(result.page_size);
        if (mode === "replace") {
          setSelected(null);
        }
      } catch {
        if (requestId === searchRequestIdRef.current) setLoadError(true);
      } finally {
        if (requestId === searchRequestIdRef.current) {
          setIsLoading(false);
        }
      }
    },
    [],
  );

  useEffect(() => {
    void performSearch(1, "", {});
    return () => { searchRequestIdRef.current++; };
  }, [performSearch]);

  async function handleSearch() {
    selection.clear();
    await performSearch(1, query, filters);
  }

  function handleDeleted(id: string) {
    setResults((current) => current.filter((row) => row.generation.id !== id));
    setTotal((current) => Math.max(0, current - 1));
    selection.remove([id]);
    if (selected?.generation.id === id) setSelected(null);
    if (lightbox?.images.some((image) => image.generationId === id)) {
      closeLightbox();
    }
  }

  const handleEditImage = useCallback(
    (imagePath: string, imageId: string, generationId: string) => {
      savePendingEditSources([
        buildEditSource(imagePath, imageId, generationId),
      ]);
      setActiveConversationId(null);
      navigate(ROUTES.generate);
    },
    [navigate, setActiveConversationId],
  );

  function resetFilters() {
    selection.clear();
    setQuery("");
    setFilters({});
    void performSearch(1, "", {});
  }

  const openResultLightbox = useCallback(
    (result: GenerationResult, index: number, source?: PreviewSource) => {
      openLightbox(generationResultToLightboxImages(result), index, source);
    },
    [openLightbox],
  );

  const handleEditLightboxImage = useCallback(
    (image: { path: string; imageId: string; generationId: string }) => {
      handleEditImage(image.path, image.imageId, image.generationId);
      closeLightbox();
    },
    [handleEditImage, closeLightbox],
  );

  const hasMore = results.length < total;
  const loadMoreRef = useInfiniteScroll({
    enabled: results.length > 0 && !loadError && !lightbox,
    hasMore,
    isLoading,
    onLoadMore: () => {
      void performSearch(Math.min(page + 1, Math.floor(results.length / pageSize) + 1), appliedSearch.current.query, appliedSearch.current.filters, "append");
    },
  });

  return (
    <div className="flex h-full overflow-hidden bg-background">
      <div className="flex min-w-0 flex-1 flex-col">
        <GallerySearchBar
          config={searchConfig}
          total={total}
          query={query}
          hasActiveFilters={hasActiveFilters}
          onQueryChange={setQuery}
          onSearch={() => void handleSearch()}
          onReset={resetFilters}
          actions={<ViewModeToggle value={viewMode} onChange={setViewMode} />}
        />

        <CollectionBatchActions rows={results} selectedIds={selection.selectedIds} onSelectLoaded={selection.selectLoaded} onClear={selection.clear} onSucceeded={(ids, kind) => {
          selection.remove(ids);
          if (kind === "delete") ids.forEach(handleDeleted);
        }} />
        <div className="flex-1 overflow-y-auto p-5">
          <CollectionStatus loading={isLoading} error={loadError} onRetry={() => void handleSearch()} />
          {results.length === 0 ? (!isLoading && !loadError && (
            <EmptyCollectionState
              title={t("gallery.noImages")}
              subtitle={t("gallery.emptyHint")}
            />
          )) : (
            <GenerationGrid
              results={results}
              selectedIds={selection.selectedIds}
              onToggleSelection={selection.toggle}
              viewMode={viewMode}
              favoriteMode="manage"
              onSelect={setSelected}
              onPreview={openResultLightbox}
              onManageFolders={openFolderSelector}
            />
          )}
          <div ref={loadMoreRef} aria-hidden="true" className="h-1" />
        </div>
      </div>

      <AnimatePresence>
        {selected && (
          <GenerationDetailPanel
            result={selected}
            title={t("gallery.detail")}
            showSaveButton
            onClose={() => setSelected(null)}
            onDelete={setDeleteTarget}
            onEditImage={handleEditImage}
            onPreview={(imageIndex, source) => openResultLightbox(selected, imageIndex, source)}
            onManageFolders={openFolderSelector}
          />
        )}
      </AnimatePresence>

      <AnimatePresence>
        {lightbox && (
          <Lightbox
            source={lightbox.source}
            images={lightbox.images}
            initialIndex={lightbox.index}
            onClose={closeLightbox}
            onEditImage={handleEditLightboxImage}
            onDelete={setDeleteTarget}
          />
        )}
      </AnimatePresence>

      {deleteTarget && <GenerationDeleteDialog generationId={deleteTarget} onClose={() => setDeleteTarget(null)} onDeleted={handleDeleted} />}
      {folderSelectorImageId && (
        <FolderSelector
          imageId={folderSelectorImageId}
          onClose={closeFolderSelector}
        />
      )}
    </div>
  );
}
