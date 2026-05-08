import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Image as ImageIcon } from "lucide-react";
import type { GenerationResult, GenerationSearchFilters } from "../../types";
import GallerySearchBar from "../gallery/GallerySearchBar";
import GenerationGrid from "../gallery/GenerationGrid";
import EmptyCollectionState from "../gallery/EmptyCollectionState";
import PaginationControls from "../gallery/PaginationControls";
import { createGallerySearchConfig } from "../../lib/galleryFilterConfig";
import { compactFilters, isFilterActive, updateFilterValue } from "../../lib/galleryFilters";

export default function ProjectImagePanel({
  results,
  total,
  page,
  pageSize,
  onSearch,
  onSelect,
  onPreview,
  onManageFolders,
}: {
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
      {/* Gallery Header */}
      <div className="shrink-0 px-6 pt-5 pb-3">
        <div className="flex items-center justify-between gap-4 mb-3">
          <div className="flex items-center gap-2">
            <ImageIcon size={15} className="text-muted" strokeWidth={1.8} />
            <h2 className="text-[13px] font-semibold text-foreground tracking-tight">
              {t("projects.imagesTitle")}
            </h2>
            {total > 0 && (
              <span className="text-[11px] text-muted tabular-nums">
                {total}
              </span>
            )}
          </div>
        </div>

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
      </div>

      {/* Gallery Grid */}
      <div className="flex-1 overflow-y-auto px-6 pb-6">
        {results.length === 0 ? (
          <EmptyCollectionState
            title={t("projects.imagesEmptyTitle")}
            subtitle={t("projects.imagesEmptyHint")}
          />
        ) : (
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
