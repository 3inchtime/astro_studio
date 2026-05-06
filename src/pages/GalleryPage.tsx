import { useCallback, useEffect, useMemo, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { useNavigate } from "react-router-dom";
import { deleteGeneration, searchGenerations } from "../lib/api";
import { savePendingEditSources } from "../lib/editSources";
import { useLayoutContext } from "../components/layout/AppLayout";
import type {
  GenerationResult,
  GenerationSearchFilters,
  ImageBackground,
  ImageInputFidelity,
  ImageModeration,
  ImageModel,
  ImageOutputFormat,
  ImageQuality,
  ImageSize,
} from "../types";
import { Filter, RotateCcw, Search } from "lucide-react";
import { useTranslation } from "react-i18next";
import { IMAGE_MODEL_CATALOG } from "../lib/modelCatalog";
import FolderSelector from "../components/favorites/FolderSelector";
import EmptyCollectionState from "../components/gallery/EmptyCollectionState";
import GenerationDetailPanel from "../components/gallery/GenerationDetailPanel";
import GenerationGrid from "../components/gallery/GenerationGrid";
import PaginationControls from "../components/gallery/PaginationControls";

const SIZE_OPTIONS: Array<{ value: ImageSize | ""; label: string }> = [
  { value: "", label: "All sizes" },
  { value: "auto", label: "Auto" },
  { value: "1024x1024", label: "1:1" },
  { value: "1536x1024", label: "3:2" },
  { value: "1024x1536", label: "2:3" },
];

const QUALITY_OPTIONS: Array<{ value: ImageQuality | ""; label: string }> = [
  { value: "", label: "All qualities" },
  { value: "auto", label: "Auto" },
  { value: "high", label: "High" },
  { value: "medium", label: "Medium" },
  { value: "low", label: "Low" },
];

const BACKGROUND_OPTIONS: Array<{ value: ImageBackground | ""; label: string }> = [
  { value: "", label: "All backgrounds" },
  { value: "auto", label: "Auto" },
  { value: "opaque", label: "Opaque" },
  { value: "transparent", label: "Transparent" },
];

const FORMAT_OPTIONS: Array<{ value: ImageOutputFormat | ""; label: string }> = [
  { value: "", label: "All formats" },
  { value: "png", label: "PNG" },
  { value: "jpeg", label: "JPEG" },
  { value: "webp", label: "WEBP" },
];

const MODERATION_OPTIONS: Array<{ value: ImageModeration | ""; label: string }> = [
  { value: "", label: "All moderation" },
  { value: "auto", label: "Auto" },
  { value: "low", label: "Low" },
];

const INPUT_FIDELITY_OPTIONS: Array<{
  value: ImageInputFidelity | "";
  label: string;
}> = [
  { value: "", label: "All fidelity" },
  { value: "high", label: "High" },
  { value: "low", label: "Low" },
];

const SOURCE_IMAGE_COUNT_OPTIONS: Array<{
  value: NonNullable<GenerationSearchFilters["source_image_count"]>;
  label: string;
}> = [
  { value: "any", label: "Any sources" },
  { value: "0", label: "No sources" },
  { value: "1", label: "1 source" },
  { value: "2", label: "2 sources" },
  { value: "3", label: "3 sources" },
  { value: "4+", label: "4+ sources" },
];

const REQUEST_KIND_OPTIONS: Array<{
  value: NonNullable<GenerationSearchFilters["request_kind"]> | "";
  label: string;
}> = [
  { value: "", label: "All request types" },
  { value: "generate", label: "Generate" },
  { value: "edit", label: "Edit" },
];

const STATUS_OPTIONS: Array<{
  value: NonNullable<GenerationSearchFilters["status"]> | "";
  label: string;
}> = [
  { value: "", label: "All statuses" },
  { value: "processing", label: "Processing" },
  { value: "completed", label: "Completed" },
  { value: "failed", label: "Failed" },
];

type FilterOption = { value: string; label: string };

function compactFilters(
  filters: GenerationSearchFilters,
): GenerationSearchFilters {
  return Object.fromEntries(
    Object.entries(filters).filter(([, value]) => {
      if (value === "" || value === undefined) return false;
      if (value === "any") return false;
      return true;
    }),
  ) as GenerationSearchFilters;
}

function isFilterActive(filters: GenerationSearchFilters, query: string): boolean {
  const normalized = compactFilters(filters);
  return (
    query.trim().length > 0 ||
    Object.values(normalized).some((value) => value !== undefined && value !== "")
  );
}

function FilterSelect({
  label,
  value,
  onChange,
  options,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: FilterOption[];
}) {
  return (
    <label className="flex min-w-0 flex-col gap-1">
      <span className="text-[10px] font-medium uppercase tracking-[0.08em] text-muted/60">
        {label}
      </span>
      <select
        value={value}
        onChange={(event) => onChange(event.target.value)}
        className="select-control h-[34px] min-w-0 rounded-[10px] border border-border-subtle bg-subtle/35 px-3 text-[12px] text-foreground outline-none transition-colors focus:border-border focus:bg-surface"
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function FilterDateInput({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string | undefined;
  onChange: (value: string) => void;
}) {
  return (
    <label className="flex min-w-0 flex-col gap-1">
      <span className="text-[10px] font-medium uppercase tracking-[0.08em] text-muted/60">
        {label}
      </span>
      <input
        type="date"
        value={value ?? ""}
        onChange={(event) => onChange(event.target.value)}
        className="h-[34px] rounded-[10px] border border-border-subtle bg-subtle/35 px-3 text-[12px] text-foreground outline-none transition-colors focus:border-border focus:bg-surface"
      />
    </label>
  );
}

function updateFilterValue<K extends keyof GenerationSearchFilters>(
  current: GenerationSearchFilters,
  key: K,
  value: GenerationSearchFilters[K],
): GenerationSearchFilters {
  return { ...current, [key]: value };
}

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
  const [selected, setSelected] = useState<GenerationResult | null>(null);
  const [folderSelectorImageId, setFolderSelectorImageId] = useState<
    string | null
  >(null);

  const modelOptions = useMemo<FilterOption[]>(
    () => [
      { value: "", label: "All models" },
      ...IMAGE_MODEL_CATALOG.map((entry) => ({
        value: entry.id,
        label: entry.label,
      })),
    ],
    [],
  );

  const hasActiveFilters = useMemo(
    () => isFilterActive(filters, query),
    [filters, query],
  );

  const performSearch = useCallback(
    async (
      pageToLoad: number,
      nextQuery: string,
      nextFilters: GenerationSearchFilters,
    ) => {
      const result = await searchGenerations(
        nextQuery.trim() || undefined,
        pageToLoad,
        false,
        compactFilters(nextFilters),
      );
      setResults(result.generations);
      setTotal(result.total);
      setPage(result.page);
      setPageSize(result.page_size);
      setSelected(null);
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

  const totalPages = Math.max(1, Math.ceil(total / pageSize));

  return (
    <div className="flex h-full">
      <div className="flex flex-1 flex-col">
        <div className="border-b border-border-subtle px-6 py-4">
          <div className="flex flex-col gap-3 xl:flex-row xl:items-center xl:justify-between">
            <div className="flex items-center gap-3">
              <h2 className="text-[15px] font-semibold tracking-tight text-foreground">
                {t("gallery.title")}
              </h2>
              {total > 0 && (
                <span className="rounded-[6px] bg-subtle px-2 py-0.5 text-[10px] font-medium tabular-nums text-muted">
                  {total}
                </span>
              )}
            </div>

            <div className="flex w-full min-w-0 flex-col gap-2 sm:flex-row sm:items-center xl:w-auto">
              <label className="relative min-w-0 flex-1 xl:w-80 xl:flex-none">
                <Search
                  size={13}
                  className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-muted/60"
                  strokeWidth={2}
                />
                <input
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      void handleSearch();
                    }
                  }}
                  placeholder={t("gallery.search")}
                  className="h-[34px] w-full rounded-[10px] border border-border-subtle bg-subtle/40 pl-7 pr-3 text-[12px] text-foreground placeholder:text-muted/50 transition-colors focus:border-border focus:bg-surface focus:outline-none"
                />
              </label>
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  onClick={() => void handleSearch()}
                  className="inline-flex h-[34px] items-center justify-center gap-1.5 rounded-[10px] gradient-primary px-4 text-[12px] font-medium text-white shadow-button transition-transform hover:-translate-y-0.5"
                >
                  <Filter size={13} />
                  {t("gallery.applyFilters")}
                </button>
                <button
                  type="button"
                  onClick={resetFilters}
                  disabled={!hasActiveFilters}
                  className="inline-flex h-[34px] items-center justify-center gap-1.5 rounded-[10px] border border-border-subtle px-4 text-[12px] font-medium text-foreground/75 transition-colors hover:border-border hover:bg-subtle hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40"
                >
                  <RotateCcw size={13} />
                  {t("gallery.resetFilters")}
                </button>
              </div>
            </div>
          </div>

          <div className="mt-4 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
            <FilterSelect
              label={t("gallery.filterModel")}
              value={filters.model ?? ""}
              onChange={(value) =>
                setFilters((current) => updateFilterValue(current, "model", value as ImageModel | ""))
              }
              options={modelOptions}
            />
            <FilterSelect
              label={t("gallery.filterRequestKind")}
              value={filters.request_kind ?? ""}
              onChange={(value) =>
                setFilters((current) =>
                  updateFilterValue(
                    current,
                    "request_kind",
                    value as GenerationSearchFilters["request_kind"],
                  ),
                )
              }
              options={REQUEST_KIND_OPTIONS}
            />
            <FilterSelect
              label={t("gallery.filterStatus")}
              value={filters.status ?? ""}
              onChange={(value) =>
                setFilters((current) =>
                  updateFilterValue(
                    current,
                    "status",
                    value as GenerationSearchFilters["status"],
                  ),
                )
              }
              options={STATUS_OPTIONS}
            />
            <FilterSelect
              label={t("gallery.filterSize")}
              value={filters.size ?? ""}
              onChange={(value) =>
                setFilters((current) =>
                  updateFilterValue(current, "size", value as ImageSize | ""),
                )
              }
              options={SIZE_OPTIONS}
            />
            <FilterSelect
              label={t("gallery.filterQuality")}
              value={filters.quality ?? ""}
              onChange={(value) =>
                setFilters((current) =>
                  updateFilterValue(current, "quality", value as ImageQuality | ""),
                )
              }
              options={QUALITY_OPTIONS}
            />
            <FilterSelect
              label={t("gallery.filterBackground")}
              value={filters.background ?? ""}
              onChange={(value) =>
                setFilters((current) =>
                  updateFilterValue(
                    current,
                    "background",
                    value as ImageBackground | "",
                  ),
                )
              }
              options={BACKGROUND_OPTIONS}
            />
            <FilterSelect
              label={t("gallery.filterFormat")}
              value={filters.output_format ?? ""}
              onChange={(value) =>
                setFilters((current) =>
                  updateFilterValue(
                    current,
                    "output_format",
                    value as ImageOutputFormat | "",
                  ),
                )
              }
              options={FORMAT_OPTIONS}
            />
            <FilterSelect
              label={t("gallery.filterModeration")}
              value={filters.moderation ?? ""}
              onChange={(value) =>
                setFilters((current) =>
                  updateFilterValue(
                    current,
                    "moderation",
                    value as ImageModeration | "",
                  ),
                )
              }
              options={MODERATION_OPTIONS}
            />
            <FilterSelect
              label={t("gallery.filterFidelity")}
              value={filters.input_fidelity ?? ""}
              onChange={(value) =>
                setFilters((current) =>
                  updateFilterValue(
                    current,
                    "input_fidelity",
                    value as ImageInputFidelity | "",
                  ),
                )
              }
              options={INPUT_FIDELITY_OPTIONS}
            />
            <FilterSelect
              label={t("gallery.filterSources")}
              value={filters.source_image_count ?? "any"}
              onChange={(value) =>
                setFilters((current) =>
                  updateFilterValue(
                    current,
                    "source_image_count",
                    value as GenerationSearchFilters["source_image_count"],
                  ),
                )
              }
              options={SOURCE_IMAGE_COUNT_OPTIONS}
            />
            <FilterDateInput
              label={t("gallery.filterCreatedFrom")}
              value={filters.created_from}
              onChange={(value) =>
                setFilters((current) =>
                  updateFilterValue(current, "created_from", value),
                )
              }
            />
            <FilterDateInput
              label={t("gallery.filterCreatedTo")}
              value={filters.created_to}
              onChange={(value) =>
                setFilters((current) =>
                  updateFilterValue(current, "created_to", value),
                )
              }
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
            onPageChange={(nextPage) => {
              void performSearch(nextPage, query, filters);
            }}
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
