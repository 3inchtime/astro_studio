import { motion } from "framer-motion";
import { toAssetUrl } from "../../lib/api";

interface ImageItem {
  path: string;
  thumbnail?: string;
}

interface ImageGridProps {
  images: ImageItem[];
  onImageClick: (imagePath: string, allImages: string[], index: number) => void;
}

export default function ImageGrid({ images, onImageClick }: ImageGridProps) {
  if (images.length === 0) return null;

  const cols = images.length >= 2 ? 2 : 1;
  const allPaths = images.map((img) => img.path);

  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.96 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.35, ease: [0.22, 1, 0.36, 1] }}
      className={`grid gap-2 ${cols === 2 ? "grid-cols-2" : "grid-cols-1"}`}
    >
      {images.map((img, i) => (
        <div
          key={img.path}
          onClick={() => onImageClick(img.path, allPaths, i)}
          className="group relative cursor-pointer overflow-hidden rounded-[12px] bg-surface shadow-card"
        >
          <img
            src={toAssetUrl(img.thumbnail || img.path)}
            alt="Generated"
            className="w-full object-cover transition-transform duration-500 group-hover:scale-[1.02]"
            loading="lazy"
          />
        </div>
      ))}
    </motion.div>
  );
}
