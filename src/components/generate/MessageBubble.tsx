import { motion } from "framer-motion";
import { Sparkles, Loader2 } from "lucide-react";
import type { Message } from "../../types";
import ImageGrid from "./ImageGrid";

interface MessageBubbleProps {
  message: Message;
  onImageClick: (imagePath: string, allImages: string[], index: number) => void;
}

export default function MessageBubble({ message, onImageClick }: MessageBubbleProps) {
  if (message.role === "user") {
    return (
      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
        className="flex justify-end"
      >
        <div className="max-w-[70%] rounded-2xl rounded-br-sm bg-bubble px-4 py-2.5">
          <p className="text-[13px] leading-relaxed text-foreground/70">
            {message.content}
          </p>
        </div>
      </motion.div>
    );
  }

  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
      className="flex items-start gap-2"
    >
      <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-[8px] gradient-primary mt-0.5">
        <Sparkles size={12} className="text-white" strokeWidth={2.5} />
      </div>
      <div className="flex-1 min-w-0">
        {message.status === "processing" && (
          <div className="space-y-3">
            <div className="flex items-center gap-2 text-[12px] text-muted">
              <Loader2 size={13} className="animate-spin text-primary" />
              Generating...
            </div>
            <div className="grid grid-cols-2 gap-2">
              {[0, 1, 2, 3].map((i) => (
                <div key={i} className="aspect-square rounded-[10px] shimmer" style={{ animationDelay: `${i * 0.15}s` }} />
              ))}
            </div>
          </div>
        )}

        {message.status === "complete" && message.imagePath && (
          <ImageGrid
            images={[{ path: message.imagePath, thumbnail: message.thumbnailPath }]}
            onImageClick={(path, images, idx) => onImageClick(path, images, idx)}
          />
        )}

        {message.status === "failed" && (
          <div className="rounded-[10px] border border-error/20 bg-error/4 p-3 text-[12px] text-error">
            {message.error || "Generation failed"}
          </div>
        )}
      </div>
    </motion.div>
  );
}
