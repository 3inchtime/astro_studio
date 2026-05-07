import { FormEvent, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { X } from "lucide-react";

interface ProjectNameDialogProps {
  open: boolean;
  title: string;
  label: string;
  initialName?: string;
  submitLabel: string;
  cancelLabel: string;
  requiredMessage: string;
  error?: string | null;
  loading?: boolean;
  onSubmit: (name: string) => void;
  onCancel: () => void;
}

export default function ProjectNameDialog({
  open,
  title,
  label,
  initialName = "",
  submitLabel,
  cancelLabel,
  requiredMessage,
  error = null,
  loading = false,
  onSubmit,
  onCancel,
}: ProjectNameDialogProps) {
  const [name, setName] = useState(initialName);
  const [validationError, setValidationError] = useState<string | null>(null);
  const wasOpenRef = useRef(open);

  useEffect(() => {
    if (!wasOpenRef.current && open) {
      setName(initialName);
      setValidationError(null);
    }
    wasOpenRef.current = open;
  }, [initialName, open]);

  function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const trimmedName = name.trim();
    if (!trimmedName) {
      setValidationError(requiredMessage);
      return;
    }
    onSubmit(trimmedName);
  }

  const visibleError = validationError ?? error;

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
            aria-labelledby="project-name-dialog-title"
          >
            <div className="flex items-start justify-between gap-3">
              <h2
                id="project-name-dialog-title"
                className="text-[15px] font-semibold tracking-tight text-foreground"
              >
                {title}
              </h2>
              <button
                type="button"
                onClick={onCancel}
                disabled={loading}
                className="flex h-7 w-7 items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-subtle hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
                aria-label="Close dialog"
              >
                <X size={16} />
              </button>
            </div>

            <form onSubmit={handleSubmit} className="mt-5">
              <label htmlFor="project-name-input" className="text-[12px] font-medium text-foreground/75">
                {label}
              </label>
              <input
                id="project-name-input"
                value={name}
                onChange={(event) => {
                  setName(event.target.value);
                  setValidationError(null);
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.preventDefault();
                    event.currentTarget.form?.requestSubmit();
                  }
                }}
                disabled={loading}
                className="mt-2 h-10 w-full rounded-[10px] border border-border-subtle bg-background px-3 text-[13px] text-foreground outline-none transition-colors placeholder:text-muted focus:border-primary/45 disabled:cursor-not-allowed disabled:opacity-60"
                autoFocus
              />

              {visibleError ? (
                <div
                  role="alert"
                  className="mt-3 rounded-[10px] border border-error/15 bg-error/8 px-3 py-2 text-[12px] text-error"
                >
                  {visibleError}
                </div>
              ) : null}

              <div className="mt-5 flex justify-end gap-2">
                <button
                  type="button"
                  onClick={onCancel}
                  disabled={loading}
                  className="rounded-[10px] border border-border-subtle px-4 py-2 text-[13px] font-medium text-foreground/75 transition-colors hover:border-border hover:bg-subtle hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {cancelLabel}
                </button>
                <button
                  type="submit"
                  disabled={loading}
                  className="rounded-[10px] bg-primary px-4 py-2 text-[13px] font-medium text-white transition-colors hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {submitLabel}
                </button>
              </div>
            </form>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
