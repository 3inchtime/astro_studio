import { motion } from "framer-motion";
import { Image as ImageIcon } from "lucide-react";

interface EmptyCollectionStateProps {
  title: string;
  subtitle: string;
}

export default function EmptyCollectionState({ title, subtitle }: EmptyCollectionStateProps) {
  return (
    <div className="flex h-full flex-col items-center justify-center">
      <motion.div
        initial={{ opacity: 0, y: 12 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.4, ease: [0.22, 1, 0.36, 1] }}
        className="studio-empty flex min-w-[280px] flex-col items-center rounded-[18px] px-8 py-8 text-center shadow-card"
      >
        <div className="mb-4 flex h-14 w-14 items-center justify-center rounded-[14px] border border-border-subtle bg-gradient-to-br from-primary/6 to-accent/4">
          <ImageIcon size={24} className="text-lavender" strokeWidth={1.4} />
        </div>
        <p className="text-[14px] font-medium text-foreground tracking-tight">{title}</p>
        <p className="mt-1 text-[12px] text-muted">{subtitle}</p>
      </motion.div>
    </div>
  );
}
