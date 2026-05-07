import { motion, AnimatePresence } from "framer-motion";
import { X } from "lucide-react";

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  confirmLabel: string;
  cancelLabel: string;
  onConfirm: () => void;
  onCancel: () => void;
  loading?: boolean;
  error?: string | null;
}

export default function ConfirmDialog({
  open,
  title,
  confirmLabel,
  cancelLabel,
  onConfirm,
  onCancel,
  loading = false,
  error = null,
}: ConfirmDialogProps) {
  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-[60] flex items-center justify-center bg-black/45 px-4 backdrop-blur-sm"
          onClick={onCancel}
        >
          <motion.div
            initial={{ opacity: 0, y: 12, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 8, scale: 0.98 }}
            transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
            className="w-full max-w-sm rounded-[20px] border border-border-subtle bg-surface p-5 shadow-[0_24px_70px_rgba(0,0,0,0.22)]"
            onClick={(event) => event.stopPropagation()}
            role="dialog"
            aria-modal="true"
            aria-labelledby="confirm-dialog-title"
          >
            <div className="flex items-start justify-between gap-3">
              <h2
                id="confirm-dialog-title"
                className="text-[15px] font-semibold tracking-tight text-foreground"
              >
                {title}
              </h2>
              <button
                onClick={onCancel}
                disabled={loading}
                className="flex h-7 w-7 items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-subtle hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
                aria-label={cancelLabel}
              >
                <X size={16} />
              </button>
            </div>

            {error ? (
              <div
                role="alert"
                className="mt-4 rounded-[10px] border border-error/15 bg-error/8 px-3 py-2 text-[12px] text-error"
              >
                {error}
              </div>
            ) : null}

            <div className="mt-5 flex justify-end gap-2">
              <button
                onClick={onCancel}
                disabled={loading}
                className="rounded-[10px] border border-border-subtle px-4 py-2 text-[13px] font-medium text-foreground/75 transition-colors hover:border-border hover:bg-subtle hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
              >
                {cancelLabel}
              </button>
              <button
                onClick={onConfirm}
                disabled={loading}
                className="rounded-[10px] border border-error/15 bg-error/8 px-4 py-2 text-[13px] font-medium text-error transition-colors hover:border-error/25 hover:bg-error/12 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {confirmLabel}
              </button>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
