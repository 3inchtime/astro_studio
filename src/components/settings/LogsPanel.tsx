import type { RefObject } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { ChevronLeft, ChevronRight, Copy, FileText, Trash2, X } from "lucide-react";
import type { TFunction } from "i18next";
import type { LogEntry, LogSettings, RuntimeLogEntry } from "../../types";
import {
  formatPersistedLog,
  formatRuntimeLogs,
  formatStructuredText,
} from "../../lib/settingsLogs";

const LOG_TYPES = ["", "api_request", "api_response", "generation", "system"] as const;
const LOG_LEVELS = ["", "debug", "info", "warn", "error"] as const;

const levelColors: Record<string, string> = {
  debug: "bg-gray-100 text-gray-600 dark:bg-gray-800 dark:text-gray-400",
  info: "bg-blue-50 text-blue-600 dark:bg-blue-900/30 dark:text-blue-400",
  warn: "bg-orange-50 text-orange-600 dark:bg-orange-900/30 dark:text-orange-400",
  error: "bg-red-50 text-red-600 dark:bg-red-900/30 dark:text-red-400",
};

const typeLabels: Record<string, string> = {
  api_request: "log.apiRequest",
  api_response: "log.apiResponse",
  generation: "log.generation",
  system: "log.system",
};

const levelLabels: Record<string, string> = {
  debug: "log.debug",
  info: "log.info",
  warn: "log.warn",
  error: "log.error",
};

interface LogsPanelProps {
  t: TFunction;
  logs: LogEntry[];
  totalLogs: number;
  logPage: number;
  totalPages: number;
  logType: string;
  logLevel: string;
  logSettings: LogSettings;
  selectedLog: LogEntry | null;
  responseContent: string | null;
  runtimeLogs: RuntimeLogEntry[];
  runtimeLogsRef: RefObject<HTMLDivElement | null>;
  autoScrollRuntimeLogs: boolean;
  copiedLogTarget: "runtime" | "detail" | null;
  onAutoScrollRuntimeLogsChange: (enabled: boolean) => void;
  onCopyText: (text: string, target: "runtime" | "detail") => void;
  onClearRuntimeLogs: () => void;
  onLogTypeChange: (logType: string) => void;
  onLogLevelChange: (logLevel: string) => void;
  onSaveRetention: (days: number) => void;
  onOpenClearLogs: () => void;
  onSelectLog: (log: LogEntry) => void;
  onLogPageChange: (updater: (page: number) => number) => void;
  onCloseSelectedLog: () => void;
}

export function LogsPanel({
  t,
  logs,
  totalLogs,
  logPage,
  totalPages,
  logType,
  logLevel,
  logSettings,
  selectedLog,
  responseContent,
  runtimeLogs,
  runtimeLogsRef,
  autoScrollRuntimeLogs,
  copiedLogTarget,
  onAutoScrollRuntimeLogsChange,
  onCopyText,
  onClearRuntimeLogs,
  onLogTypeChange,
  onLogLevelChange,
  onSaveRetention,
  onOpenClearLogs,
  onSelectLog,
  onLogPageChange,
  onCloseSelectedLog,
}: LogsPanelProps) {
  return (
    <motion.div
      key="logs"
      initial={{ opacity: 0, x: 10 }}
      animate={{ opacity: 1, x: 0 }}
      exit={{ opacity: 0, x: 10 }}
      transition={{ duration: 0.2 }}
    >
      <div className="space-y-4">
        <div className="rounded-[12px] border border-border-subtle bg-surface shadow-card">
          <div className="flex flex-wrap items-start justify-between gap-3 border-b border-border-subtle px-4 py-3">
            <div>
              <div className="flex items-center gap-2">
                <span className="inline-flex h-2.5 w-2.5 rounded-full bg-emerald-500 shadow-[0_0_0_4px_rgba(16,185,129,0.12)]" />
                <h3 className="text-[13px] font-semibold text-foreground">{t("log.liveTitle")}</h3>
                <span className="rounded-full border border-emerald-500/20 bg-emerald-500/8 px-2 py-0.5 text-[10px] font-medium text-emerald-600">
                  {t("log.liveConnected")}
                </span>
              </div>
              <p className="mt-1 text-[11px] text-muted/60">{t("log.liveDesc")}</p>
            </div>
            <div className="flex items-center gap-2">
              <label className="flex items-center gap-2 rounded-[8px] border border-border-subtle bg-subtle/20 px-3 py-1.5 text-[11px] text-muted/70">
                <input
                  type="checkbox"
                  checked={autoScrollRuntimeLogs}
                  onChange={(e) => onAutoScrollRuntimeLogsChange(e.target.checked)}
                  className="h-3.5 w-3.5 rounded border-border-subtle"
                />
                {t("log.autoScroll")}
              </label>
              <button
                type="button"
                onClick={() => onCopyText(formatRuntimeLogs(runtimeLogs), "runtime")}
                disabled={runtimeLogs.length === 0}
                className="flex h-[30px] items-center gap-1.5 rounded-[8px] border border-border-subtle px-3 text-[11px] text-muted transition-all hover:border-border hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40"
              >
                <Copy size={12} />
                {copiedLogTarget === "runtime" ? t("log.copied") : t("log.copyRuntimeLogs")}
              </button>
              <button
                type="button"
                onClick={onClearRuntimeLogs}
                className="flex h-[30px] items-center gap-1.5 rounded-[8px] border border-border-subtle px-3 text-[11px] text-muted transition-all hover:border-border hover:text-foreground"
              >
                <X size={12} />
                {t("log.clearView")}
              </button>
            </div>
          </div>

          <div
            ref={runtimeLogsRef}
            className="h-[320px] overflow-y-auto bg-[#171412] px-4 py-3 font-mono text-[11px] leading-5 text-stone-200"
          >
            {runtimeLogs.length === 0 ? (
              <div className="flex h-full flex-col items-center justify-center text-center text-stone-400/80">
                <p className="text-[12px] font-medium">{t("log.liveEmpty")}</p>
                <p className="mt-1 max-w-md text-[11px]">{t("log.liveHint")}</p>
              </div>
            ) : (
              <div className="space-y-1.5">
                {runtimeLogs.map((log) => (
                  <div key={log.sequence} className="rounded-[8px] border border-white/5 bg-white/[0.03] px-3 py-2">
                    <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
                      <span className="text-stone-500">{log.timestamp}</span>
                      <span className={`rounded-[4px] px-1.5 py-0.5 text-[10px] font-semibold uppercase ${levelColors[log.level] || "bg-stone-700 text-stone-100"}`}>
                        {log.level}
                      </span>
                      <span className="text-stone-400">{log.target}</span>
                    </div>
                    <p className="mt-1 whitespace-pre-wrap break-words text-stone-100">{log.message}</p>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="flex items-center justify-between border-t border-border-subtle px-4 py-2.5">
            <span className="text-[11px] text-muted/50">
              {t("log.liveRecent", { count: runtimeLogs.length })}
            </span>
            <span className="text-[11px] text-muted/40">runtime-log:new</span>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <select
            value={logType}
            onChange={(e) => onLogTypeChange(e.target.value)}
            className="select-control h-[34px] rounded-[8px] border border-border-subtle bg-subtle/30 px-3 pr-8 text-[12px] text-foreground transition-all focus:border-primary/25 focus:outline-none"
          >
            <option value="">{t("log.allTypes")}</option>
            {LOG_TYPES.filter(Boolean).map((lt) => (
              <option key={lt} value={lt}>{t(typeLabels[lt] || lt)}</option>
            ))}
          </select>

          <select
            value={logLevel}
            onChange={(e) => onLogLevelChange(e.target.value)}
            className="select-control h-[34px] rounded-[8px] border border-border-subtle bg-subtle/30 px-3 pr-8 text-[12px] text-foreground transition-all focus:border-primary/25 focus:outline-none"
          >
            <option value="">{t("log.allLevels")}</option>
            {LOG_LEVELS.filter(Boolean).map((ll) => (
              <option key={ll} value={ll}>{t(levelLabels[ll] || ll)}</option>
            ))}
          </select>

          <div className="flex items-center gap-2">
            <span className="text-[11px] text-muted/60">{t("log.retentionDays")}:</span>
            <select
              value={logSettings.retention_days}
              onChange={(e) => onSaveRetention(Number(e.target.value))}
              className="select-control h-[30px] rounded-[6px] border border-border-subtle bg-subtle/30 px-2 pr-7 text-[11px] text-foreground focus:border-primary/25 focus:outline-none"
            >
              {[3, 7, 14, 30].map((d) => (
                <option key={d} value={d}>{d} {t("log.days")}</option>
              ))}
            </select>
          </div>

          <button
            type="button"
            onClick={onOpenClearLogs}
            className="ml-auto flex h-[34px] items-center gap-1.5 rounded-[8px] border border-border-subtle px-3 text-[12px] text-muted transition-all hover:border-red-300 hover:text-red-500"
          >
            <Trash2 size={12} />
            {t("log.clearLogs")}
          </button>
        </div>

        <div className="rounded-[12px] border border-border-subtle bg-surface shadow-card">
          {logs.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-16 text-muted/40">
              <FileText size={32} className="mb-3" />
              <p className="text-[13px] font-medium">{t("log.noLogs")}</p>
              <p className="mt-1 text-[11px]">{t("log.noLogsHint")}</p>
            </div>
          ) : (
            <>
              <div className="grid grid-cols-[140px_100px_70px_1fr] gap-2 border-b border-border-subtle px-4 py-2 text-[11px] font-medium text-muted/50">
                <span>Time</span>
                <span>{t("log.filterType")}</span>
                <span>{t("log.filterLevel")}</span>
                <span>Message</span>
              </div>
              <div className="max-h-[500px] overflow-y-auto">
                {logs.map((log) => (
                  <button
                    key={log.id}
                    onClick={() => onSelectLog(log)}
                    className={`grid w-full grid-cols-[140px_100px_70px_1fr] gap-2 border-b border-border-subtle/50 px-4 py-2.5 text-left text-[12px] transition-colors hover:bg-subtle/30 ${
                      selectedLog?.id === log.id ? "bg-primary/5" : ""
                    }`}
                  >
                    <span className="text-muted/60 font-mono text-[11px]">{log.timestamp}</span>
                    <span className="text-muted/70">{log.log_type.replace("_", " ")}</span>
                    <span>
                      <span className={`inline-block rounded-[4px] px-1.5 py-0.5 text-[10px] font-medium ${levelColors[log.level] || ""}`}>
                        {log.level.toUpperCase()}
                      </span>
                    </span>
                    <span className="truncate text-foreground">{log.message}</span>
                  </button>
                ))}
              </div>

              <div className="flex items-center justify-between border-t border-border-subtle px-4 py-2.5">
                <span className="text-[11px] text-muted/50">
                  {t("log.totalCount", { count: totalLogs })}
                </span>
                <div className="flex items-center gap-1">
                  <button
                    disabled={logPage <= 1}
                    onClick={() => onLogPageChange((p) => p - 1)}
                    className="flex h-7 w-7 items-center justify-center rounded-[6px] text-muted/50 transition-colors hover:bg-subtle hover:text-foreground disabled:opacity-30"
                  >
                    <ChevronLeft size={14} />
                  </button>
                  <span className="px-2 text-[11px] text-muted/60">{logPage} / {totalPages || 1}</span>
                  <button
                    disabled={logPage >= totalPages}
                    onClick={() => onLogPageChange((p) => p + 1)}
                    className="flex h-7 w-7 items-center justify-center rounded-[6px] text-muted/50 transition-colors hover:bg-subtle hover:text-foreground disabled:opacity-30"
                  >
                    <ChevronRight size={14} />
                  </button>
                </div>
              </div>
            </>
          )}
        </div>

        <AnimatePresence>
          {selectedLog && (
            <motion.div
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: "auto" }}
              exit={{ opacity: 0, height: 0 }}
              className="overflow-hidden"
            >
              <div className="rounded-[12px] border border-border-subtle bg-surface shadow-card">
                <div className="flex items-center justify-between border-b border-border-subtle px-4 py-2.5">
                  <span className="text-[12px] font-medium text-foreground">{t("log.detail")}</span>
                  <div className="flex items-center gap-1.5">
                    <button
                      type="button"
                      onClick={() => onCopyText(formatPersistedLog(selectedLog, responseContent), "detail")}
                      className="flex h-7 items-center gap-1.5 rounded-[6px] border border-border-subtle px-2 text-[11px] text-muted transition-colors hover:border-border hover:text-foreground"
                    >
                      <Copy size={12} />
                      {copiedLogTarget === "detail" ? t("log.copied") : t("log.copyLog")}
                    </button>
                    <button
                      onClick={onCloseSelectedLog}
                      className="flex h-6 w-6 items-center justify-center rounded-[6px] text-muted/40 hover:bg-subtle hover:text-muted"
                    >
                      <X size={13} />
                    </button>
                  </div>
                </div>
                <div className="space-y-3 p-4">
                  <div className="grid grid-cols-[80px_1fr] gap-2 text-[12px]">
                    <span className="text-muted/50">Time</span>
                    <span className="font-mono text-foreground">{selectedLog.timestamp}</span>
                    <span className="text-muted/50">Type</span>
                    <span className="text-foreground">{selectedLog.log_type}</span>
                    <span className="text-muted/50">Level</span>
                    <span>
                      <span className={`inline-block rounded-[4px] px-1.5 py-0.5 text-[10px] font-medium ${levelColors[selectedLog.level] || ""}`}>
                        {selectedLog.level.toUpperCase()}
                      </span>
                    </span>
                    <span className="text-muted/50">Message</span>
                    <pre className="whitespace-pre-wrap break-words text-foreground">
                      {selectedLog.message}
                    </pre>
                    {selectedLog.generation_id && (
                      <>
                        <span className="text-muted/50">Gen ID</span>
                        <span className="font-mono text-[11px] text-foreground">{selectedLog.generation_id}</span>
                      </>
                    )}
                  </div>

                  {selectedLog.metadata && (
                    <div>
                      <h4 className="mb-1 text-[11px] font-medium text-muted/60">{t("log.metadata")}</h4>
                      <pre className="max-h-[200px] overflow-auto whitespace-pre-wrap break-words rounded-[8px] bg-subtle/30 p-3 text-[11px] text-foreground">
                        {formatStructuredText(selectedLog.metadata)}
                      </pre>
                    </div>
                  )}

                  {responseContent && (
                    <div>
                      <h4 className="mb-1 text-[11px] font-medium text-muted/60">{t("log.rawResponse")}</h4>
                      <pre className="max-h-[300px] overflow-auto whitespace-pre-wrap break-words rounded-[8px] bg-subtle/30 p-3 text-[11px] text-foreground">
                        {formatStructuredText(responseContent)}
                      </pre>
                    </div>
                  )}
                </div>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </motion.div>
  );
}
