import { motion } from "framer-motion";
import { toAssetUrl, copyImageToClipboard, saveImageToFile } from "../../lib/api";
import { Copy, Download, Trash2 } from "lucide-react";
import FavoriteButton from "../favorites/FavoriteButton";

interface ImageItem {
  path: string;
  thumbnail?: string;
  imageId: string;
  generationId: string;
}

interface ImageGridProps {
  images: ImageItem[];
  onImageClick: (imagePath: string, allImages: string[], index: number, imageId: string) => void;
  onDelete?: (generationId: string) => void;
}

export default function ImageGrid({ images, onImageClick, onDelete }: ImageGridProps) {
  if (images.length === 0) return null;

  const allPaths = images.map((img) => img.path);

  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.96 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.35, ease: [0.22, 1, 0.36, 1] }}
      className="inline-block"
    >
      {images.map((img, i) => (
        <div key={img.path} className="inline-block">
          <div
            onClick={() => onImageClick(img.path, allPaths, i, img.imageId)}
            className="group relative cursor-pointer overflow-hidden rounded-[16px]"
          >
            <img
              src={toAssetUrl(img.thumbnail || img.path)}
              alt="Generated"
              className="block h-[50vh] w-auto transition-transform duration-500 group-hover:scale-[1.02]"
              loading="lazy"
            />
          </div>
          <div className="flex items-center justify-center gap-1 mt-2">
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
            <button
              onClick={() => { if (onDelete && confirm("Delete this image?")) onDelete(img.generationId); }}
              className="p-2 rounded-full hover:bg-subtle text-muted hover:text-foreground transition-colors"
            >
              <Trash2 size={16} />
            </button>
            <div className="w-px h-5 bg-border mx-1" />
            <FavoriteButton imageId={img.imageId} size={16} />
          </div>
        </div>
      ))}
    </motion.div>
  );
}
