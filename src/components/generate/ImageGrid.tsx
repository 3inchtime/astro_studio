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
      className="w-full"
    >
      {images.map((img, i) => (
        <div
          key={img.path}
          onClick={() => onImageClick(img.path, allPaths, i)}
          className="group relative cursor-pointer overflow-hidden w-full"
          style={{ maxHeight: "calc(100vh - 260px)" }}
        >
          <img
            src={toAssetUrl(img.thumbnail || img.path)}
            alt="Generated"
            className="w-full block transition-transform duration-500 group-hover:scale-[1.02]"
            style={{ maxHeight: "calc(100vh - 260px)", objectFit: "contain", objectPosition: "top" }}
            loading="lazy"
          />
        </div>
      ))}
    </motion.div>
  );
}
