import { useEffect, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { Search, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  getTrashSettings,
  permanentlyDeleteGeneration,
  restoreGeneration,
  searchGenerations,
} from "../lib/api";
import type { GenerationResult } from "../types";
import EmptyCollectionState from "../components/gallery/EmptyCollectionState";
import GenerationDetailPanel from "../components/gallery/GenerationDetailPanel";
import GenerationGrid from "../components/gallery/GenerationGrid";
import PaginationControls from "../components/gallery/PaginationControls";
import { useLayoutContext } from "../components/layout/AppLayout";

export default function TrashPage() {
  const { t } = useTranslation();
  const { refreshConversations } = useLayoutContext();
  const [results, setResults] = useState<GenerationResult[]>([]);
  const [query, setQuery] = useState("");
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [selected, setSelected] = useState<GenerationResult | null>(null);
  const [retentionDays, setRetentionDays] = useState(30);

  async function loadTrash(p: number, q?: string) {
    const result = await searchGenerations(q || query || undefined, p, true);
    setResults(result.generations);
    setTotal(result.total);
    setPage(result.page);
    setPageSize(result.page_size);
  }

  useEffect(() => {
    loadTrash(1);
    getTrashSettings().then((settings) => setRetentionDays(settings.retention_days));
  }, []);

  function handleSearch() {
    loadTrash(1, query);
  }

  async function handleRestore(id: string) {
    await restoreGeneration(id);
    refreshConversations();
    await loadTrash(page, query);
    if (selected?.generation.id === id) setSelected(null);
  }

  async function handlePermanentDelete(id: string) {
    await permanentlyDeleteGeneration(id);
    refreshConversations();
    await loadTrash(page, query);
    if (selected?.generation.id === id) setSelected(null);
  }

  const totalPages = Math.ceil(total / pageSize);

  return (
    <div className="flex h-full overflow-hidden">
      <div className="flex flex-1 flex-col">
        <div className="flex flex-col gap-3 border-b border-border-subtle px-6 py-4 lg:flex-row lg:items-center lg:justify-between">
          <div className="flex items-center gap-3">
            <h2 className="text-[15px] font-semibold text-foreground tracking-tight">
              {t("trash.title")}
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
              placeholder={t("trash.search")}
              className="h-[30px] w-52 rounded-[8px] border border-border-subtle bg-subtle/40 pl-7 pr-3 text-[12px] text-foreground placeholder:text-muted/50 focus:outline-none focus:border-border focus:bg-surface transition-colors"
            />
          </div>
        </div>

        <div className="border-b border-border-subtle bg-subtle/20 px-6 py-3">
          <div className="flex items-start gap-3 rounded-[12px] border border-border-subtle bg-surface px-4 py-3 shadow-card">
            <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-[10px] border border-error/10 bg-error/5">
              <Trash2 size={14} className="text-error/70" />
            </div>
            <div>
              <p className="text-[12px] font-medium text-foreground">
                {t("trash.autoDeleteNotice", { days: retentionDays })}
              </p>
              <p className="mt-1 text-[11px] leading-relaxed text-muted/70">
                {t("trash.settingsHint")}
              </p>
            </div>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-5">
          {results.length === 0 ? (
            <EmptyCollectionState title={t("trash.emptyTitle")} subtitle={t("trash.emptyHint")} />
          ) : (
            <GenerationGrid results={results} favoriteMode="hidden" onSelect={setSelected} />
          )}

          <PaginationControls page={page} totalPages={totalPages} onPageChange={(p) => loadTrash(p, query)} />
        </div>
      </div>

      <AnimatePresence>
        {selected && (
          <GenerationDetailPanel
            result={selected}
            title={t("trash.detail")}
            showSaveButton
            showManageFolders={false}
            onClose={() => setSelected(null)}
            onDelete={(id) => void handlePermanentDelete(id)}
            onRestore={(id) => void handleRestore(id)}
            deleteLabel={t("trash.deleteNow")}
            restoreLabel={t("trash.restore")}
            deletedAtLabel={t("trash.deletedAt")}
          />
        )}
      </AnimatePresence>
    </div>
  );
}
