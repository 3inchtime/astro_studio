import { AnimatePresence, motion } from "framer-motion";
import { Image as ImageIcon } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { Message, MessageImage, PromptAgentMessage } from "../../types";
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
  agentMessages?: PromptAgentMessage[];
  isPromptAgentRunning?: boolean;
  onAcceptAgentDraft?: (message: PromptAgentMessage) => void;
  onContinueAgentDraft?: (message: PromptAgentMessage) => void;
  onEditAgentDraft?: (message: PromptAgentMessage) => void;
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
  agentMessages = [],
  isPromptAgentRunning = false,
  onAcceptAgentDraft,
  onContinueAgentDraft,
  onEditAgentDraft,
}: GenerationFeedProps) {
  const { t } = useTranslation();
  const visibleAgentMessages = isPromptAgentRunning
    ? [
        ...agentMessages,
        {
          id: "prompt-agent-pending",
          session_id: "pending",
          role: "assistant" as const,
          content: t("generate.agent.thinking"),
          draft_prompt: null,
          selected_skill_ids: [],
          suggested_params: {},
          ready_to_generate: false,
          created_at: new Date().toISOString(),
        },
      ]
    : agentMessages;

  if (messages.length === 0 && visibleAgentMessages.length === 0) {
    return <EmptyState />;
  }

  return (
    <div
      data-testid="generation-feed"
      className="relative min-h-full w-full bg-canvas px-5 py-5"
    >
      <div className="relative mx-auto flex min-h-full w-full max-w-[1200px] flex-col gap-3">
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
          {visibleAgentMessages.map((message) => (
            <MessageBubble
              key={message.id}
              agentMessage={message}
              onAcceptAgentDraft={onAcceptAgentDraft}
              onContinueAgentDraft={onContinueAgentDraft}
              onEditAgentDraft={onEditAgentDraft}
            />
          ))}
        </AnimatePresence>
      </div>
    </div>
  );
}

function EmptyState() {
  const { t } = useTranslation();
  return (
    <div className="relative flex h-full flex-col items-center justify-center overflow-hidden px-6">
      <motion.div
        initial={{ opacity: 0, y: 6 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
        className="relative flex max-w-[440px] flex-col items-center px-6 py-8 text-center"
      >
        <div className="relative mb-6">
          <div className="flex h-14 w-14 items-center justify-center rounded-xl border border-border-subtle bg-surface">
            <ImageIcon size={26} className="text-muted" strokeWidth={1.4} />
          </div>
        </div>
        <p className="text-[20px] font-semibold text-foreground tracking-tight">
          {t("generate.emptyTitle")}
        </p>
        <p className="mt-2 max-w-[360px] text-sm leading-relaxed text-muted">
          {t("generate.emptySubtitle")}
        </p>
      </motion.div>
    </div>
  );
}
