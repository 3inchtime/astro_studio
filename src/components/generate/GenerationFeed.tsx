import { AnimatePresence, motion } from "framer-motion";
import { Image as ImageIcon, Sparkles } from "lucide-react";
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
    <div className="space-y-7 relative min-h-full w-full overflow-hidden px-6 py-7">
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_58%_42%_at_54%_18%,rgba(79,106,255,0.075),transparent_72%),radial-gradient(ellipse_44%_36%_at_88%_8%,rgba(212,145,42,0.055),transparent_68%)]" />
      <div className="pointer-events-none absolute inset-x-8 top-6 h-px bg-gradient-to-r from-transparent via-primary/12 to-transparent" />
      <div className="relative mx-auto flex min-h-full w-full max-w-[980px] flex-col justify-center gap-7">
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
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_50%_38%_at_50%_32%,rgba(79,106,255,0.085),transparent_70%),radial-gradient(ellipse_38%_32%_at_62%_26%,rgba(212,145,42,0.06),transparent_72%)]" />
      <motion.div
        initial={{ opacity: 0, y: 16, filter: "blur(8px)" }}
        animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
        transition={{ duration: 0.6, ease: [0.22, 1, 0.36, 1] }}
        className="studio-empty relative flex max-w-[420px] flex-col items-center rounded-[22px] px-9 py-9 text-center shadow-float"
      >
        <div className="relative mb-6">
          <div className="flex h-20 w-20 items-center justify-center rounded-[20px] border border-border-subtle bg-gradient-to-br from-primary/8 via-surface to-warning/8">
            <ImageIcon size={32} className="text-lavender" strokeWidth={1.4} />
          </div>
          <div className="absolute -right-2 -top-2 flex h-7 w-7 items-center justify-center rounded-full border border-white/70 bg-surface text-primary shadow-card">
            <Sparkles size={13} />
          </div>
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
