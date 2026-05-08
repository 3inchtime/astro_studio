import { IMAGE_MODEL_CATALOG } from "./modelCatalog";
import type { GenerationSearchFilters } from "../types";

type TranslationFn = (key: string) => string;

export interface GalleryFilterOption<
  TValue extends string = string,
> {
  value: TValue;
  label: string;
}

type SelectFilterKey = "model";

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
  searchLabel: string;
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
    searchLabel: t("gallery.prompt"),
    searchPlaceholder: t("gallery.search"),
    applyFilters: t("gallery.applyFilters"),
    resetFilters: t("gallery.resetFilters"),
    fields: [
      selectField(
        "model",
        t("gallery.filterModel"),
        filters.model ?? "",
        [
          { value: "", label: t("gallery.allModels") },
          ...IMAGE_MODEL_CATALOG.map((entry) => ({
            value: entry.id,
            label: t(entry.i18nKey),
          })),
        ],
        (value) => onFilterChange("model", value),
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
