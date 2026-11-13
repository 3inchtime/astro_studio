import { useCollectionSelection } from "../gallery/useCollectionSelection";
import ExportSelectionAction from "../gallery/ExportSelectionAction";
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
  const selectable = results.filter(row => row.images.length > 0);
  const selection = useCollectionSelection(selectable.map(row => row.generation.id));
  const [selectionMode, setSelectionMode] = useState(false);
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
          headingLevel="section"
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

      <div className="flex min-h-12 flex-wrap items-center gap-2 border-b border-border-subtle px-6 py-2">
        <button type="button" aria-pressed={selectionMode} onClick={() => { setSelectionMode(!selectionMode); if (selectionMode) selection.clear(); }} className="studio-control focus-ring min-h-9 rounded-lg px-3 text-xs">{t("export.select", { defaultValue: "Select images" })}</button>
        {(selectionMode || selection.selectedIds.size > 0) && <><button type="button" onClick={selection.selectLoaded} className="studio-control focus-ring min-h-9 rounded-lg px-3 text-xs">{t("studio.collection.selectVisible", { defaultValue: "Select all visible" })}</button><span className="text-xs text-muted">{t("studio.collection.selectedCount", { count: selection.selectedIds.size, defaultValue: "{{count}} generation record(s) selected" })}</span></>}
        {selection.selectedIds.size > 0 && <ExportSelectionAction rows={selectable} selectedIds={selection.selectedIds} />}
      </div>
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
              selectedIds={selection.selectedIds}
              selectionMode={selectionMode || selection.selectedIds.size > 0}
              onToggleSelection={selection.toggle}
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
