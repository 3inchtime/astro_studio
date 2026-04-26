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
        className="flex flex-col items-center"
      >
        <div className="mb-4 flex h-14 w-14 items-center justify-center rounded-[14px] bg-gradient-to-br from-primary/6 to-accent/4 border border-border-subtle">
          <ImageIcon size={24} className="text-lavender" strokeWidth={1.4} />
        </div>
        <p className="text-[14px] font-medium text-foreground tracking-tight">{title}</p>
        <p className="mt-1 text-[12px] text-muted">{subtitle}</p>
      </motion.div>
    </div>
  );
}
