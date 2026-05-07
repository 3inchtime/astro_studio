import type { GenerationSearchFilters } from "../types";

export function compactFilters(
  filters: GenerationSearchFilters,
): GenerationSearchFilters {
  return Object.fromEntries(
    Object.entries(filters).filter(([, value]) => value !== "" && value !== undefined),
  ) as GenerationSearchFilters;
}

export function isFilterActive(
  filters: GenerationSearchFilters,
  query: string,
): boolean {
  const normalized = compactFilters(filters);
  return (
    query.trim().length > 0 ||
    Object.values(normalized).some((value) => value !== undefined && value !== "")
  );
}

export function updateFilterValue<K extends keyof GenerationSearchFilters>(
  current: GenerationSearchFilters,
  key: K,
  value: GenerationSearchFilters[K],
): GenerationSearchFilters {
  return { ...current, [key]: value };
}
