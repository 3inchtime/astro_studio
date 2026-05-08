import { motion } from "framer-motion";
import { Sparkles, X } from "lucide-react";
import { useEffect, useRef } from "react";

interface OptimizePromptModalProps {
  open: boolean;
  originalPrompt: string;
  optimizedPrompt: string;
  onUseOptimized: () => void;
  onKeepOriginal: () => void;
  onOptimizedChange: (value: string) => void;
}

export default function OptimizePromptModal({
  open,
  originalPrompt,
  optimizedPrompt,
  onUseOptimized,
  onKeepOriginal,
  onOptimizedChange,
}: OptimizePromptModalProps) {
  const overlayRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === "Escape") onKeepOriginal();
    };
    document.addEventListener("keydown", handleEscape);
    return () => document.removeEventListener("keydown", handleEscape);
  }, [open, onKeepOriginal]);

  if (!open) return null;

  return (
    <motion.div
      ref={overlayRef}
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.2 }}
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/45 px-4 backdrop-blur-sm"
      onClick={(e) => {
        if (e.target === overlayRef.current) onKeepOriginal();
      }}
    >
      <motion.div
        initial={{ opacity: 0, scale: 0.95, y: 12 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.95, y: 12 }}
        transition={{ duration: 0.25, ease: [0.22, 1, 0.36, 1] }}
        onClick={(e) => e.stopPropagation()}
        className="glass-strong w-full max-w-[640px] overflow-hidden rounded-[16px] border border-border-subtle shadow-float"
        role="dialog"
        aria-modal="true"
        aria-label="Optimize Prompt"
      >
        {/* Header */}
        <div className="flex items-center justify-between gap-3 border-b border-border-subtle px-6 py-4">
          <div className="flex items-center gap-2.5">
            <div className="flex h-7 w-7 items-center justify-center rounded-[8px] border border-primary/10 bg-primary/5">
              <Sparkles size={14} className="text-primary" strokeWidth={2} />
            </div>
            <h2 className="text-[14px] font-semibold text-foreground">
              Optimize Prompt
            </h2>
          </div>
          <button
            onClick={onKeepOriginal}
            className="flex h-7 w-7 items-center justify-center rounded-[8px] text-muted/60 transition-colors hover:bg-subtle hover:text-foreground"
          >
            <X size={15} />
          </button>
        </div>

        {/* Two-column comparison */}
        <div className="grid grid-cols-2 gap-4 p-5">
          {/* Original */}
          <div className="flex flex-col gap-2">
            <span className="text-[11px] font-semibold uppercase tracking-[0.06em] text-muted/60">
              Original
            </span>
            <textarea
              readOnly
              value={originalPrompt}
              rows={6}
              className="w-full resize-none rounded-[10px] border border-border-subtle bg-subtle/30 px-3.5 py-3 text-[13px] leading-[1.6] text-foreground/70 placeholder:text-muted/40 focus:outline-none"
            />
          </div>

          {/* Optimized */}
          <div className="flex flex-col gap-2">
            <span className="text-[11px] font-semibold uppercase tracking-[0.06em] text-primary/80">
              Optimized
            </span>
            <textarea
              value={optimizedPrompt}
              onChange={(e) => onOptimizedChange(e.target.value)}
              rows={6}
              className="w-full resize-none rounded-[10px] border border-primary/25 bg-surface px-3.5 py-3 text-[13px] leading-[1.6] text-foreground placeholder:text-muted/40 transition-all duration-200 focus:border-primary/40 focus:shadow-[0_0_0_3px_rgba(79,106,255,0.08)] focus:outline-none"
            />
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 border-t border-border-subtle px-5 py-4">
          <motion.button
            onClick={onKeepOriginal}
            whileTap={{ scale: 0.97 }}
            className="flex h-[34px] items-center justify-center gap-1.5 rounded-[9px] border border-border-subtle bg-surface px-4 text-[12px] font-medium text-muted transition-all hover:border-border hover:text-foreground"
          >
            Keep Original
          </motion.button>
          <motion.button
            onClick={onUseOptimized}
            whileTap={{ scale: 0.97 }}
            className="flex h-[34px] items-center justify-center gap-1.5 rounded-[9px] gradient-primary px-4 text-[12px] font-semibold text-white shadow-[0_2px_8px_rgba(79,106,255,0.25)] transition-shadow hover:shadow-[0_4px_12px_rgba(79,106,255,0.35)]"
          >
            <Sparkles size={13} />
            Use Optimized
          </motion.button>
        </div>
      </motion.div>
    </motion.div>
  );
}
