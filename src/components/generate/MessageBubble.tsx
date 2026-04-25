import { motion, AnimatePresence } from "framer-motion";
import { Sparkles } from "lucide-react";
import type { Message } from "../../types";
import ImageGrid from "./ImageGrid";
import { useTranslation } from "react-i18next";

interface MessageBubbleProps {
  message: Message;
  onImageClick: (imagePath: string, allImages: string[], index: number, imageId: string) => void;
  onDelete?: (generationId: string) => void;
}

function DreamBubbles() {
  const bubbles = [
    { size: 40, x: 20, delay: 0 },
    { size: 28, x: 60, delay: 0.4 },
    { size: 50, x: 100, delay: 0.8 },
    { size: 22, x: 45, delay: 1.2 },
    { size: 36, x: 80, delay: 0.6 },
    { size: 18, x: 10, delay: 1.0 },
    { size: 32, x: 70, delay: 1.4 },
  ];

  return (
    <div className="relative flex items-center justify-center py-6" style={{ minHeight: 120 }}>
      {/* Background glow */}
      <div
        className="absolute inset-0 flex items-center justify-center"
        style={{ animation: "bubble-glow 3s ease-in-out infinite" }}
      >
        <div className="h-20 w-40 rounded-full bg-primary/5 blur-2xl" />
      </div>

      {/* Floating bubbles */}
      {bubbles.map((b, i) => (
        <div
          key={i}
          className="absolute"
          style={{
            left: `calc(50% - 80px + ${b.x}px)`,
            width: b.size,
            height: b.size,
            animation: `bubble-float 2.4s ease-in-out ${b.delay}s infinite, bubble-shimmer 3s ease-in-out ${b.delay}s infinite`,
          }}
        >
          <div
            className="h-full w-full rounded-full"
            style={{
              background: `linear-gradient(135deg, rgba(79,106,255,0.12) 0%, rgba(124,92,252,0.08) 50%, rgba(184,169,255,0.12) 100%)`,
              backgroundSize: "200% 200%",
              border: "1px solid rgba(79,106,255,0.1)",
              backdropFilter: "blur(4px)",
            }}
          />
        </div>
      ))}

      {/* Center sparkle */}
      <motion.div
        animate={{
          scale: [1, 1.2, 1],
          opacity: [0.5, 0.8, 0.5],
        }}
        transition={{ duration: 2, repeat: Infinity, ease: "easeInOut" }}
        className="relative z-10 flex h-10 w-10 items-center justify-center rounded-full bg-primary/10 border border-primary/10"
      >
        <Sparkles size={16} className="text-primary" />
      </motion.div>
    </div>
  );
}

export default function MessageBubble({ message, onImageClick, onDelete }: MessageBubbleProps) {
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
      <div className="flex-1 min-w-0">
        <AnimatePresence mode="wait">
          {message.status === "processing" && (
            <div
              key="loading"
              className="rounded-[20px] rounded-bl-[6px] bg-surface border border-border-subtle px-5 py-3.5 shadow-sm"
            >
              <motion.div
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                exit={{ opacity: 0, scale: 1.05, filter: "blur(4px)" }}
                transition={{ duration: 0.4, ease: [0.22, 1, 0.36, 1] }}
              >
                <DreamBubbles />
              </motion.div>
            </div>
          )}

          {message.status === "complete" && message.imagePath && (
            <motion.div
              key="image"
              initial={{ opacity: 0, scale: 0.85, filter: "blur(8px)" }}
              animate={{ opacity: 1, scale: 1, filter: "blur(0px)" }}
              transition={{ duration: 0.6, ease: [0.22, 1, 0.36, 1] }}
              className="inline-block rounded-[16px] overflow-hidden shadow-[0_8px_30px_rgba(0,0,0,0.15)]"
            >
              <ImageGrid
                images={[{ path: message.imagePath!, thumbnail: message.thumbnailPath, imageId: `${message.generationId}_0`, generationId: message.generationId! }]}
                onImageClick={(path, images, idx, imgId) => onImageClick(path, images, idx, imgId)}
                onDelete={onDelete}
              />
            </motion.div>
          )}

          {message.status === "failed" && (
            <div
              key="error"
              className="rounded-[20px] rounded-bl-[6px] bg-surface border border-border-subtle px-5 py-3.5 shadow-sm"
            >
              <motion.div
                initial={{ opacity: 0, y: 5 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3 }}
                className="text-[13px] text-error"
              >
                {message.error || t("generate.generationFailed")}
              </motion.div>
            </div>
          )}
        </AnimatePresence>
      </div>
    </motion.div>
  );
}
