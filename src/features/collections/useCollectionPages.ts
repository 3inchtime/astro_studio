import { useCallback, useEffect, useRef, useState } from "react";
import { getFavoriteImages, searchGenerations } from "../../lib/api";
import type { GenerationResult, GenerationSearchFilters } from "../../types";

type CollectionSearch =
  | { kind: "gallery"; query: string; filters?: GenerationSearchFilters }
  | { kind: "project"; projectId: string; query: string; filters: GenerationSearchFilters }
  | { kind: "favorites"; folderId: string; query: string }
  | { kind: "trash"; query: string };

function fetchPage(search: CollectionSearch, page: number) {
  const query = search.query.trim() || undefined;
  switch (search.kind) {
    case "favorites":
      return getFavoriteImages(search.folderId || undefined, query, page);
    case "trash":
      return searchGenerations(query, page, true);
    case "gallery":
      return searchGenerations(query, page, false, search.filters, undefined);
    case "project":
      return searchGenerations(query, page, false, search.filters, search.projectId);
  }
}

export function useCollectionPages() {
  const [{ results, total }, setRows] = useState({ results: [] as GenerationResult[], total: 0 });
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState(false);
  const request = useRef(0);
  const deletionVersion = useRef(0);

  useEffect(() => () => { request.current++; }, []);

  const loadPage = useCallback(async (search: CollectionSearch, page: number, mode: "replace" | "append" = "replace") => {
    const token = ++request.current;
    setLoading(true);
    setLoadError(false);
    try {
      let result;
      // Each refresh reads SQLite after its caller's mutation has committed.
      // A replacement search that overlaps a deletion also needs a fresh total.
      for (;;) {
        const version = deletionVersion.current;
        result = await fetchPage(search, page);
        if (request.current !== token) return false;
        if (deletionVersion.current === version) break;
        // Infinite scroll computes its next offset from the remaining rows.
        if (mode === "append") return false;
      }
      const { generations, total } = result;
      setRows(current => {
        if (mode === "replace") return { results: generations, total };
        const seen = new Set(current.results.map(row => row.generation.id));
        return { results: [...current.results, ...generations.filter(row => !seen.has(row.generation.id))], total };
      });
      setPage(result.page);
      setPageSize(result.page_size);
      return true;
    } catch {
      if (request.current === token) setLoadError(true);
      return false;
    } finally {
      if (request.current === token) setLoading(false);
    }
  }, []);

  const removeResult = (id: string) => {
    deletionVersion.current++;
    setRows(current => {
      const remaining = current.results.filter(row => row.generation.id !== id);
      if (remaining.length === current.results.length) return current;
      return { results: remaining, total: Math.max(0, current.total - 1) };
    });
  };
  const resetResults = useCallback(() => {
    request.current++;
    setRows({ results: [], total: 0 });
    setPage(1);
  }, []);
  return { results, total, page, pageSize, loading, loadError, loadPage, removeResult, resetResults };
}
