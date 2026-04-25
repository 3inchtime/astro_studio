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

  const allPaths = images.map((img) => img.path);

  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.96 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.35, ease: [0.22, 1, 0.36, 1] }}
    >
      {images.map((img, i) => (
        <div
          key={img.path}
          onClick={() => onImageClick(img.path, allPaths, i)}
          className="group relative cursor-pointer overflow-hidden"
          style={{ maxHeight: "calc(100vh - 260px)" }}
        >
          <img
            src={toAssetUrl(img.thumbnail || img.path)}
            alt="Generated"
            className="w-full h-auto max-h-[calc(100vh-260px)] object-contain block transition-transform duration-500 group-hover:scale-[1.02]"
            loading="lazy"
          />
        </div>
      ))}
    </motion.div>
  );
}
