import { AlertCircle, LoaderCircle, RotateCcw } from "lucide-react";
import { useTranslation } from "react-i18next";

/** Shared asynchronous state; previous results remain visible during refresh. */
export default function CollectionStatus({ loading, error, onRetry, errorTitle }: {
  errorTitle?: string;
  loading: boolean;
  error: boolean;
  onRetry: () => void;
}) {
  const { t } = useTranslation();
  if (error) return (
    <div role="alert" className="mb-5 flex flex-wrap items-center gap-3 rounded-xl border border-error/20 bg-surface p-4 text-sm">
      <AlertCircle size={18} className="shrink-0 text-error" />
      <div className="min-w-0 flex-1">
        <p className="font-medium text-foreground">{errorTitle ?? t("studio.collection.loadError", { defaultValue: "Unable to load this collection" })}</p>
        <p className="mt-1 text-xs text-muted">{t("studio.collection.retryHint", { defaultValue: "Your search is saved. Try loading it again." })}</p>
      </div>
      <button onClick={onRetry} className="studio-control focus-ring flex h-9 items-center gap-2 rounded-lg px-3 text-sm">
        <RotateCcw size={14} />{t("studio.collection.retry", { defaultValue: "Try again" })}
      </button>
    </div>
  );
  if (loading) return (
    <div role="status" className="flex items-center gap-2 py-5 text-sm text-muted">
      <LoaderCircle size={16} className="animate-spin motion-reduce:animate-none" />
      {t("studio.collection.loading", { defaultValue: "Loading collection…" })}
    </div>
  );
  return null;
}
