import { motion } from "framer-motion";
import { Info } from "lucide-react";
import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { toAssetUrl } from "../../lib/api";
import type { GalleryViewMode } from "../../lib/galleryViewMode";
import { cn, formatLocalDateTime } from "../../lib/utils";
import type { GenerationResult } from "../../types";
import FavoriteButton from "../favorites/FavoriteButton";

interface GenerationGridProps {
  results: GenerationResult[];
  viewMode?: GalleryViewMode;
  favoriteMode?: "manage" | "status" | "hidden";
  onSelect: (result: GenerationResult) => void;
  onPreview?: (result: GenerationResult, index: number) => void;
  onManageFolders?: (imageId: string) => void;
}

export default function GenerationGrid({
  results,
  viewMode = "masonry",
  favoriteMode = "status",
  onSelect,
  onPreview,
  onManageFolders,
}: GenerationGridProps) {
  const { t } = useTranslation();
  const seenIds = useRef(new Set<string>());
  let newCardOffset = 0;

  if (viewMode === "list") {
    return (
      <div className="space-y-2">
        {results.map((result) => {
          const img = result.images[0];
          if (!img) return null;

          const isNew = !seenIds.current.has(img.id);
          if (isNew) seenIds.current.add(img.id);
          const delay = isNew ? newCardOffset * 0.03 : 0;
          if (isNew) newCardOffset++;

          return (
            <motion.article
              key={img.id}
              initial={isNew ? { opacity: 0, y: 6, scale: 0.99 } : false}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              transition={{
                delay,
                duration: 0.3,
                ease: [0.22, 1, 0.36, 1],
              }}
              className="group grid grid-cols-[104px_minmax(0,1fr)_auto] gap-3 rounded-[10px] border border-border-subtle bg-surface p-2.5 shadow-sm transition-colors hover:border-border hover:bg-surface/95"
            >
              <button
                type="button"
                onClick={() => (onPreview ? onPreview(result, 0) : onSelect(result))}
                aria-label={`Preview ${result.generation.prompt}`}
                className="relative h-[104px] overflow-hidden rounded-[8px] bg-subtle text-left"
              >
                <img
                  src={toAssetUrl(img.thumbnail_path)}
                  alt={result.generation.prompt}
                  className="h-full w-full object-cover"
                  loading="lazy"
                />
                {result.images.length > 1 && (
                  <div className="absolute left-2 top-2 rounded-[6px] bg-black/60 px-1.5 py-0.5 text-[10px] font-medium text-white backdrop-blur-sm">
                    {result.images.length}
                  </div>
                )}
              </button>

              <button
                type="button"
                onClick={() => onSelect(result)}
                className="min-w-0 py-1 text-left"
              >
                <p className="line-clamp-2 text-[13px] font-medium leading-snug text-foreground/88 transition-colors group-hover:text-foreground">
                  {result.generation.prompt}
                </p>
                <div className="mt-3 flex min-w-0 flex-wrap items-center gap-1.5 text-[11px] text-muted">
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
                {favoriteMode !== "hidden" && (
                  <div
                    onClick={(e) => e.stopPropagation()}
                    className="flex h-8 w-8 items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-subtle hover:text-foreground"
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
                  className="flex h-8 w-8 items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-subtle hover:text-foreground"
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
    <div className="columns-2 gap-3 sm:columns-3 lg:columns-4">
      {results.map((result) => {
        const img = result.images[0];
        if (!img) return null;
        const aspectRatio =
          img.width > 0 && img.height > 0
            ? `${img.width} / ${img.height}`
            : "1 / 1";

        const isNew = !seenIds.current.has(img.id);
        if (isNew) seenIds.current.add(img.id);
        const delay = isNew ? newCardOffset * 0.03 : 0;
        if (isNew) newCardOffset++;

        return (
          <motion.div
            key={img.id}
            initial={isNew ? { opacity: 0, y: 6, scale: 0.98 } : false}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            transition={{ delay, duration: 0.35, ease: [0.22, 1, 0.36, 1] }}
            className="group mb-3 break-inside-avoid overflow-hidden rounded-[12px] bg-surface border border-border-subtle shadow-card transition-shadow duration-300 hover:shadow-float hover:border-border"
          >
            <div className="relative overflow-hidden">
              <button
                type="button"
                onClick={() => (onPreview ? onPreview(result, 0) : onSelect(result))}
                aria-label={`Preview ${result.generation.prompt}`}
                className="block h-full w-full cursor-pointer overflow-hidden text-left"
                style={{ aspectRatio }}
              >
                <img
                  src={toAssetUrl(img.thumbnail_path)}
                  alt={result.generation.prompt}
                  className="h-full w-full object-cover"
                  loading="lazy"
                />
                {result.images.length > 1 && (
                  <div className="absolute left-2 top-2 rounded-[6px] bg-black/60 px-1.5 py-0.5 text-[10px] font-medium text-white backdrop-blur-sm">
                    {result.images.length}
                  </div>
                )}
              </button>
              <div className="absolute right-2 top-2 flex items-center gap-1">
                {favoriteMode !== "hidden" && (
                  <div
                    onClick={(e) => e.stopPropagation()}
                    className={cn(
                      "flex h-7 w-7 items-center justify-center rounded-[8px] bg-black/45 text-white/80 backdrop-blur-sm transition-all hover:bg-black/60 hover:text-white",
                      favoriteMode === "manage"
                        ? "opacity-0 group-hover:opacity-100"
                        : "",
                    )}
                  >
                    <FavoriteButton
                      imageId={img.id}
                      size={14}
                      onClick={favoriteMode === "manage" ? () => onManageFolders?.(img.id) : undefined}
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
                  className="flex h-7 w-7 items-center justify-center rounded-[8px] bg-black/45 text-white/80 backdrop-blur-sm transition-all hover:bg-black/60 hover:text-white"
                >
                  <Info size={13} />
                </button>
              </div>
            </div>
            <div className="px-3 py-2.5">
              <p className="line-clamp-2 text-[11px] leading-snug text-muted transition-colors group-hover:text-foreground/70">
                {result.generation.prompt}
              </p>
            </div>
          </motion.div>
        );
      })}
    </div>
  );
}
