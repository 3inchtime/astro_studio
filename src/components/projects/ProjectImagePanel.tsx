import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
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
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [filters, setFilters] = useState<GenerationSearchFilters>({});

  const config = useMemo(
    () => ({
      ...createGallerySearchConfig(t, filters, (key, value) =>
        setFilters((current) => updateFilterValue(current, key, value)),
      ),
      title: t("projects.imagesTitle"),
    }),
    [filters, t],
  );

  return (
    <div className="rounded-[18px] border border-border-subtle bg-surface">
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
      <div className="p-5">
        {results.length === 0 ? (
          <EmptyCollectionState
            title={t("projects.imagesEmptyTitle")}
            subtitle={t("projects.imagesEmptyHint")}
          />
        ) : (
          <GenerationGrid results={results} favoriteMode="manage" onSelect={onSelect} onPreview={onPreview} onManageFolders={onManageFolders} />
        )}
        <PaginationControls
          page={page}
          totalPages={Math.max(1, Math.ceil(total / pageSize))}
          onPageChange={(nextPage) => void onSearch(query, compactFilters(filters), nextPage)}
        />
      </div>
    </div>
  );
}
