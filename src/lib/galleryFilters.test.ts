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

  it("builds gallery search config with only model and date fields", () => {
    const filters: GenerationSearchFilters = {
      model: "gpt-image-2",
      created_from: "2026-05-01",
      created_to: "2026-05-31",
    };
    const config = createGallerySearchConfig((key) => key, filters, () => undefined);

    expect(config.searchLabel).toBe("gallery.prompt");
    expect(config.fields.map((field) => field.key)).toEqual([
      "model",
      "created_from",
      "created_to",
    ]);
  });
});
