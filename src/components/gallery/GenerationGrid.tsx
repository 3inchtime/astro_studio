import { motion } from "framer-motion";
import { Check, Info } from "lucide-react";
import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { distributeMasonry } from "./masonryLayout";
import { useTranslation } from "react-i18next";
import { toAssetUrl } from "../../lib/api";
import type { GalleryViewMode } from "../../lib/galleryViewMode";
import { capturePreviewSource, type PreviewSource } from "../../lib/store";
import { cn, formatLocalDateTime } from "../../lib/utils";
import type { GenerationResult } from "../../types";
import FavoriteButton from "../favorites/FavoriteButton";

interface GenerationGridProps {
  results: GenerationResult[];
  selectedIds?: Set<string>;
  selectionMode?: boolean;
  onToggleSelection?: (id: string) => void;
  viewMode?: GalleryViewMode;
  favoriteMode?: "manage" | "status" | "hidden";
  onSelect: (result: GenerationResult) => void;
  onPreview?: (
    result: GenerationResult,
    index: number,
    source?: PreviewSource,
  ) => void;
  onManageFolders?: (imageId: string) => void;
}

export default function GenerationGrid({
  results,
  selectedIds,
  selectionMode = false,
  onToggleSelection,
  viewMode = "masonry",
  favoriteMode = "status",
  onSelect,
  onPreview,
  onManageFolders,
}: GenerationGridProps) {
  const { t } = useTranslation();
  const seenIds = useRef(new Set<string>());
  const gridRef = useRef<HTMLDivElement>(null);
  const [gridWidth, setGridWidth] = useState(0);
  useLayoutEffect(() => {
    const element = gridRef.current;
    if (!element) return;
    const measure = () => setGridWidth(element.clientWidth);
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [viewMode]);
  const columnCount = Math.max(1, Math.floor((gridWidth + 16) / 236));
  const columns = useMemo(
    () => distributeMasonry(results, columnCount, (gridWidth - (columnCount - 1) * 16) / columnCount),
    [results, columnCount, gridWidth],
  );
  const selectionControl = (result: GenerationResult, overlay = false) => onToggleSelection && (
    <label className={cn(
      "relative flex h-8 w-8 shrink-0 cursor-pointer items-center justify-center transition-opacity",
      overlay && "absolute bottom-2 left-2",
      !selectionMode && !selectedIds?.has(result.generation.id) && "opacity-0 group-hover:opacity-100 focus-within:opacity-100 [@media(hover:none)]:opacity-100",
    )}>
      <input type="checkbox" checked={selectedIds?.has(result.generation.id) ?? false}
        onChange={() => onToggleSelection(result.generation.id)}
        aria-label={t("studio.collection.selectRecord", { defaultValue: "Select generation: {{prompt}}", prompt: result.generation.prompt })}
        className="peer absolute inset-0 h-8 w-8 cursor-pointer opacity-0" />
      <span aria-hidden="true" className="pointer-events-none flex h-5 w-5 items-center justify-center rounded-md border border-control-border bg-surface text-primary-foreground shadow-sm peer-checked:border-primary peer-checked:bg-primary peer-focus-visible:outline-2 peer-focus-visible:outline-offset-2 peer-focus-visible:outline-primary">
        {selectedIds?.has(result.generation.id) && <Check size={13} strokeWidth={3} />}
      </span>
    </label>
  );

  if (viewMode === "list") {
    return (
      <div className="space-y-2">
        {results.map((result) => {
          const img = result.images[0];
          if (!img) return null;

          const isNew = !seenIds.current.has(img.id);
          if (isNew) seenIds.current.add(img.id);

          return (
            <motion.article
              key={img.id}
              initial={isNew ? { opacity: 0, y: 6, scale: 0.99 } : false}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              transition={{
                duration: 0.18,
                ease: [0.22, 1, 0.36, 1],
              }}
              className={cn(
                selectedIds?.has(result.generation.id) && "ring-2 ring-primary",
                "studio-card group grid grid-cols-[104px_minmax(0,1fr)_auto] gap-3 rounded-[8px] p-2.5 hover:studio-card-hover",
              )}
            >
              <button
                type="button"
                onClick={(event) =>
                  onPreview
                    ? onPreview(
                        result,
                        0,
                        capturePreviewSource(event.currentTarget, img.id),
                      )
                    : onSelect(result)
                }
                aria-label={`${t("lightbox.preview")} ${result.generation.prompt}`}
                className="focus-ring relative h-[104px] overflow-hidden rounded-[8px] bg-subtle text-left"
              >
                <img
                  data-preview-image-id={img.id}
                  src={toAssetUrl(img.thumbnail_path || img.file_path)}
                  alt={result.generation.prompt}
                  className="h-full w-full object-cover"
                  decoding="async"
                  loading="lazy"
                />
                {result.images.length > 1 && (
                  <div className="absolute left-2 top-2 rounded-[6px] bg-black/60 px-1.5 py-0.5 text-xs font-medium text-white backdrop-blur-sm">
                    {result.images.length}
                  </div>
                )}
              </button>

              <button
                type="button"
                onClick={() => onSelect(result)}
                className="focus-ring min-w-0 rounded-[8px] py-1 text-left"
              >
                <p className="line-clamp-2 text-sm font-medium leading-snug text-foreground transition-colors group-hover:text-foreground">
                  {result.generation.prompt}
                </p>
                <div className="mt-3 flex min-w-0 flex-wrap items-center gap-1.5 text-xs text-muted">
                  <span className="rounded-[6px] bg-subtle px-2 py-0.5">
                    {result.generation.engine}
                  </span>
                  <span className="rounded-[6px] bg-subtle px-2 py-0.5">
                    {result.generation.size}
                  </span>
                  <span className="rounded-[6px] bg-subtle px-2 py-0.5">
                    {formatLocalDateTime(result.generation.created_at)}
                  </span>
                </div>
              </button>

              <div className="flex shrink-0 items-start gap-1">
                {selectionControl(result)}
                {favoriteMode !== "hidden" && (
                  <div
                    onClick={(e) => e.stopPropagation()}
                    className="focus-ring flex h-8 w-8 items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-subtle hover:text-foreground"
                  >
                    <FavoriteButton
                      imageId={img.id}
                      size={14}
                      onClick={
                        favoriteMode === "manage"
                          ? () => onManageFolders?.(img.id)
                          : undefined
                      }
                    />
                  </div>
                )}
                <button
                  type="button"
                  aria-label={t("gallery.detail")}
                  title={t("gallery.detail")}
                  onClick={() => onSelect(result)}
                  className="focus-ring flex h-8 w-8 cursor-pointer items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-subtle hover:text-foreground"
                >
                  <Info size={14} />
                </button>
              </div>
            </motion.article>
          );
        })}
      </div>
    );
  }

  return (
    <div ref={gridRef} className="grid items-start gap-4" style={{ gridTemplateColumns: `repeat(${columnCount}, minmax(0, 1fr))` }}>
      {columns.map((column, columnIndex) => (
        <div key={columnIndex} data-gallery-column={columnIndex} className="min-w-0 space-y-4">
          {column.map((result) => {
        const img = result.images[0];
        if (!img) return null;
        const aspectRatio =
          img.width > 0 && img.height > 0
            ? `${img.width} / ${img.height}`
            : "1 / 1";

        const isNew = !seenIds.current.has(img.id);
        if (isNew) seenIds.current.add(img.id);

        return (
          <motion.div
            key={img.id}
            initial={isNew ? { opacity: 0, y: 6, scale: 0.98 } : false}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            transition={{ duration: 0.18, ease: [0.22, 1, 0.36, 1] }}
            className={cn(
              selectedIds?.has(result.generation.id) && "ring-2 ring-primary",
              "studio-card group overflow-hidden rounded-[12px] hover:studio-card-hover",
            )}
          >
            <div className="relative overflow-hidden">
              <button
                type="button"
                onClick={(event) =>
                  onPreview
                    ? onPreview(
                        result,
                        0,
                        capturePreviewSource(event.currentTarget, img.id),
                      )
                    : onSelect(result)
                }
                aria-label={`${t("lightbox.preview")} ${result.generation.prompt}`}
                className="focus-ring block h-full w-full cursor-pointer overflow-hidden text-left"
                style={{ aspectRatio }}
              >
                <img
                  data-preview-image-id={img.id}
                  src={toAssetUrl(img.thumbnail_path || img.file_path)}
                  alt={result.generation.prompt}
                  className="h-full w-full object-cover"
                  decoding="async"
                  loading="lazy"
                />
                {result.images.length > 1 && (
                  <div className="absolute left-2 top-2 rounded-[6px] bg-black/60 px-1.5 py-0.5 text-xs font-medium text-white backdrop-blur-sm">
                    {result.images.length}
                  </div>
                )}
              </button>
              {selectionControl(result, true)}
              <div className="absolute right-2 top-2 flex items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100 [@media(hover:none)]:opacity-100">
                {favoriteMode !== "hidden" && (
                  <div
                    onClick={(e) => e.stopPropagation()}
                    className={cn(
                      "focus-ring flex h-8 w-8 items-center justify-center rounded-[8px] bg-black/65 text-white backdrop-blur-sm transition-all hover:bg-black/75 hover:text-white",
                      favoriteMode === "manage"
                        ? "opacity-0 group-hover:opacity-100 focus-within:opacity-100 [@media(hover:none)]:opacity-100"
                        : "",
                    )}
                  >
                    <FavoriteButton
                      imageId={img.id}
                      size={14}
                      className="text-white hover:bg-transparent hover:text-white"
                      onClick={
                        favoriteMode === "manage"
                          ? () => onManageFolders?.(img.id)
                          : undefined
                      }
                    />
                  </div>
                )}
                <button
                  type="button"
                  aria-label={t("gallery.detail")}
                  title={t("gallery.detail")}
                  onClick={(event) => {
                    event.stopPropagation();
                    onSelect(result);
                  }}
                  className="focus-ring flex h-8 w-8 cursor-pointer items-center justify-center rounded-[8px] bg-black/65 text-white backdrop-blur-sm transition-all hover:bg-black/75 hover:text-white"
                >
                  <Info size={14} />
                </button>
              </div>
            </div>
            <div className="px-3 py-2.5">
              <p className="line-clamp-1 text-xs leading-snug text-foreground">
                {result.generation.prompt}
              </p>
            </div>
          </motion.div>
        );
          })}
        </div>
      ))}
    </div>
  );
}
