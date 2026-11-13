import { motion, AnimatePresence } from "framer-motion";
import { X } from "lucide-react";
import { useId } from "react";
import { useDialogFocus } from "../../hooks/useDialogFocus";

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  description?: string;
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
  description,
  confirmLabel,
  cancelLabel,
  onConfirm,
  onCancel,
  loading = false,
  error = null,
}: ConfirmDialogProps) {
  const titleId = useId();
  const dialogRef = useDialogFocus({ open, onClose: onCancel, busy: loading });
  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-[60] flex items-center justify-center bg-black/45 px-4 backdrop-blur-sm"
          onClick={() => { if (!loading) onCancel(); }}
        >
          <motion.div
            ref={dialogRef}
            tabIndex={-1}
            initial={{ opacity: 0, y: 12, scale: 0.96 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 8, scale: 0.98 }}
            transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
            className="studio-floating-panel w-full max-w-sm rounded-[12px] p-6 outline-none"
            onClick={(event) => event.stopPropagation()}
            role="dialog"
            aria-modal="true"
            aria-labelledby={titleId}
            aria-busy={loading}
          >
            <div className="flex items-start justify-between gap-3">
              <h2
                id={titleId}
                className="text-body-lg font-semibold tracking-tight text-foreground"
              >
                {title}
              </h2>
              <button
                type="button"
                onClick={onCancel}
                disabled={loading}
                className="flex h-7 w-7 items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-subtle hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
                aria-label={cancelLabel}
              >
                <X size={16} />
              </button>
            </div>

            {description && <p className="mt-3 text-sm leading-relaxed text-muted">{description}</p>}
            {error ? (
              <div
                role="alert"
                className="mt-4 rounded-[10px] border border-error/15 bg-error/8 px-3 py-2 text-xs text-error"
              >
                {error}
              </div>
            ) : null}

            <div className="mt-5 flex justify-end gap-2">
              <button
                type="button"
                onClick={onCancel}
                disabled={loading}
                className="rounded-[10px] border border-border-subtle px-4 py-2 text-body-sm font-medium text-foreground transition-colors hover:border-border hover:bg-subtle hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
              >
                {cancelLabel}
              </button>
              <button
                type="button"
                onClick={onConfirm}
                disabled={loading}
                className="rounded-[10px] border border-error/15 bg-error/8 px-4 py-2 text-body-sm font-medium text-error transition-colors hover:border-error/25 hover:bg-error/12 disabled:cursor-not-allowed disabled:opacity-50"
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
