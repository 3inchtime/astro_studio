import { motion } from "framer-motion";
import { Info } from "lucide-react";
import { useRef } from "react";
import { useTranslation } from "react-i18next";
import { toAssetUrl } from "../../lib/api";
import type { GenerationResult } from "../../types";
import FavoriteButton from "../favorites/FavoriteButton";

interface GenerationGridProps {
  results: GenerationResult[];
  favoriteMode?: "manage" | "status" | "hidden";
  onSelect: (result: GenerationResult) => void;
  onPreview?: (result: GenerationResult, index: number) => void;
  onManageFolders?: (imageId: string) => void;
}

export default function GenerationGrid({
  results,
  favoriteMode = "status",
  onSelect,
  onPreview,
  onManageFolders,
}: GenerationGridProps) {
  const { t } = useTranslation();
  const seenIds = useRef(new Set<string>());
  let newCardOffset = 0;

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
                    className={
                      favoriteMode === "manage"
                        ? "flex h-7 w-7 items-center justify-center rounded-[8px] bg-black/45 text-white/80 opacity-0 backdrop-blur-sm transition-all hover:bg-black/60 hover:text-white group-hover:opacity-100"
                        : "flex h-7 w-7 items-center justify-center rounded-[8px] bg-black/45 text-white/80 backdrop-blur-sm transition-all hover:bg-black/60 hover:text-white"
                    }
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
