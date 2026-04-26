import { motion } from "framer-motion";
import { toAssetUrl } from "../../lib/api";
import type { GenerationResult } from "../../types";
import FavoriteButton from "../favorites/FavoriteButton";

interface GenerationGridProps {
  results: GenerationResult[];
  favoriteMode?: "manage" | "status" | "hidden";
  onSelect: (result: GenerationResult) => void;
  onManageFolders?: (imageId: string) => void;
}

export default function GenerationGrid({
  results,
  favoriteMode = "status",
  onSelect,
  onManageFolders,
}: GenerationGridProps) {
  return (
    <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
      {results.map((result, i) => {
        const img = result.images[0];
        if (!img) return null;

        return (
          <motion.div
            key={result.generation.id}
            initial={{ opacity: 0, y: 6, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            transition={{ delay: i * 0.03, duration: 0.35, ease: [0.22, 1, 0.36, 1] }}
            onClick={() => onSelect(result)}
            className="group cursor-pointer overflow-hidden rounded-[12px] bg-surface border border-border-subtle shadow-card transition-all duration-300 hover:shadow-float hover:border-border hover:-translate-y-0.5"
          >
            <div className="relative overflow-hidden">
              <img
                src={toAssetUrl(img.thumbnail_path)}
                alt={result.generation.prompt}
                className="aspect-square w-full object-cover transition-transform duration-500 group-hover:scale-[1.04]"
                loading="lazy"
              />
              {result.images.length > 1 && (
                <div className="absolute left-2 top-2 rounded-[6px] bg-black/60 px-1.5 py-0.5 text-[10px] font-medium text-white backdrop-blur-sm">
                  {result.images.length}
                </div>
              )}
              {favoriteMode !== "hidden" && (
                <div
                  onClick={(e) => e.stopPropagation()}
                  className={
                    favoriteMode === "manage"
                      ? "absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity"
                      : "absolute top-2 right-2"
                  }
                >
                  <FavoriteButton
                    imageId={img.id}
                    size={14}
                    onClick={favoriteMode === "manage" ? () => onManageFolders?.(img.id) : undefined}
                  />
                </div>
              )}
            </div>
            <div className="px-3 py-2.5">
              <p className="line-clamp-2 text-[11px] leading-snug text-muted group-hover:text-foreground/70 transition-colors">
                {result.generation.prompt}
              </p>
            </div>
          </motion.div>
        );
      })}
    </div>
  );
}
