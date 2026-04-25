import { motion } from "framer-motion";
import { Sparkles, Loader2, Clock } from "lucide-react";
import type { Message } from "../../types";
import ImageGrid from "./ImageGrid";
import { useTranslation } from "react-i18next";

interface MessageBubbleProps {
  message: Message;
  onImageClick: (imagePath: string, allImages: string[], index: number) => void;
}

function formatMessageTime(dateStr: string): string {
  const date = new Date(dateStr);
  return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

export default function MessageBubble({ message, onImageClick }: MessageBubbleProps) {
  const { t } = useTranslation();

  if (message.role === "user") {
    return (
      <motion.div
        initial={{ opacity: 0, y: 10, scale: 0.97 }}
        animate={{ opacity: 1, y: 0, scale: 1 }}
        transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
        className="flex justify-end"
      >
        <div className="max-w-[75%] flex flex-col items-end">
          <div className="relative rounded-2xl rounded-br-md bg-primary/10 px-4 py-3 border border-primary/10">
            {/* Bubble tail */}
            <div className="absolute bottom-0 right-[-6px] w-3 h-3 overflow-hidden">
              <div className="absolute top-0 left-0 w-full h-full bg-primary/10 border-r border-b border-primary/10 transform rotate-45 translate-y-[-6px]" />
            </div>
            <p className="text-[13px] leading-relaxed text-foreground whitespace-pre-wrap">
              {message.content}
            </p>
          </div>
          <span className="mt-1 mr-2 flex items-center gap-1 text-[10px] text-muted/40">
            <Clock size={9} />
            {formatMessageTime(message.createdAt)}
          </span>
        </div>
      </motion.div>
    );
  }

  // Assistant message
  return (
    <motion.div
      initial={{ opacity: 0, y: 10, scale: 0.97 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
      className="flex items-start gap-2.5"
    >
      <div className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full gradient-primary shadow-card mt-0.5">
        <Sparkles size={12} className="text-white" strokeWidth={2.5} />
      </div>
      <div className="flex-1 min-w-0 flex flex-col">
        <div className="relative rounded-2xl rounded-bl-md bg-surface px-4 py-3 border border-border-subtle shadow-card">
          {/* Bubble tail */}
          <div className="absolute bottom-0 left-[-6px] w-3 h-3 overflow-hidden">
            <div className="absolute top-0 right-0 w-full h-full bg-surface border-l border-b border-border-subtle transform rotate-45 translate-y-[-6px]" />
          </div>

          {message.status === "processing" && (
            <div className="space-y-3">
              <div className="flex items-center gap-2 text-[12px] text-muted">
                <Loader2 size={13} className="animate-spin text-primary" />
                {t("generate.generating")}
              </div>
              <div className="grid grid-cols-2 gap-2">
                {[0, 1, 2, 3].map((i) => (
                  <div key={i} className="aspect-square rounded-[10px] shimmer" style={{ animationDelay: `${i * 0.15}s` }} />
                ))}
              </div>
            </div>
          )}

          {message.status === "complete" && message.imagePath && (
            <div className="-mx-1 -my-1">
              <ImageGrid
                images={[{ path: message.imagePath, thumbnail: message.thumbnailPath }]}
                onImageClick={(path, images, idx) => onImageClick(path, images, idx)}
              />
            </div>
          )}

          {message.status === "failed" && (
            <div className="text-[12px] text-error">
              {message.error || t("generate.generationFailed")}
            </div>
          )}
        </div>
        <span className="mt-1 ml-2 flex items-center gap-1 text-[10px] text-muted/40">
          <Clock size={9} />
          {formatMessageTime(message.createdAt)}
        </span>
      </div>
    </motion.div>
  );
}
