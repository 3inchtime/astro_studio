import { motion, AnimatePresence } from "framer-motion";
import { RotateCcw, Sparkles } from "lucide-react";
import { toAssetUrl } from "../../lib/api";
import type { Message, MessageImage } from "../../types";
import ImageGrid from "./ImageGrid";
import GenerationLoadingScene from "./GenerationLoadingScene";
import { useTranslation } from "react-i18next";

interface MessageBubbleProps {
  message: Message;
  onImageClick: (images: MessageImage[], index: number) => void;
  onDelete?: (generationId: string) => void;
  onEditImage?: (image: MessageImage) => void;
  onFavoriteClick?: (imageId: string) => void;
  onRetry?: (message: Message) => void;
}

export default function MessageBubble({
  message,
  onImageClick,
  onDelete,
  onEditImage,
  onFavoriteClick,
  onRetry,
}: MessageBubbleProps) {
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
            {message.sourceImages && message.sourceImages.length > 0 && (
              <div className="mt-3 flex flex-wrap gap-2">
                {message.sourceImages.map((image, index) => (
                  <div
                    key={`${image.path}-${index}`}
                    className="overflow-hidden rounded-[12px] border border-white/15 bg-white/10"
                  >
                    <img
                      src={toAssetUrl(image.thumbnailPath || image.path)}
                      alt=""
                      className="h-16 w-16 object-cover"
                    />
                  </div>
                ))}
              </div>
            )}
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
            <motion.div
              key="loading"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0, scale: 1.03, filter: "blur(6px)" }}
              transition={{ duration: 0.45, ease: [0.22, 1, 0.36, 1] }}
              className="overflow-hidden rounded-[22px] rounded-bl-[8px]"
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
                className="inline-block overflow-hidden rounded-[18px] shadow-[0_12px_34px_rgba(0,0,0,0.16)]"
              >
                <ImageGrid
                  images={message.images.map((image) => ({
                    path: image.path,
                    thumbnail: image.thumbnailPath,
                    imageId: image.imageId,
                    generationId: image.generationId,
                  }))}
                  onImageClick={(images, idx) =>
                    onImageClick(
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
              className="rounded-[20px] rounded-bl-[6px] bg-surface border border-border-subtle px-5 py-3.5 shadow-sm"
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
                  className="mt-3 inline-flex items-center gap-2 rounded-[10px] border border-border-subtle px-3 py-2 text-[12px] font-medium text-foreground/75 transition-colors hover:border-border hover:bg-subtle hover:text-foreground"
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
