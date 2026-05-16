import { motion, AnimatePresence } from "framer-motion";
import { Pencil, RotateCcw, Sparkles, Star } from "lucide-react";
import { toAssetUrl } from "../../lib/api";
import type { Message, MessageImage, PromptAgentMessage } from "../../types";
import ImageGrid from "./ImageGrid";
import GenerationLoadingScene from "./GenerationLoadingScene";
import { useTranslation } from "react-i18next";

interface MessageBubbleProps {
  message?: Message;
  agentMessage?: PromptAgentMessage;
  onImageClick?: (images: MessageImage[], index: number) => void;
  onDelete?: (generationId: string) => void;
  onEditImage?: (image: MessageImage) => void;
  onEditPrompt?: (message: Message) => void;
  onFavoritePrompt?: (message: Message) => void;
  isPromptFavorited?: boolean;
  onFavoriteClick?: (imageId: string) => void;
  onRetry?: (message: Message) => void;
  onAcceptAgentDraft?: (message: PromptAgentMessage) => void;
  onContinueAgentDraft?: (message: PromptAgentMessage) => void;
  onEditAgentDraft?: (message: PromptAgentMessage) => void;
  chatViewportHeight?: number;
}

export default function MessageBubble({
  message,
  onImageClick,
  onDelete,
  onEditImage,
  onEditPrompt,
  onFavoritePrompt,
  isPromptFavorited,
  onFavoriteClick,
  onRetry,
  agentMessage,
  onAcceptAgentDraft,
  onContinueAgentDraft,
  onEditAgentDraft,
  chatViewportHeight,
}: MessageBubbleProps) {
  const { t } = useTranslation();
  const sourceImageMaxHeight =
    chatViewportHeight && chatViewportHeight > 0
      ? `${Math.round(chatViewportHeight * 0.8)}px`
      : undefined;
  const hasSourceImages = Boolean(message?.sourceImages?.length);

  if (agentMessage) {
    const isUser = agentMessage.role === "user";
    if (isUser) {
      return (
        <motion.div
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
          data-message-role="user"
          className="flex justify-end"
        >
          <div className="max-w-[68%] rounded-[18px] border border-border-subtle bg-surface/88 px-5 py-2.5 text-foreground shadow-card">
            <p className="text-[14px] leading-[1.65] text-foreground whitespace-pre-wrap">
              {agentMessage.content}
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
        data-message-role="assistant"
        className="flex items-start justify-start gap-3"
      >
        <div className="mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-full gradient-primary shadow-card">
          <Sparkles size={14} className="text-white" strokeWidth={2.5} />
        </div>
        <div className="studio-card max-w-[min(78vw,760px)] rounded-[16px] rounded-bl-[5px] px-5 py-3.5">
          <p className="whitespace-pre-wrap break-words text-[13px] leading-relaxed text-foreground/86">
            {agentMessage.content}
          </p>
          {agentMessage.draft_prompt && agentMessage.ready_to_generate && (
            <div className="mt-3 rounded-md border border-border/70 bg-surface/80 p-3">
              <p className="text-[11px] font-semibold uppercase text-muted/70">
                {t("generate.agent.finalPrompt")}
              </p>
              <p className="mt-2 whitespace-pre-wrap text-[13px] leading-relaxed text-foreground">
                {agentMessage.draft_prompt}
              </p>
              <div className="mt-3 flex flex-wrap gap-2">
                <button
                  type="button"
                  className="rounded-md bg-accent px-3 py-1.5 text-[12px] font-semibold text-white"
                  onClick={() => onAcceptAgentDraft?.(agentMessage)}
                >
                  {t("generate.agent.acceptAndGenerate")}
                </button>
                <button
                  type="button"
                  className="rounded-md border border-border/70 px-3 py-1.5 text-[12px] font-medium text-foreground"
                  onClick={() => onContinueAgentDraft?.(agentMessage)}
                >
                  {t("generate.agent.continueRefining")}
                </button>
                <button
                  type="button"
                  className="rounded-md border border-border/70 px-3 py-1.5 text-[12px] font-medium text-foreground"
                  onClick={() => onEditAgentDraft?.(agentMessage)}
                >
                  {t("generate.agent.editManually")}
                </button>
              </div>
            </div>
          )}
        </div>
      </motion.div>
    );
  }

  if (!message) return null;

  if (message.role === "user") {
    return (
      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
        data-message-role="user"
        className="flex justify-end"
      >
        <div className="group flex max-w-[68%] items-start gap-2">
          <div
            className={`border border-border-subtle bg-surface/88 px-5 py-2.5 text-foreground shadow-card selection:bg-primary/18 selection:text-foreground dark:bg-surface-elevated ${
              hasSourceImages ? "rounded-[18px]" : "rounded-[999px]"
            }`}
          >
            <p className="line-clamp-3 text-[14px] leading-[1.65] text-foreground whitespace-pre-wrap">
              {message.content}
            </p>
            {hasSourceImages && (
              <div className="mt-3 flex flex-wrap gap-2">
                {message.sourceImages?.map((image, index) => (
                  <div
                    key={`${image.path}-${index}`}
                    className="max-w-full overflow-hidden rounded-[12px] border border-primary/18 bg-primary/5"
                  >
                    <img
                      src={toAssetUrl(image.thumbnailPath || image.path)}
                      alt=""
                      className={
                        sourceImageMaxHeight
                          ? "block h-auto w-auto max-w-full object-contain"
                          : "h-16 w-16 object-cover"
                      }
                      style={
                        sourceImageMaxHeight
                          ? { maxHeight: sourceImageMaxHeight }
                          : undefined
                      }
                    />
                  </div>
                ))}
              </div>
            )}
          </div>
          {(onFavoritePrompt || onEditPrompt) && (
            <div className="mt-2 flex shrink-0 flex-col gap-1 opacity-0 transition-opacity focus-within:opacity-100 group-hover:opacity-100">
              {onFavoritePrompt && (
                <button
                  onClick={() => onFavoritePrompt(message)}
                  className={`focus-ring flex h-8 w-8 cursor-pointer items-center justify-center rounded-full border bg-surface shadow-sm transition-all ${
                    isPromptFavorited
                      ? "border-primary/18 text-primary hover:bg-primary/6"
                      : "border-primary/14 text-primary/70 hover:bg-primary/6 hover:text-primary"
                  }`}
                  aria-label={
                    isPromptFavorited
                      ? t("generate.removePromptFavorite")
                      : t("generate.favoritePrompt")
                  }
                  title={
                    isPromptFavorited
                      ? t("generate.removePromptFavorite")
                      : t("generate.favoritePrompt")
                  }
                >
                  {isPromptFavorited ? (
                    <Star size={13} fill="currentColor" />
                  ) : (
                    <Star size={13} />
                  )}
                </button>
              )}
              {onEditPrompt && (
                <button
                  onClick={() => onEditPrompt(message)}
                  className="focus-ring flex h-8 w-8 cursor-pointer items-center justify-center rounded-full border border-primary/14 bg-surface text-primary/70 shadow-sm transition-all hover:bg-primary/6 hover:text-primary"
                  aria-label={t("generate.editPrompt")}
                  title={t("generate.editPrompt")}
                >
                  <Pencil size={13} />
                </button>
              )}
            </div>
          )}
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
      data-message-role="assistant"
      className="flex items-start justify-start gap-3"
    >
      <div className="mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-full gradient-primary shadow-card">
        <Sparkles size={14} className="text-white" strokeWidth={2.5} />
      </div>
      <div className="min-w-0 max-w-[min(78vw,760px)]">
        <AnimatePresence mode="wait">
          {message.status === "processing" && (
            <motion.div
              key="loading"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0, scale: 1.03, filter: "blur(6px)" }}
              transition={{ duration: 0.45, ease: [0.22, 1, 0.36, 1] }}
              className="overflow-hidden rounded-[16px] rounded-bl-[5px]"
            >
              <GenerationLoadingScene />
            </motion.div>
          )}

          {message.status === "complete" &&
            message.images &&
            message.images.length > 0 && (
              <motion.div
                key="image"
                initial={{ opacity: 0, scale: 0.85, filter: "blur(8px)" }}
                animate={{ opacity: 1, scale: 1, filter: "blur(0px)" }}
                transition={{ duration: 0.65, ease: [0.22, 1, 0.36, 1] }}
                className="inline-block overflow-hidden rounded-[20px] border border-white/70 bg-surface shadow-[0_20px_54px_rgba(45,42,38,0.16)] ring-1 ring-border-subtle/70"
              >
                <ImageGrid
                  images={message.images.map((image) => ({
                    path: image.path,
                    thumbnail: image.thumbnailPath,
                    imageId: image.imageId,
                    generationId: image.generationId,
                  }))}
                  onImageClick={(images, idx) =>
                    onImageClick?.(
                      images.map((image) => ({
                        imageId: image.imageId,
                        generationId: image.generationId,
                        path: image.path,
                        thumbnailPath: image.thumbnail,
                      })),
                      idx,
                    )
                  }
                  onDelete={onDelete}
                  onEditImage={
                    onEditImage
                      ? (image) =>
                          onEditImage({
                            imageId: image.imageId,
                            generationId: image.generationId,
                            path: image.path,
                            thumbnailPath: image.thumbnail,
                          })
                      : undefined
                  }
                  onFavoriteClick={onFavoriteClick}
                />
              </motion.div>
            )}

          {message.status === "failed" && (
            <div
              key="error"
              className="studio-card rounded-[16px] rounded-bl-[5px] px-5 py-3.5"
            >
              <motion.div
                initial={{ opacity: 0, y: 5 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.3 }}
                className="whitespace-pre-wrap break-words text-[13px] text-error"
              >
                {message.error || t("generate.generationFailed")}
              </motion.div>
              {message.retryRequest && onRetry && (
                <button
                  onClick={() => onRetry(message)}
                  className="studio-control focus-ring mt-3 inline-flex items-center gap-2 rounded-[10px] px-3 py-2 text-[12px] font-medium hover:studio-control-hover"
                >
                  <RotateCcw size={13} />
                  {t("generate.retry")}
                </button>
              )}
            </div>
          )}
        </AnimatePresence>
      </div>
    </motion.div>
  );
}
