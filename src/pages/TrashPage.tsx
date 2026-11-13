import { useEffect, useRef, useState } from "react";
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
import ConfirmDialog from "../components/common/ConfirmDialog";
import CollectionStatus from "../components/gallery/CollectionStatus";
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

  const requestIdRef = useRef(0);
  const appliedQueryRef = useRef("");
  const mutationBusyRef = useRef(false);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);

  async function loadTrash(p: number, q = query) {
    const requestId = ++requestIdRef.current;
    appliedQueryRef.current = q;
    setLoading(true);
    setLoadError(false);
    try {
      const result = await searchGenerations(q.trim() || undefined, p, true);
      if (requestId !== requestIdRef.current) return;
      setResults(result.generations);
      setTotal(result.total);
      setPage(result.page);
      setPageSize(result.page_size);
      setSelected(null);
    } catch {
      if (requestId === requestIdRef.current) setLoadError(true);
    } finally {
      if (requestId === requestIdRef.current) setLoading(false);
    }
  }

  useEffect(() => {
    void loadTrash(1);
    getTrashSettings()
      .then((settings) => setRetentionDays(settings.retention_days))
      .catch(() => {});
    return () => {
      requestIdRef.current++;
    };
  }, []);

  function handleSearch() {
    void loadTrash(1, query);
  }

  async function handleRestore(id: string) {
    if (mutationBusyRef.current) return;
    mutationBusyRef.current = true;
    setBusy(true);
    setActionError(null);
    try {
      await restoreGeneration(id);
      refreshConversations();
      await loadTrash(1, appliedQueryRef.current);
      setSelected(null);
    } catch {
      setActionError(
        t("studio.collection.actionError", {
          defaultValue: "The change could not be saved. Please try again.",
        }),
      );
    } finally {
      mutationBusyRef.current = false;
      setBusy(false);
    }
  }

  async function handlePermanentDelete() {
    if (!deleteTarget || mutationBusyRef.current) return;
    mutationBusyRef.current = true;
    setBusy(true);
    setActionError(null);
    try {
      await permanentlyDeleteGeneration(deleteTarget);
      refreshConversations();
      setDeleteTarget(null);
      await loadTrash(1, appliedQueryRef.current);
      setSelected(null);
    } catch {
      setActionError(
        t("studio.collection.actionError", {
          defaultValue: "The change could not be saved. Please try again.",
        }),
      );
    } finally {
      mutationBusyRef.current = false;
      setBusy(false);
    }
  }

  const totalPages = Math.ceil(total / pageSize);

  return (
    <div className="flex h-full overflow-hidden bg-background">
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="studio-toolbar flex flex-col gap-3 px-6 py-4 lg:flex-row lg:items-center lg:justify-between">
          <div className="flex items-center gap-3">
            <h2 className="text-[20px] font-semibold text-foreground tracking-tight">
              {t("trash.title")}
            </h2>
            {total > 0 && (
              <span className="rounded-[6px] bg-subtle px-2 py-0.5 text-[12px] font-medium text-muted tabular-nums">
                {total}
              </span>
            )}
          </div>

          <div className="relative">
            <Search
              size={13}
              className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"
              strokeWidth={2}
            />
            <input
              aria-label={t("trash.search")}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleSearch()}
              placeholder={t("trash.search")}
              className="studio-input focus-ring h-9 w-full sm:w-52 rounded-[8px] pl-7 pr-3 text-[14px] placeholder:text-muted"
            />
          </div>
        </div>

        <div className="border-b border-border-subtle bg-subtle/20 px-6 py-3">
          <div className="flex items-start gap-3">
            <div className="flex h-8 w-8 shrink-0 items-center justify-center text-muted">
              <Trash2 size={14} className="text-muted" />
            </div>
            <div>
              <p className="text-[12px] font-medium text-foreground">
                {t("trash.autoDeleteNotice", { days: retentionDays })}
              </p>
              <p className="mt-1 text-[12px] leading-relaxed text-muted">
                {t("trash.settingsHint")}
              </p>
            </div>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-5">
          <CollectionStatus
            loading={loading}
            error={loadError}
            onRetry={handleSearch}
          />
          {actionError && !deleteTarget && (
            <p role="alert" className="mb-4 rounded-lg border border-error/20 bg-error/5 px-3 py-2 text-sm text-error">
              {actionError}
            </p>
          )}
          {results.length === 0 ? (
            !loading &&
            !loadError && (
              <EmptyCollectionState
                title={t("trash.emptyTitle")}
                subtitle={t("trash.emptyHint")}
              />
            )
          ) : (
            <GenerationGrid
              results={results}
              favoriteMode="hidden"
              onSelect={setSelected}
            />
          )}

          <PaginationControls
            page={page}
            totalPages={totalPages}
            onPageChange={(p) => loadTrash(p, query)}
          />
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
            onDelete={(id) => {
              setActionError(null);
              setDeleteTarget(id);
            }}
            busy={busy}
            onRestore={(id) => void handleRestore(id)}
            deleteLabel={t("trash.deleteNow")}
            restoreLabel={t("trash.restore")}
            deletedAtLabel={t("trash.deletedAt")}
          />
        )}
      </AnimatePresence>
      <ConfirmDialog
        open={deleteTarget !== null}
        title={t("studio.trash.confirmDelete", {
          defaultValue: "Permanently delete this generation and its images?",
        })}
        description={t("studio.trash.cannotUndo", {
          defaultValue:
            "These files will be removed from your device. This cannot be undone.",
        })}
        confirmLabel={t("trash.deleteNow")}
        cancelLabel={t("projects.deleteCancel")}
        loading={busy}
        error={actionError}
        onConfirm={() => void handlePermanentDelete()}
        onCancel={() => {
          if (!busy) setDeleteTarget(null);
        }}
      />
    </div>
  );
}
