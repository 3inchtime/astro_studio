import { useCallback, useEffect, useMemo, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { useNavigate } from "react-router-dom";
import { deleteGeneration, searchGenerations } from "../lib/api";
import { savePendingEditSources } from "../lib/editSources";
import { useLayoutContext } from "../components/layout/AppLayout";
import type { GenerationResult, GenerationSearchFilters, MessageImage } from "../types";
import { useTranslation } from "react-i18next";
import FolderSelector from "../components/favorites/FolderSelector";
import EmptyCollectionState from "../components/gallery/EmptyCollectionState";
import GenerationDetailPanel from "../components/gallery/GenerationDetailPanel";
import GenerationGrid from "../components/gallery/GenerationGrid";
import GallerySearchBar from "../components/gallery/GallerySearchBar";
import Lightbox from "../components/lightbox/Lightbox";
import { generationResultToLightboxImages } from "../lib/lightboxImages";
import { createGallerySearchConfig } from "../lib/galleryFilterConfig";
import { useInfiniteScroll } from "../hooks/useInfiniteScroll";
import {
  compactFilters,
  isFilterActive,
  updateFilterValue,
} from "../lib/galleryFilters";

export default function GalleryPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { setActiveConversationId } = useLayoutContext();
  const [results, setResults] = useState<GenerationResult[]>([]);
  const [query, setQuery] = useState("");
  const [filters, setFilters] = useState<GenerationSearchFilters>({});
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [isLoading, setIsLoading] = useState(false);
  const [selected, setSelected] = useState<GenerationResult | null>(null);
  const [folderSelectorImageId, setFolderSelectorImageId] = useState<
    string | null
  >(null);
  const [lightboxState, setLightboxState] = useState<{
    images: MessageImage[];
    index: number;
  } | null>(null);

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
    () => createGallerySearchConfig(t, filters, handleFilterChange),
    [filters, handleFilterChange, t],
  );

  const performSearch = useCallback(
    async (
      pageToLoad: number,
      nextQuery: string,
      nextFilters: GenerationSearchFilters,
      mode: "replace" | "append" = "replace",
    ) => {
      setIsLoading(true);
      const result = await searchGenerations(
        nextQuery.trim() || undefined,
        pageToLoad,
        false,
        compactFilters(nextFilters),
        undefined,
      );
      setResults((current) =>
        mode === "append"
          ? [...current, ...result.generations]
          : result.generations,
      );
      setTotal(result.total);
      setPage(result.page);
      setPageSize(result.page_size);
      if (mode === "replace") {
        setSelected(null);
      }
      setIsLoading(false);
    },
    [],
  );

  useEffect(() => {
    void performSearch(1, "", {});
  }, [performSearch]);

  async function handleSearch() {
    await performSearch(1, query, filters);
  }

  async function handleDelete(id: string) {
    await deleteGeneration(id);
    await performSearch(page, query, filters);
    if (selected?.generation.id === id) setSelected(null);
    setLightboxState((current) => {
      if (!current) return null;
      return current.images.some((image) => image.generationId === id)
        ? null
        : current;
    });
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

  function resetFilters() {
    setQuery("");
    setFilters({});
    void performSearch(1, "", {});
  }

  const openLightbox = useCallback(
    (result: GenerationResult, index: number) => {
      setLightboxState({
        images: generationResultToLightboxImages(result),
        index,
      });
    },
    [],
  );

  const handleEditLightboxImage = useCallback(
    (image: MessageImage) => {
      handleEditImage(image.path, image.imageId, image.generationId);
      setLightboxState(null);
    },
    [],
  );

  const hasMore = page * pageSize < total;
  const loadMoreRef = useInfiniteScroll({
    enabled: results.length > 0,
    hasMore,
    isLoading,
    onLoadMore: () => {
      void performSearch(page + 1, query, filters, "append");
    },
  });

  return (
    <div className="flex h-full">
      <div className="flex flex-1 flex-col">
        <GallerySearchBar
          config={searchConfig}
          total={total}
          query={query}
          hasActiveFilters={hasActiveFilters}
          onQueryChange={setQuery}
          onSearch={() => void handleSearch()}
          onReset={resetFilters}
        />

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
              onPreview={openLightbox}
              onManageFolders={setFolderSelectorImageId}
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
            onDelete={(id) => void handleDelete(id)}
            onEditImage={handleEditImage}
            onPreview={(imageIndex) => openLightbox(selected, imageIndex)}
            onManageFolders={setFolderSelectorImageId}
          />
        )}
      </AnimatePresence>

      <AnimatePresence>
        {lightboxState && (
          <Lightbox
            images={lightboxState.images}
            initialIndex={lightboxState.index}
            onClose={() => setLightboxState(null)}
            onEditImage={handleEditLightboxImage}
            onDelete={(id) => void handleDelete(id)}
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
