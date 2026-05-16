import { motion } from "framer-motion";
import { useMemo, useState } from "react";
import {
  toAssetUrl,
  copyImageToClipboard,
  saveImageToFile,
} from "../../lib/api";
import { Copy, Download, Trash2, Wand2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import FavoriteButton from "../favorites/FavoriteButton";

interface ImageItem {
  path: string;
  thumbnail?: string;
  imageId: string;
  generationId: string;
}

interface ImageGridProps {
  images: ImageItem[];
  onImageClick: (images: ImageItem[], index: number) => void;
  onDelete?: (generationId: string) => void;
  onEditImage?: (image: ImageItem) => void;
  onFavoriteClick?: (imageId: string) => void;
}

function ImagePreview({
  image,
  isMultiImage,
}: {
  image: ImageItem;
  isMultiImage: boolean;
}) {
  const primarySrc = useMemo(() => {
    if (isMultiImage) {
      return image.thumbnail || image.path;
    }
    return image.path;
  }, [image.path, image.thumbnail, isMultiImage]);
  const [src, setSrc] = useState(primarySrc);

  return (
    <img
      src={toAssetUrl(src)}
      alt="Generated"
      className={`block h-full transition-transform duration-500 group-hover:scale-[1.03] ${
        isMultiImage
          ? "w-full object-cover"
          : "w-auto max-w-full object-cover object-center"
      }`}
      loading="lazy"
      onError={() => {
        if (!isMultiImage && image.thumbnail && src !== image.thumbnail) {
          setSrc(image.thumbnail);
        }
      }}
    />
  );
}

export default function ImageGrid({
  images,
  onImageClick,
  onDelete,
  onEditImage,
  onFavoriteClick,
}: ImageGridProps) {
  const { t } = useTranslation();
  const isMultiImage = images.length > 1;

  if (images.length === 0) return null;

  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.96 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.35, ease: [0.22, 1, 0.36, 1] }}
      className={
        isMultiImage
          ? "grid grid-cols-2 gap-3"
          : "inline-flex flex-col items-start"
      }
    >
      {images.map((img, i) => (
        <div key={img.path} className={isMultiImage ? "min-w-0" : "w-fit"}>
          <div
            onClick={() => onImageClick(images, i)}
            className={`group relative cursor-pointer overflow-hidden rounded-[18px] bg-subtle/70 shadow-[inset_0_0_0_1px_rgba(255,255,255,0.45)] ${
              isMultiImage
                ? "h-60 w-full"
                : "inline-block h-72 w-fit max-w-[min(76vw,36rem)]"
            }`}
          >
            <ImagePreview image={img} isMultiImage={isMultiImage} />
            <div className="absolute bottom-3 left-1/2 mx-auto flex w-fit -translate-x-1/2 items-center justify-center gap-1 rounded-full border border-white/55 bg-surface/86 px-1.5 py-1 opacity-0 shadow-float backdrop-blur-xl transition-all duration-200 group-hover:translate-y-0 group-hover:opacity-100 group-focus-within:opacity-100">
            <button
              onClick={(event) => {
                event.stopPropagation();
                void copyImageToClipboard(img.path);
              }}
              aria-label={t("lightbox.copy")}
              title={t("lightbox.copy")}
              className="focus-ring p-2 rounded-full text-muted transition-colors hover:bg-subtle hover:text-foreground"
            >
              <Copy size={16} />
            </button>
            <button
              onClick={(event) => {
                event.stopPropagation();
                void saveImageToFile(img.path);
              }}
              aria-label={t("lightbox.download")}
              title={t("lightbox.download")}
              className="focus-ring p-2 rounded-full text-muted transition-colors hover:bg-subtle hover:text-foreground"
            >
              <Download size={16} />
            </button>
            {onEditImage && (
              <button
                onClick={(event) => {
                  event.stopPropagation();
                  onEditImage(img);
                }}
                aria-label={t("lightbox.edit")}
                title={t("lightbox.edit")}
                className="focus-ring p-2 rounded-full text-muted transition-colors hover:bg-subtle hover:text-foreground"
              >
                <Wand2 size={16} />
              </button>
            )}
            <button
              onClick={(event) => {
                event.stopPropagation();
                onDelete?.(img.generationId);
              }}
              aria-label={t("lightbox.delete")}
              title={t("lightbox.delete")}
              className="focus-ring p-2 rounded-full text-muted transition-colors hover:bg-subtle hover:text-foreground"
            >
              <Trash2 size={16} />
            </button>
            <div className="w-px h-5 bg-border mx-1" />
            <div onClick={(event) => event.stopPropagation()}>
            <FavoriteButton
              imageId={img.imageId}
              size={16}
              openSelector={
                onFavoriteClick ? () => onFavoriteClick(img.imageId) : undefined
              }
            />
            </div>
            </div>
          </div>
        </div>
      ))}
    </motion.div>
  );
}
