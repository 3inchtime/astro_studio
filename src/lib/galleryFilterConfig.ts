import { IMAGE_MODEL_CATALOG } from "./modelCatalog";
import type { GenerationSearchFilters } from "../types";

type TranslationFn = (key: string) => string;

export interface GalleryFilterOption<
  TValue extends string = string,
> {
  value: TValue;
  label: string;
}

type SelectFilterKey = Exclude<
  keyof GenerationSearchFilters,
  "created_from" | "created_to"
>;

export interface GallerySelectFilterConfig<
  TKey extends SelectFilterKey = SelectFilterKey,
> {
  type: "select";
  key: TKey;
  label: string;
  value: string;
  options: GalleryFilterOption<
    NonNullable<GenerationSearchFilters[TKey]> & string
  >[];
  onChange: (value: string) => void;
}

export interface GalleryDateFilterConfig {
  type: "date";
  key: "created_from" | "created_to";
  label: string;
  value: string;
  onChange: (value: string) => void;
}

export type GalleryFilterFieldConfig =
  | GallerySelectFilterConfig
  | GalleryDateFilterConfig;

export interface GallerySearchConfig {
  title: string;
  searchPlaceholder: string;
  applyFilters: string;
  resetFilters: string;
  fields: GalleryFilterFieldConfig[];
}

function selectField<TKey extends SelectFilterKey>(
  key: TKey,
  label: string,
  value: string,
  options: GalleryFilterOption<
    NonNullable<GenerationSearchFilters[TKey]> & string
  >[],
  onChange: (value: GenerationSearchFilters[TKey]) => void,
): GallerySelectFilterConfig<TKey> {
  return {
    type: "select",
    key,
    label,
    value,
    options,
    onChange: (nextValue) => onChange(nextValue as GenerationSearchFilters[TKey]),
  };
}

function dateField(
  key: "created_from" | "created_to",
  label: string,
  value: string,
  onChange: (value: string) => void,
): GalleryDateFilterConfig {
  return {
    type: "date",
    key,
    label,
    value,
    onChange,
  };
}

export function createGallerySearchConfig(
  t: TranslationFn,
  filters: GenerationSearchFilters,
  onFilterChange: <K extends keyof GenerationSearchFilters>(
    key: K,
    value: GenerationSearchFilters[K],
  ) => void,
): GallerySearchConfig {
  return {
    title: t("gallery.title"),
    searchPlaceholder: t("gallery.search"),
    applyFilters: t("gallery.applyFilters"),
    resetFilters: t("gallery.resetFilters"),
    fields: [
      selectField(
        "model",
        t("gallery.filterModel"),
        filters.model ?? "",
        [
          { value: "", label: "All models" },
          ...IMAGE_MODEL_CATALOG.map((entry) => ({
            value: entry.id,
            label: entry.label,
          })),
        ],
        (value) => onFilterChange("model", value),
      ),
      selectField(
        "request_kind",
        t("gallery.filterRequestKind"),
        filters.request_kind ?? "",
        [
          { value: "", label: "All request types" },
          { value: "generate", label: "Generate" },
          { value: "edit", label: "Edit" },
        ],
        (value) => onFilterChange("request_kind", value),
      ),
      selectField(
        "status",
        t("gallery.filterStatus"),
        filters.status ?? "",
        [
          { value: "", label: "All statuses" },
          { value: "processing", label: "Processing" },
          { value: "completed", label: "Completed" },
          { value: "failed", label: "Failed" },
        ],
        (value) => onFilterChange("status", value),
      ),
      selectField(
        "size",
        t("gallery.filterSize"),
        filters.size ?? "",
        [
          { value: "", label: "All sizes" },
          { value: "auto", label: "Auto" },
          { value: "1024x1024", label: "1:1" },
          { value: "1536x1024", label: "3:2" },
          { value: "1024x1536", label: "2:3" },
        ],
        (value) => onFilterChange("size", value),
      ),
      selectField(
        "quality",
        t("gallery.filterQuality"),
        filters.quality ?? "",
        [
          { value: "", label: "All qualities" },
          { value: "auto", label: "Auto" },
          { value: "high", label: "High" },
          { value: "medium", label: "Medium" },
          { value: "low", label: "Low" },
        ],
        (value) => onFilterChange("quality", value),
      ),
      selectField(
        "background",
        t("gallery.filterBackground"),
        filters.background ?? "",
        [
          { value: "", label: "All backgrounds" },
          { value: "auto", label: "Auto" },
          { value: "opaque", label: "Opaque" },
          { value: "transparent", label: "Transparent" },
        ],
        (value) => onFilterChange("background", value),
      ),
      selectField(
        "output_format",
        t("gallery.filterFormat"),
        filters.output_format ?? "",
        [
          { value: "", label: "All formats" },
          { value: "png", label: "PNG" },
          { value: "jpeg", label: "JPEG" },
          { value: "webp", label: "WEBP" },
        ],
        (value) => onFilterChange("output_format", value),
      ),
      selectField(
        "moderation",
        t("gallery.filterModeration"),
        filters.moderation ?? "",
        [
          { value: "", label: "All moderation" },
          { value: "auto", label: "Auto" },
          { value: "low", label: "Low" },
        ],
        (value) => onFilterChange("moderation", value),
      ),
      selectField(
        "input_fidelity",
        t("gallery.filterFidelity"),
        filters.input_fidelity ?? "",
        [
          { value: "", label: "All fidelity" },
          { value: "high", label: "High" },
          { value: "low", label: "Low" },
        ],
        (value) => onFilterChange("input_fidelity", value),
      ),
      selectField(
        "source_image_count",
        t("gallery.filterSources"),
        filters.source_image_count ?? "any",
        [
          { value: "any", label: "Any sources" },
          { value: "0", label: "No sources" },
          { value: "1", label: "1 source" },
          { value: "2", label: "2 sources" },
          { value: "3", label: "3 sources" },
          { value: "4+", label: "4+ sources" },
        ],
        (value) => onFilterChange("source_image_count", value),
      ),
      dateField(
        "created_from",
        t("gallery.filterCreatedFrom"),
        filters.created_from ?? "",
        (value) => onFilterChange("created_from", value),
      ),
      dateField(
        "created_to",
        t("gallery.filterCreatedTo"),
        filters.created_to ?? "",
        (value) => onFilterChange("created_to", value),
      ),
    ],
  };
}
