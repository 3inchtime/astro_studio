import { motion } from "framer-motion";
import { Image as ImageIcon } from "lucide-react";

interface EmptyCollectionStateProps {
  title: string;
  subtitle: string;
}

export default function EmptyCollectionState({ title, subtitle }: EmptyCollectionStateProps) {
  return (
    <div className="flex min-h-[240px] flex-col items-center justify-center">
      <motion.div
        initial={{ opacity: 0, y: 12 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.4, ease: [0.22, 1, 0.36, 1] }}
        className="flex w-full max-w-md flex-col items-center rounded-[12px] px-8 py-8 text-center"
      >
        <div className="mb-4 flex h-14 w-14 items-center justify-center rounded-[12px] border border-border-subtle bg-subtle">
          <ImageIcon size={24} className="text-muted" strokeWidth={1.4} />
        </div>
        <p className="text-sm font-medium text-foreground tracking-tight">{title}</p>
        <p className="mt-1 text-xs text-muted">{subtitle}</p>
      </motion.div>
    </div>
  );
}
