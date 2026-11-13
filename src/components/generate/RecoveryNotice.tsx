import { useId, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronDown, RotateCcw } from "lucide-react";
import { Link } from "react-router-dom";
import { useTranslation } from "react-i18next";
import {
  getGenerationRecoverySummary,
  recoverGeneration,
} from "../../lib/recoveryApi";
import "../../lib/recoveryI18n";

const buttonClass =
  "focus-ring inline-flex min-h-9 items-center justify-center rounded-lg border border-control-border bg-surface px-3 py-2 text-xs font-medium text-foreground transition-colors hover:bg-subtle disabled:cursor-not-allowed disabled:opacity-50";

export default function RecoveryNotice() {
  const { t } = useTranslation();
  const detailsId = useId();
  const client = useQueryClient();
  const [expanded, setExpanded] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const active = useRef(false);
  const summary = useQuery({
    queryKey: ["generation-recovery"],
    queryFn: getGenerationRecoverySummary,
    staleTime: 15_000,
    refetchInterval: 30_000,
  });
  const recovery = useMutation({
    mutationFn: (id: string) => recoverGeneration(id),
    onSuccess: async (result) => {
      setNotice(t(result.recovered > 0 ? "recovery.saved" : "recovery.failed"));
      await Promise.all([
        client.invalidateQueries({ queryKey: ["generation-recovery"] }),
        client.invalidateQueries({ queryKey: ["generations"] }),
        client.invalidateQueries({ queryKey: ["conversations"] }),
      ]);
    },
    onError: () => setNotice(t("recovery.failed")),
    onSettled: () => {
      active.current = false;
    },
  });
  const entries = summary.data?.entries ?? [];
  const startupRecovered = summary.data?.recovered ?? 0;
  const statusNotice =
    notice ??
    (startupRecovered > 0
      ? t("recovery.startupRecovered", { count: startupRecovered })
      : null);
  if (!entries.length && !statusNotice) return null;
  function recover(id: string) {
    if (active.current) return;
    active.current = true;
    setNotice(null);
    recovery.mutate(id);
  }
  return (
    <section
      aria-label={t("recovery.title")}
      className="shrink-0 border-b border-border-subtle bg-surface px-5 py-2"
    >
      {entries.length > 0 && (
        <button
          type="button"
          aria-expanded={expanded}
          aria-controls={detailsId}
          onClick={() => setExpanded((value) => !value)}
          className="focus-ring flex min-h-9 w-full items-center gap-2 rounded-lg text-left text-xs font-medium text-foreground"
        >
          <RotateCcw size={15} aria-hidden className="shrink-0 text-muted" />
          <span className="flex-1">
            {t("recovery.title")} · {entries.length}
          </span>
          <ChevronDown
            size={15}
            aria-hidden
            className={`shrink-0 transition-transform motion-reduce:transition-none ${expanded ? "rotate-180" : ""}`}
          />
        </button>
      )}
      {expanded && entries.length > 0 && (
        <ul
          id={detailsId}
          className="max-h-56 divide-y divide-border-subtle overflow-y-auto"
        >
          {entries.map((entry) => (
            <li
              key={entry.generation_id}
              className="flex flex-wrap items-center gap-3 py-3"
            >
              <div className="min-w-0 flex-[1_1_320px]">
                <p className="line-clamp-1 break-words text-sm text-foreground">
                  {entry.prompt}
                </p>
                <p className="mt-1 max-w-prose text-xs leading-relaxed text-muted">
                  {t(
                    entry.can_recover
                      ? "recovery.localOnly"
                      : entry.result_unknown
                        ? "recovery.unknown"
                        : "recovery.notSent",
                  )}
                </p>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                {entry.can_recover && (
                  <button
                    type="button"
                    className={buttonClass}
                    disabled={recovery.isPending}
                    onClick={() => recover(entry.generation_id)}
                  >
                    {t(
                      recovery.isPending &&
                        recovery.variables === entry.generation_id
                        ? "recovery.saving"
                        : "recovery.continue",
                    )}
                  </button>
                )}
                <Link
                  className={buttonClass}
                  to="/generate"
                  state={{
                    recipeGenerationId: entry.generation_id,
                    recipeMode: "edit",
                  }}
                >
                  {t("recovery.review")}
                </Link>
              </div>
            </li>
          ))}
        </ul>
      )}
      {statusNotice && (
        <p role="status" className="py-2 text-xs leading-relaxed text-muted">
          {statusNotice}
        </p>
      )}
    </section>
  );
}
