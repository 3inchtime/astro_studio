import { motion } from "framer-motion";
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
            className={`group relative cursor-pointer overflow-hidden rounded-[16px] bg-subtle/70 ${
              isMultiImage
                ? "h-60 w-full"
                : "inline-block h-72 w-fit max-w-[min(76vw,36rem)]"
            }`}
          >
            <img
              src={toAssetUrl(img.thumbnail || img.path)}
              alt="Generated"
              className={`block h-full transition-transform duration-500 group-hover:scale-[1.03] ${
                isMultiImage
                  ? "w-full object-cover"
                  : "w-auto max-w-full object-cover object-center"
              }`}
              loading="lazy"
            />
          </div>
          <div className="mt-2 flex w-fit items-center justify-center gap-1 mx-auto">
            <button
              onClick={() => copyImageToClipboard(img.path)}
              className="p-2 rounded-full hover:bg-subtle text-muted hover:text-foreground transition-colors"
            >
              <Copy size={16} />
            </button>
            <button
              onClick={() => saveImageToFile(img.path)}
              className="p-2 rounded-full hover:bg-subtle text-muted hover:text-foreground transition-colors"
            >
              <Download size={16} />
            </button>
            {onEditImage && (
              <button
                onClick={() => onEditImage(img)}
                className="p-2 rounded-full hover:bg-subtle text-muted hover:text-foreground transition-colors"
                title={t("lightbox.edit")}
              >
                <Wand2 size={16} />
              </button>
            )}
            <button
              onClick={() => onDelete?.(img.generationId)}
              className="p-2 rounded-full hover:bg-subtle text-muted hover:text-foreground transition-colors"
              title={t("lightbox.delete")}
            >
              <Trash2 size={16} />
            </button>
            <div className="w-px h-5 bg-border mx-1" />
            <FavoriteButton
              imageId={img.imageId}
              size={16}
              openSelector={
                onFavoriteClick ? () => onFavoriteClick(img.imageId) : undefined
              }
            />
          </div>
        </div>
      ))}
    </motion.div>
  );
}
