import { useState, useCallback } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { X, Download, CheckCircle, AlertCircle } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { UpdateMetadata, DownloadEvent } from "../../lib/api";
import { installUpdate, relaunchApp } from "../../lib/api";

interface UpdateDialogProps {
  open: boolean;
  update: UpdateMetadata | null;
  onClose: () => void;
}

type UpdateStatus = "idle" | "downloading" | "finished" | "error";

export default function UpdateDialog({ open, update, onClose }: UpdateDialogProps) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const [progress, setProgress] = useState({ downloaded: 0, total: 0 });
  const [error, setError] = useState<string | null>(null);

  const handleDownload = useCallback(async () => {
    if (!update) return;

    setStatus("downloading");
    setError(null);

    try {
      await installUpdate((event: DownloadEvent) => {
        switch (event.event) {
          case "Started":
            setProgress({
              downloaded: 0,
              total: event.data.contentLength ?? 0,
            });
            break;
          case "Progress":
            setProgress((prev) => ({
              ...prev,
              downloaded: event.data.totalDownloaded,
            }));
            break;
          case "Finished":
            setStatus("finished");
            break;
        }
      });
    } catch (err) {
      setStatus("error");
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [update]);

  const handleRelaunch = useCallback(async () => {
    try {
      await relaunchApp();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  const handleClose = useCallback(() => {
    if (status === "downloading") return;
    onClose();
  }, [status, onClose]);

  if (!update) return null;

  const progressPercent = progress.total > 0
    ? Math.round((progress.downloaded / progress.total) * 100)
    : 0;

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
  };

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-[60] flex items-center justify-center bg-black/45 px-4 backdrop-blur-sm"
          onClick={handleClose}
        >
          <motion.div
            initial={{ opacity: 0, y: 12, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 8, scale: 0.98 }}
            transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
            className="w-full max-w-md rounded-[20px] border border-border-subtle bg-surface p-6 shadow-[0_24px_70px_rgba(0,0,0,0.22)]"
            onClick={(event) => event.stopPropagation()}
            role="dialog"
            aria-modal="true"
            aria-labelledby="update-dialog-title"
          >
            <div className="flex items-start justify-between gap-3">
              <div>
                <h2
                  id="update-dialog-title"
                  className="text-[16px] font-semibold tracking-tight text-foreground"
                >
                  {t("update.title")}
                </h2>
                <p className="mt-1 text-[13px] text-muted/65">
                  {t("update.available", { version: update.version })}
                </p>
              </div>
              {status !== "downloading" && (
                <button
                  onClick={handleClose}
                  className="flex h-7 w-7 items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-subtle hover:text-foreground"
                  aria-label={t("update.close")}
                >
                  <X size={16} />
                </button>
              )}
            </div>

            {update.body && (
              <div className="mt-4 max-h-[200px] overflow-y-auto rounded-[10px] border border-border-subtle bg-subtle/30 p-3">
                <p className="whitespace-pre-wrap text-[12px] leading-relaxed text-foreground/80">
                  {update.body}
                </p>
              </div>
            )}

            {status === "downloading" && (
              <div className="mt-5">
                <div className="flex items-center justify-between text-[12px] text-muted/65">
                  <span>{t("update.downloading")}</span>
                  <span>
                    {progress.total > 0
                      ? `${formatBytes(progress.downloaded)} / ${formatBytes(progress.total)}`
                      : formatBytes(progress.downloaded)}
                  </span>
                </div>
                <div className="mt-2 h-2 overflow-hidden rounded-full bg-subtle">
                  <motion.div
                    className="h-full gradient-primary"
                    initial={{ width: 0 }}
                    animate={{ width: progress.total > 0 ? `${progressPercent}%` : "100%" }}
                    transition={{ duration: 0.3 }}
                  />
                </div>
                {progress.total > 0 && (
                  <p className="mt-1 text-center text-[11px] text-muted/50">
                    {progressPercent}%
                  </p>
                )}
              </div>
            )}

            {status === "finished" && (
              <div className="mt-4 flex items-center gap-2 rounded-[10px] border border-success/15 bg-success/8 px-3 py-2">
                <CheckCircle size={16} className="text-success" />
                <p className="text-[12px] text-success">
                  {t("update.downloadComplete")}
                </p>
              </div>
            )}

            {status === "error" && error && (
              <div className="mt-4 flex items-start gap-2 rounded-[10px] border border-error/15 bg-error/8 px-3 py-2">
                <AlertCircle size={16} className="mt-0.5 shrink-0 text-error" />
                <p className="text-[12px] text-error">{error}</p>
              </div>
            )}

            <div className="mt-5 flex justify-end gap-2">
              {status === "idle" && (
                <>
                  <button
                    onClick={handleClose}
                    className="rounded-[10px] border border-border-subtle px-4 py-2 text-[13px] font-medium text-foreground/75 transition-colors hover:border-border hover:bg-subtle hover:text-foreground"
                  >
                    {t("update.later")}
                  </button>
                  <button
                    onClick={handleDownload}
                    className="flex items-center gap-2 rounded-[10px] gradient-primary px-4 py-2 text-[13px] font-medium text-white transition-opacity hover:opacity-90"
                  >
                    <Download size={14} />
                    {t("update.download")}
                  </button>
                </>
              )}

              {status === "downloading" && (
                <button
                  disabled
                  className="cursor-not-allowed rounded-[10px] gradient-primary px-4 py-2 text-[13px] font-medium text-white opacity-70"
                >
                  {t("update.downloading")}
                </button>
              )}

              {status === "finished" && (
                <button
                  onClick={handleRelaunch}
                  className="flex items-center gap-2 rounded-[10px] gradient-primary px-4 py-2 text-[13px] font-medium text-white transition-opacity hover:opacity-90"
                >
                  <CheckCircle size={14} />
                  {t("update.relaunch")}
                </button>
              )}

              {status === "error" && (
                <>
                  <button
                    onClick={handleClose}
                    className="rounded-[10px] border border-border-subtle px-4 py-2 text-[13px] font-medium text-foreground/75 transition-colors hover:border-border hover:bg-subtle hover:text-foreground"
                  >
                    {t("update.close")}
                  </button>
                  <button
                    onClick={handleDownload}
                    className="flex items-center gap-2 rounded-[10px] gradient-primary px-4 py-2 text-[13px] font-medium text-white transition-opacity hover:opacity-90"
                  >
                    <Download size={14} />
                    {t("update.retry")}
                  </button>
                </>
              )}
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
