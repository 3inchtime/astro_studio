import { useEffect, useRef } from "react";

interface UseInfiniteScrollOptions {
  enabled: boolean;
  hasMore: boolean;
  isLoading: boolean;
  onLoadMore: () => void;
  rootMargin?: string;
}

export function useInfiniteScroll({
  enabled,
  hasMore,
  isLoading,
  onLoadMore,
  rootMargin = "240px 0px",
}: UseInfiniteScrollOptions) {
  const targetRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const node = targetRef.current;
    if (!enabled || !node || !hasMore) {
      return;
    }

    const observer = new IntersectionObserver(
      (entries) => {
        if (!entries[0]?.isIntersecting || isLoading) {
          return;
        }

        onLoadMore();
      },
      { rootMargin },
    );

    observer.observe(node);

    return () => {
      observer.disconnect();
    };
  }, [enabled, hasMore, isLoading, onLoadMore, rootMargin]);

  return targetRef;
}
