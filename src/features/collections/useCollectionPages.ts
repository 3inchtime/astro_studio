import { useCallback, useEffect, useRef, useState } from "react";
import { getFavoriteImages, searchGenerations } from "../../lib/api";
import { queryClient } from "../../lib/queryClient";
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
  const [results, setResults] = useState<GenerationResult[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState(false);
  const request = useRef(0);

  useEffect(() => () => { request.current++; }, []);

  const loadPage = useCallback(async (search: CollectionSearch, page: number, mode: "replace" | "append" = "replace") => {
    const token = ++request.current;
    setLoading(true);
    setLoadError(false);
    try {
      const result = await queryClient.fetchQuery({
        queryKey: ["generations", search, page],
        queryFn: () => fetchPage(search, page),
        staleTime: 0,
      });
      if (request.current !== token) return false;
      setResults(current => {
        if (mode === "replace") return result.generations;
        const seen = new Set(current.map(row => row.generation.id));
        return [...current, ...result.generations.filter(row => !seen.has(row.generation.id))];
      });
      setPage(result.page);
      setTotal(result.total);
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
    setResults(current => current.filter(row => row.generation.id !== id));
    setTotal(current => Math.max(0, current - 1));
  };
  const resetResults = useCallback(() => {
    request.current++;
    setResults([]);
    setTotal(0);
    setPage(1);
  }, []);
  return { results, total, page, pageSize, loading, loadError, loadPage, removeResult, resetResults };
}
