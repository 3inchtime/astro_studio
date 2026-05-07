import { describe, expect, it } from "vitest";
import {
  compactFilters,
  isFilterActive,
  updateFilterValue,
} from "./galleryFilters";
import { createGallerySearchConfig } from "./galleryFilterConfig";
import type { GenerationSearchFilters } from "../types";

describe("galleryFilters", () => {
  it("removes empty dropdown values and the any source sentinel", () => {
    const filters: GenerationSearchFilters = {
      model: "gpt-image-2",
      size: "",
      source_image_count: "any",
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
    expect(isFilterActive({ source_image_count: "any" }, "")).toBe(false);
    expect(isFilterActive({}, "sunrise")).toBe(true);
    expect(isFilterActive({ status: "completed" }, "")).toBe(true);
  });

  it("returns a new filters object with the updated value", () => {
    const filters: GenerationSearchFilters = {
      model: "gpt-image-2",
      status: "completed",
    };

    const updated = updateFilterValue(filters, "status", "failed");

    expect(updated).toEqual({
      model: "gpt-image-2",
      status: "failed",
    });
    expect(updated).not.toBe(filters);
    expect(filters.status).toBe("completed");
  });

  it("builds gallery search config with current filter values", () => {
    const filters: GenerationSearchFilters = {
      model: "gpt-image-2",
      source_image_count: "2",
      created_from: "2026-05-01",
      created_to: "",
    };
    const config = createGallerySearchConfig(
      (key) => key,
      filters,
      () => undefined,
    );

    expect(config.fields).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          key: "model",
          value: "gpt-image-2",
        }),
        expect.objectContaining({
          key: "source_image_count",
          value: "2",
        }),
        expect.objectContaining({
          key: "created_from",
          value: "2026-05-01",
        }),
        expect.objectContaining({
          key: "created_to",
          value: "",
        }),
      ]),
    );
  });
});
