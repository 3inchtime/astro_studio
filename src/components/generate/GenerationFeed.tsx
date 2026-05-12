import { AnimatePresence, motion } from "framer-motion";
import { Image as ImageIcon } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { Message, MessageImage } from "../../types";
import MessageBubble from "./MessageBubble";

interface GenerationFeedProps {
  messages: Message[];
  chatViewportHeight: number;
  isPromptFavorited: (message: Message) => boolean;
  onImageClick: (images: MessageImage[], index: number) => void;
  onDelete: (generationId: string) => void;
  onEditImage: (image: MessageImage) => void;
  onEditPrompt: (message: Message) => void;
  onFavoritePrompt: (prompt: string) => void;
  onFavoriteClick: (imageId: string) => void;
  onRetry: (message: Message) => void;
}

export default function GenerationFeed({
  messages,
  chatViewportHeight,
  isPromptFavorited,
  onImageClick,
  onDelete,
  onEditImage,
  onEditPrompt,
  onFavoritePrompt,
  onFavoriteClick,
  onRetry,
}: GenerationFeedProps) {
  if (messages.length === 0) {
    return <EmptyState />;
  }

  return (
    <div className="w-full space-y-7 px-6 py-6">
      <AnimatePresence initial={false}>
        {messages.map((msg) => (
          <MessageBubble
            key={msg.id}
            message={msg}
            onImageClick={onImageClick}
            onDelete={onDelete}
            onEditImage={onEditImage}
            onEditPrompt={onEditPrompt}
            onFavoritePrompt={(message) => onFavoritePrompt(message.content)}
            isPromptFavorited={isPromptFavorited(msg)}
            onFavoriteClick={onFavoriteClick}
            onRetry={onRetry}
            chatViewportHeight={chatViewportHeight}
          />
        ))}
      </AnimatePresence>
    </div>
  );
}

function EmptyState() {
  const { t } = useTranslation();
  return (
    <div className="flex h-full flex-col items-center justify-center px-6">
      <motion.div
        initial={{ opacity: 0, y: 16, filter: "blur(8px)" }}
        animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
        transition={{ duration: 0.6, ease: [0.22, 1, 0.36, 1] }}
        className="studio-empty flex max-w-[360px] flex-col items-center rounded-[18px] px-8 py-8 text-center shadow-card"
      >
        <div className="relative mb-6">
          <div className="flex h-20 w-20 items-center justify-center rounded-[20px] border border-border-subtle bg-gradient-to-br from-primary/8 via-lavender-light to-accent/6">
            <ImageIcon size={32} className="text-lavender" strokeWidth={1.4} />
          </div>
          <div className="absolute -top-1 -right-1 h-3 w-3 rounded-full bg-primary/30 animate-pulse" />
        </div>
        <p className="text-[15px] font-semibold text-foreground tracking-tight">
          {t("generate.emptyTitle")}
        </p>
        <p className="mt-2 max-w-[260px] text-[13px] leading-relaxed text-muted">
          {t("generate.emptySubtitle")}
        </p>
      </motion.div>
    </div>
  );
}
