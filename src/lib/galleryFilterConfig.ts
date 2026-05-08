import { IMAGE_MODEL_CATALOG } from "./modelCatalog";
import type { GenerationSearchFilters } from "../types";
import {
  formatDateRangeDisplay,
  type DateRangeFilterValue,
} from "./dateRangeFilters";
import { getDayPickerLocale } from "./dayPickerLocale";

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

export interface GalleryDateRangeFilterConfig {
  type: "date-range";
  key: "created_range";
  label: string;
  value: DateRangeFilterValue;
  displayValue: string;
  locale: ReturnType<typeof getDayPickerLocale>;
  presets: {
    today: string;
    last7Days: string;
    last30Days: string;
    thisMonth: string;
    clear: string;
    done: string;
  };
  onChange: (value: DateRangeFilterValue) => void;
}

export type GalleryFilterFieldConfig =
  | GallerySelectFilterConfig
  | GalleryDateRangeFilterConfig;

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

function dateRangeField(
  label: string,
  value: DateRangeFilterValue,
  t: TranslationFn,
  language: string | undefined,
  onChange: (value: DateRangeFilterValue) => void,
): GalleryDateRangeFilterConfig {
  const locale = getDayPickerLocale(language);

  return {
    type: "date-range",
    key: "created_range",
    label,
    value,
    displayValue: formatDateRangeDisplay(value, t("gallery.rangeAllTime"), locale.code),
    locale,
    presets: {
      today: t("gallery.rangePresetToday"),
      last7Days: t("gallery.rangePresetLast7Days"),
      last30Days: t("gallery.rangePresetLast30Days"),
      thisMonth: t("gallery.rangePresetThisMonth"),
      clear: t("gallery.rangePresetClear"),
      done: t("gallery.rangePresetDone"),
    },
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
  language?: string,
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
      dateRangeField(
        t("gallery.filterCreatedRange"),
        {
          from: filters.created_from ?? "",
          to: filters.created_to ?? "",
        },
        t,
        language,
        (value) => {
          onFilterChange("created_from", value.from);
          onFilterChange("created_to", value.to);
        },
      ),
    ],
  };
}
