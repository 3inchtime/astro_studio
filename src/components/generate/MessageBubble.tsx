import { motion } from "framer-motion";
import { Sparkles, Loader2 } from "lucide-react";
import type { Message } from "../../types";
import ImageGrid from "./ImageGrid";
import { useTranslation } from "react-i18next";

interface MessageBubbleProps {
  message: Message;
  onImageClick: (imagePath: string, allImages: string[], index: number) => void;
}

export default function MessageBubble({ message, onImageClick }: MessageBubbleProps) {
  const { t } = useTranslation();

  if (message.role === "user") {
    return (
      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
        className="flex justify-end"
      >
        <div className="max-w-[75%]">
          <div className="rounded-[20px] rounded-br-[6px] bg-primary px-5 py-3.5 shadow-sm">
            <p className="text-[14px] leading-[1.7] text-white whitespace-pre-wrap">
              {message.content}
            </p>
          </div>
        </div>
      </motion.div>
    );
  }

  // Assistant message
  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
      className="flex items-start gap-3"
    >
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full gradient-primary shadow-card">
        <Sparkles size={14} className="text-white" strokeWidth={2.5} />
      </div>
      <div className="max-w-[80%] flex-1 min-w-0">
        <div className="rounded-[20px] rounded-bl-[6px] bg-surface border border-border-subtle px-5 py-3.5 shadow-sm">
          {message.status === "processing" && (
            <div className="space-y-3">
              <div className="flex items-center gap-2 text-[13px] text-muted">
                <Loader2 size={14} className="animate-spin text-primary" />
                {t("generate.generating")}
              </div>
              <div className="grid grid-cols-2 gap-2">
                {[0, 1, 2, 3].map((i) => (
                  <div key={i} className="aspect-square rounded-[12px] shimmer" style={{ animationDelay: `${i * 0.15}s` }} />
                ))}
              </div>
            </div>
          )}

          {message.status === "complete" && message.imagePath && (
            <div className="rounded-[16px] overflow-hidden border-[3px] border-white shadow-[0_8px_20px_rgba(0,0,0,0.1)] -mx-1 -my-1">
              <ImageGrid
                images={[{ path: message.imagePath, thumbnail: message.thumbnailPath }]}
                onImageClick={(path, images, idx) => onImageClick(path, images, idx)}
              />
            </div>
          )}

          {message.status === "failed" && (
            <div className="text-[13px] text-error">
              {message.error || t("generate.generationFailed")}
            </div>
          )}
        </div>
      </div>
    </motion.div>
  );
}
