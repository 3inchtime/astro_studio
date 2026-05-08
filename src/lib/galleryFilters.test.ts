import { describe, expect, it } from "vitest";
import {
  compactFilters,
  isFilterActive,
  updateFilterValue,
} from "./galleryFilters";
import { createGallerySearchConfig } from "./galleryFilterConfig";
import type { GenerationSearchFilters } from "../types";

describe("galleryFilters", () => {
  it("removes empty and undefined filter values", () => {
    const filters: GenerationSearchFilters = {
      model: "gpt-image-2",
      created_from: "2026-05-01",
      created_to: undefined,
    };

    expect(compactFilters(filters)).toEqual({
      model: "gpt-image-2",
      created_from: "2026-05-01",
    });
  });

  it("treats a trimmed query or compact filter value as active", () => {
    expect(isFilterActive({}, "   ")).toBe(false);
    expect(isFilterActive({}, "sunrise")).toBe(true);
    expect(isFilterActive({ model: "gpt-image-2" }, "")).toBe(true);
  });

  it("returns a new filters object with the updated value", () => {
    const filters: GenerationSearchFilters = {
      model: "gpt-image-2",
      created_to: "2026-05-12",
    };

    const updated = updateFilterValue(filters, "created_to", "2026-05-31");

    expect(updated).toEqual({
      model: "gpt-image-2",
      created_to: "2026-05-31",
    });
    expect(updated).not.toBe(filters);
    expect(filters.created_to).toBe("2026-05-12");
  });

  it("builds gallery search config with a single date range field", () => {
    const filters: GenerationSearchFilters = {
      model: "gpt-image-2",
      created_from: "2026-05-01",
      created_to: "2026-05-31",
    };
    const config = createGallerySearchConfig((key) => key, filters, () => undefined);

    expect(config.searchLabel).toBe("gallery.prompt");
    expect(config.fields.map((field) => field.key)).toEqual([
      "model",
      "created_range",
    ]);
  });

  it("localizes date range display and calendar locale from the current language", () => {
    const filters: GenerationSearchFilters = {
      created_from: "2026-05-01",
      created_to: "2026-05-31",
    };

    const config = createGallerySearchConfig(
      (key) => key,
      filters,
      () => undefined,
      "zh-CN",
    );
    const dateField = config.fields.find((field) => field.key === "created_range");

    expect(dateField?.type).toBe("date-range");
    if (dateField?.type !== "date-range") return;

    expect(dateField.locale.code).toBe("zh-CN");
    expect(dateField.displayValue).toContain("2026年5月1日");
  });
});
