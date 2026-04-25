import { useEffect, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { searchGenerations, deleteGeneration, toAssetUrl } from "../lib/api";
import type { GenerationResult } from "../types";
import { Search, Trash2, X, Image as ImageIcon, Calendar, Download } from "lucide-react";
import { useTranslation } from "react-i18next";
import FavoriteButton from "../components/favorites/FavoriteButton";
import FolderSelector from "../components/favorites/FolderSelector";

export default function GalleryPage() {
  const { t } = useTranslation();
  const [results, setResults] = useState<GenerationResult[]>([]);
  const [query, setQuery] = useState("");
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [selected, setSelected] = useState<GenerationResult | null>(null);
  const [folderSelectorImageId, setFolderSelectorImageId] = useState<string | null>(null);

  async function loadGenerations(p: number, q?: string) {
    const result = await searchGenerations(q || query || undefined, p);
    setResults(result.generations);
    setTotal(result.total);
    setPage(result.page);
    setPageSize(result.page_size);
  }

  useEffect(() => {
    loadGenerations(1);
  }, []);

  function handleSearch() {
    loadGenerations(1, query);
  }

  async function handleDelete(id: string) {
    await deleteGeneration(id);
    loadGenerations(page, query);
    if (selected?.generation.id === id) setSelected(null);
  }

  const totalPages = Math.ceil(total / pageSize);

  return (
    <div className="flex h-full">
      <div className="flex flex-1 flex-col">
        <div className="flex items-center justify-between border-b border-border-subtle px-6 py-4">
          <div className="flex items-center gap-3">
            <h2 className="text-[15px] font-semibold text-foreground tracking-tight">
              {t("gallery.title")}
            </h2>
            {total > 0 && (
              <span className="rounded-[6px] bg-subtle px-2 py-0.5 text-[10px] font-medium text-muted tabular-nums">
                {total}
              </span>
            )}
          </div>

          <div className="relative">
            <Search size={13} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted/60" strokeWidth={2} />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleSearch()}
              placeholder={t("gallery.search")}
              className="h-[30px] w-52 rounded-[8px] border border-border-subtle bg-subtle/40 pl-7 pr-3 text-[12px] text-foreground placeholder:text-muted/50 focus:outline-none focus:border-border focus:bg-surface transition-colors"
            />
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-5">
          {results.length === 0 ? (
            <div className="flex h-full flex-col items-center justify-center">
              <motion.div
                initial={{ opacity: 0, y: 12 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.4, ease: [0.22, 1, 0.36, 1] }}
                className="flex flex-col items-center"
              >
                <div className="mb-4 flex h-14 w-14 items-center justify-center rounded-[14px] bg-gradient-to-br from-primary/6 to-accent/4 border border-border-subtle">
                  <ImageIcon size={24} className="text-lavender" strokeWidth={1.4} />
                </div>
                <p className="text-[14px] font-medium text-foreground tracking-tight">
                  {t("gallery.noImages")}
                </p>
                <p className="mt-1 text-[12px] text-muted">
                  {t("gallery.emptyHint")}
                </p>
              </motion.div>
            </div>
          ) : (
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4">
              {results.map((result, i) => {
                const img = result.images[0];
                return (
                  <motion.div
                    key={result.generation.id}
                    initial={{ opacity: 0, y: 6, scale: 0.98 }}
                    animate={{ opacity: 1, y: 0, scale: 1 }}
                    transition={{
                      delay: i * 0.03,
                      duration: 0.35,
                      ease: [0.22, 1, 0.36, 1],
                    }}
                    onClick={() => setSelected(result)}
                    className="group cursor-pointer overflow-hidden rounded-[12px] bg-surface border border-border-subtle shadow-card transition-all duration-300 hover:shadow-float hover:border-border hover:-translate-y-0.5"
                  >
                    {img && (
                      <div className="relative overflow-hidden">
                        <img
                          src={toAssetUrl(img.thumbnail_path)}
                          alt={result.generation.prompt}
                          className="aspect-square w-full object-cover transition-transform duration-500 group-hover:scale-[1.04]"
                          loading="lazy"
                        />
                        <div className="absolute top-2 right-2 opacity-0 group-hover:opacity-100 transition-opacity">
                          <FavoriteButton
                            imageId={`${result.generation.id}_0`}
                            size={14}
                            onClick={() => setFolderSelectorImageId(`${result.generation.id}_0`)}
                          />
                        </div>
                      </div>
                    )}
                    <div className="px-3 py-2.5">
                      <p className="line-clamp-2 text-[11px] leading-snug text-muted group-hover:text-foreground/70 transition-colors">
                        {result.generation.prompt}
                      </p>
                    </div>
                  </motion.div>
                );
              })}
            </div>
          )}

          {totalPages > 1 && (
            <div className="mt-6 flex items-center justify-center gap-2">
              <button
                onClick={() => loadGenerations(page - 1)}
                disabled={page <= 1}
                className="h-[28px] rounded-[8px] px-3 text-[11px] text-muted hover:bg-subtle disabled:opacity-30 transition-all"
              >
                {t("gallery.prev")}
              </button>
              <span className="px-2 text-[11px] text-muted tabular-nums">
                {page} / {totalPages}
              </span>
              <button
                onClick={() => loadGenerations(page + 1)}
                disabled={page >= totalPages}
                className="h-[28px] rounded-[8px] px-3 text-[11px] text-muted hover:bg-subtle disabled:opacity-30 transition-all"
              >
                {t("gallery.next")}
              </button>
            </div>
          )}
        </div>
      </div>

      <AnimatePresence>
        {selected && (
          <motion.div
            initial={{ width: 0, opacity: 0 }}
            animate={{ width: 340, opacity: 1 }}
            exit={{ width: 0, opacity: 0 }}
            transition={{ duration: 0.25, ease: [0.22, 1, 0.36, 1] }}
            className="w-[340px] h-full overflow-y-auto overflow-hidden border-l border-border-subtle bg-surface"
          >
            <div className="p-5">
              <div className="mb-4 flex items-center justify-between">
                <h3 className="text-[13px] font-semibold text-foreground tracking-tight">
                  {t("gallery.detail")}
                </h3>
                <button
                  onClick={() => setSelected(null)}
                  className="flex h-6 w-6 items-center justify-center rounded-[8px] text-muted hover:bg-subtle transition-colors"
                >
                  <X size={14} />
                </button>
              </div>

              {selected.images[0] && (
                <motion.div
                  initial={{ opacity: 0, scale: 0.96 }}
                  animate={{ opacity: 1, scale: 1 }}
                  transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
                  className="mb-4 overflow-hidden rounded-[12px] border border-border-subtle"
                >
                  <img
                    src={toAssetUrl(selected.images[0].file_path)}
                    alt={selected.generation.prompt}
                    className="w-full"
                  />
                </motion.div>
              )}

              <div className="space-y-3">
                <div>
                  <span className="text-[10px] font-medium uppercase tracking-wider text-muted/50">
                    {t("gallery.prompt")}
                  </span>
                  <p className="mt-1 text-[13px] leading-relaxed text-foreground/80">
                    {selected.generation.prompt}
                  </p>
                </div>
                <div className="flex gap-5">
                  <div>
                    <span className="text-[10px] font-medium uppercase tracking-wider text-muted/50">
                      {t("gallery.size")}
                    </span>
                    <p className="mt-0.5 text-[13px] text-foreground/80">
                      {selected.generation.size}
                    </p>
                  </div>
                  <div>
                    <span className="text-[10px] font-medium uppercase tracking-wider text-muted/50">
                      {t("gallery.quality")}
                    </span>
                    <p className="mt-0.5 text-[13px] text-foreground/80">
                      {selected.generation.quality}
                    </p>
                  </div>
                </div>
                <div className="flex items-center gap-1.5">
                  <Calendar size={11} className="text-muted/40" />
                  <span className="text-[11px] text-muted/60">
                    {selected.generation.created_at}
                  </span>
                </div>
              </div>

              <div className="mt-6 space-y-2">
                <button className="flex w-full items-center justify-center gap-2 rounded-[10px] border border-border-subtle py-2.5 text-[12px] font-medium text-foreground/70 transition-all hover:border-border hover:bg-subtle hover:text-foreground">
                  <Download size={13} />
                  {t("gallery.saveImage")}
                </button>
                <button
                  onClick={() => handleDelete(selected.generation.id)}
                  className="flex w-full items-center justify-center gap-2 rounded-[10px] border border-error/10 py-2.5 text-[12px] font-medium text-error/60 transition-all hover:border-error/20 hover:bg-error/4 hover:text-error"
                >
                  <Trash2 size={13} />
                  {t("gallery.delete")}
                </button>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {folderSelectorImageId && (
        <FolderSelector imageId={folderSelectorImageId} onClose={() => setFolderSelectorImageId(null)} />
      )}
    </div>
  );
}
