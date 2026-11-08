import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import CollectionStatus from "../gallery/CollectionStatus";
import type { GenerationResult, GenerationSearchFilters } from "../../types";
import GallerySearchBar from "../gallery/GallerySearchBar";
import GenerationGrid from "../gallery/GenerationGrid";
import EmptyCollectionState from "../gallery/EmptyCollectionState";
import PaginationControls from "../gallery/PaginationControls";
import { createGallerySearchConfig } from "../../lib/galleryFilterConfig";
import { compactFilters, isFilterActive, updateFilterValue } from "../../lib/galleryFilters";

export default function ProjectImagePanel({
  loading = false,
  error = false,
  results,
  total,
  page,
  pageSize,
  onSearch,
  onSelect,
  onPreview,
  onManageFolders,
}: {
  loading?: boolean;
  error?: boolean;
  results: GenerationResult[];
  total: number;
  page: number;
  pageSize: number;
  onSearch: (query: string, filters: GenerationSearchFilters, page: number) => Promise<void>;
  onSelect: (result: GenerationResult) => void;
  onPreview?: (result: GenerationResult, index: number) => void;
  onManageFolders: (imageId: string) => void;
}) {
  const { t, i18n } = useTranslation();
  const [query, setQuery] = useState("");
  const [filters, setFilters] = useState<GenerationSearchFilters>({});

  const config = useMemo(
    () => ({
      ...createGallerySearchConfig(
        t,
        filters,
        (key, value) =>
          setFilters((current) => updateFilterValue(current, key, value)),
        i18n.resolvedLanguage ?? i18n.language,
      ),
      title: t("projects.imagesTitle"),
    }),
    [filters, i18n.language, i18n.resolvedLanguage, t],
  );

  const totalPages = Math.max(1, Math.ceil(total / pageSize));

  return (
    <div className="flex h-full flex-col">
        <GallerySearchBar
          config={config}
          total={total}
          query={query}
          hasActiveFilters={isFilterActive(filters, query)}
          onQueryChange={setQuery}
          onSearch={() => void onSearch(query, compactFilters(filters), 1)}
          onReset={() => {
            setQuery("");
            setFilters({});
            void onSearch("", {}, 1);
          }}
        />

      {/* Gallery Grid */}
      <div className="flex-1 overflow-y-auto p-6">
        <CollectionStatus loading={loading} error={error} onRetry={() => void onSearch(query, compactFilters(filters), page)} />
        {results.length === 0 ? (!loading && !error && (
          <EmptyCollectionState
            title={t("projects.imagesEmptyTitle")}
            subtitle={t("projects.imagesEmptyHint")}
          />
        )) : (
          <>
            <GenerationGrid
              results={results}
              favoriteMode="manage"
              onSelect={onSelect}
              onPreview={onPreview}
              onManageFolders={onManageFolders}
            />
            <div className="mt-5">
              <PaginationControls
                page={page}
                totalPages={totalPages}
                onPageChange={(nextPage) => void onSearch(query, compactFilters(filters), nextPage)}
              />
            </div>
          </>
        )}
      </div>
    </div>
  );
}
