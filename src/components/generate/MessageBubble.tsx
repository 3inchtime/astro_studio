import { useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { Pencil, RotateCcw, Sparkles, Star } from "lucide-react";
import { formatLocalDateTime } from "../../lib/utils";
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
  onReuseRecipe?: (generationId: string, mode: "edit" | "generate") => void;
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
  onReuseRecipe,
  agentMessage,
  onAcceptAgentDraft,
  onContinueAgentDraft,
  onEditAgentDraft,
  chatViewportHeight,
}: MessageBubbleProps) {
  const { t } = useTranslation();
  const [promptExpanded, setPromptExpanded] = useState(false);
  const promptIsLong =
    (message?.content.length ?? 0) > 280 ||
    (message?.content.split("\n").length ?? 0) > 3;
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
          transition={{ duration: 0.18, ease: [0.22, 1, 0.36, 1] }}
          data-message-role="user"
          className="flex justify-start"
        >
          <div className="max-w-[68%] rounded-xl border border-border-subtle bg-surface px-5 py-2.5 text-foreground shadow-card">
            <p className="text-sm leading-[1.65] text-foreground whitespace-pre-wrap">
              {agentMessage.content}
            </p>
            {agentMessage.visual_context && (
              agentMessage.visual_context.text_only
                ? <p className="mt-2 text-body-sm text-muted">{t("promptVision.textOnly")}</p>
                : <details className="mt-3 border-t border-border-subtle pt-2 text-body-sm text-muted">
                    <summary className="focus-ring cursor-pointer rounded-sm">{t("promptVision.summary", { count: agentMessage.visual_context.image_hashes.length })}</summary>
                    <div className="mt-2 space-y-3">
                      {agentMessage.visual_context.source_image_indices?.length ? <p>{t("promptVision.sourcePositions", { positions: agentMessage.visual_context.source_image_indices.join(", ") })}</p> : null}
                      {(["observations", "preserve", "changes"] as const).map((field) => agentMessage.visual_context?.[field].length ? <div key={field}>
                        <p className="font-medium text-foreground">{t(`promptVision.${field}`)}</p>
                        <ul className="mt-1 list-disc space-y-1 pl-4">{agentMessage.visual_context[field].map((line, index) => <li key={index} className="break-words">{line}</li>)}</ul>
                      </div> : null)}
                    </div>
                  </details>
            )}
          </div>
        </motion.div>
      );
    }

    return (
      <motion.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.18, ease: [0.22, 1, 0.36, 1] }}
        data-message-role="assistant"
        className="flex items-start justify-start gap-3"
      >
        <div className="mt-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-border bg-surface text-primary">
          <Sparkles size={14} className="text-primary" strokeWidth={2.5} />
        </div>
        <div className="studio-card max-w-[min(78vw,760px)] rounded-xl  px-5 py-3.5">
          <p className="whitespace-pre-wrap break-words text-sm leading-relaxed text-foreground">
            {agentMessage.content}
          </p>
          {agentMessage.draft_prompt && agentMessage.ready_to_generate && (
            <div className="mt-3 rounded-md border border-border/70 bg-surface/80 p-3">
              <p className="text-xs font-semibold uppercase text-muted">
                {t("generate.agent.finalPrompt")}
              </p>
              <p className="mt-2 whitespace-pre-wrap text-sm leading-relaxed text-foreground">
                {agentMessage.draft_prompt}
              </p>
              <div className="mt-3 flex flex-wrap gap-2">
                <button
                  type="button"
                  className="rounded-md bg-primary px-3 py-1.5 text-xs font-semibold text-primary-foreground"
                  onClick={() => onAcceptAgentDraft?.(agentMessage)}
                >
                  {t("generate.agent.acceptAndGenerate")}
                </button>
                <button
                  type="button"
                  className="rounded-md border border-border/70 px-3 py-1.5 text-xs font-medium text-foreground"
                  onClick={() => onContinueAgentDraft?.(agentMessage)}
                >
                  {t("generate.agent.continueRefining")}
                </button>
                <button
                  type="button"
                  className="rounded-md border border-border/70 px-3 py-1.5 text-xs font-medium text-foreground"
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
        initial={{ opacity: 0, y: 6 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.18 }}
        data-message-role="user"
        className="flex justify-start"
      >
        <div className="group w-full min-w-0 bg-transparent text-foreground selection:bg-primary/15">
          <div className="mb-1 flex min-h-8 items-center gap-3">
            <span className="text-xs font-medium text-foreground">
              {t("studio.prompt", { defaultValue: "Prompt" })}
            </span>
            <time
              dateTime={message.createdAt}
              className="min-w-0 truncate text-xs tabular-nums text-muted"
            >
              {formatLocalDateTime(message.createdAt)}
            </time>
            {(onFavoritePrompt || onEditPrompt) && (
              <div
                data-context-actions
                className="ml-auto flex shrink-0 items-center gap-1"
              >
                {onFavoritePrompt && (
                  <button
                    type="button"
                    onClick={() => onFavoritePrompt(message)}
                    aria-pressed={isPromptFavorited}
                    aria-label={t(
                      isPromptFavorited
                        ? "generate.removePromptFavorite"
                        : "generate.favoritePrompt",
                    )}
                    title={t(
                      isPromptFavorited
                        ? "generate.removePromptFavorite"
                        : "generate.favoritePrompt",
                    )}
                    className={`focus-ring flex h-8 w-8 items-center justify-center rounded-lg transition-colors hover:bg-surface ${isPromptFavorited ? "text-primary" : "text-muted hover:text-foreground"}`}
                  >
                    <Star
                      size={15}
                      fill={isPromptFavorited ? "currentColor" : "none"}
                    />
                  </button>
                )}
                {onEditPrompt && (
                  <button
                    type="button"
                    onClick={() => onEditPrompt(message)}
                    aria-label={t("generate.editPrompt")}
                    title={t("generate.editPrompt")}
                    className="focus-ring flex h-8 w-8 items-center justify-center rounded-lg text-muted transition-colors hover:bg-surface hover:text-foreground"
                  >
                    <Pencil size={15} />
                  </button>
                )}
              </div>
            )}
          </div>
          <p
            className={`${promptIsLong && !promptExpanded ? "line-clamp-3" : ""} whitespace-pre-wrap break-words text-sm leading-6 text-foreground`}
          >
            {message.content}
          </p>
          {promptIsLong && (
            <button
              type="button"
              aria-expanded={promptExpanded}
              onClick={() => setPromptExpanded(!promptExpanded)}
              className="focus-ring mt-2 min-h-8 rounded-lg text-xs font-medium text-primary"
            >
              {t(
                promptExpanded
                  ? "studio.collapsePrompt"
                  : "studio.expandPrompt",
                {
                  defaultValue: promptExpanded
                    ? "Show less"
                    : "Show full prompt",
                },
              )}
            </button>
          )}
          {hasSourceImages && (
            <div className="mt-3 flex flex-wrap gap-2">
              {message.sourceImages?.map((image, index) => (
                <div
                  key={`${image.path}-${index}`}
                  className="overflow-hidden rounded-lg border border-border-subtle bg-surface"
                >
                  <img
                    src={toAssetUrl(image.thumbnailPath || image.path)}
                    alt=""
                    className="block h-16 w-auto max-w-32 object-contain"
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
      </motion.div>
    );
  }

  // Assistant message
  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.18, ease: [0.22, 1, 0.36, 1] }}
      data-message-role="assistant"
      className="flex items-start justify-start gap-3"
    >
      <div className="min-w-0 w-full">
        {message.generationId &&
          message.status !== "processing" &&
          onReuseRecipe && (
            <div className="mb-2 flex flex-wrap gap-2">
              <button
                type="button"
                onClick={() => onReuseRecipe(message.generationId!, "edit")}
                className="studio-control focus-ring min-h-8 rounded-lg px-3 text-xs"
              >
                {t("recipe.useParameters")}
              </button>
              <button
                type="button"
                onClick={() => onReuseRecipe(message.generationId!, "generate")}
                className="studio-control focus-ring min-h-8 rounded-lg px-3 text-xs"
              >
                {t("recipe.regenerate")}
              </button>
            </div>
          )}

        <AnimatePresence initial={false}>
          {message.status === "processing" && (
            <motion.div
              key="loading"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.18, ease: [0.22, 1, 0.36, 1] }}
              className="overflow-hidden rounded-xl "
            >
              {message.partialPreview ? (
                <img
                  src={message.partialPreview}
                  alt={t("generate.advanced.partialImages")}
                  className="max-h-[420px] w-full object-contain"
                />
              ) : (
                <GenerationLoadingScene />
              )}
            </motion.div>
          )}

          {message.status === "complete" &&
            message.images &&
            message.images.length > 0 && (
              <motion.div
                key="image"
                initial={{ opacity: 0, y: 6 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
                className="flex w-full max-w-full justify-center bg-canvas"
              >
                <ImageGrid
                  maxImageHeight={
                    chatViewportHeight
                      ? Math.max(160, Math.min(520, chatViewportHeight - 96))
                      : undefined
                  }
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
              className="studio-card rounded-xl border border-error/20 px-4 py-3"
            >
              <motion.div
                initial={{ opacity: 0, y: 5 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.18 }}
                className="whitespace-pre-wrap break-words text-sm text-error"
              >
                {message.error || t("generate.generationFailed")}
              </motion.div>
              {message.retryRequest && onRetry && (
                <button
                  onClick={() => onRetry(message)}
                  className="studio-control focus-ring mt-3 inline-flex items-center gap-2 rounded-lg px-3 py-2 text-xs font-medium hover:studio-control-hover"
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
