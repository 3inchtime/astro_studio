import { useState, useCallback, useEffect, useRef } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  X,
  ChevronLeft,
  ChevronRight,
  ZoomIn,
  ZoomOut,
  RotateCcw,
  Copy,
  Download,
  Trash2,
} from "lucide-react";
import { toAssetUrl, copyImageToClipboard, saveImageToFile } from "../../lib/api";
import { useTranslation } from "react-i18next";
import FavoriteButton from "../favorites/FavoriteButton";
import FolderSelector from "../favorites/FolderSelector";

interface LightboxProps {
  images: string[];
  initialIndex: number;
  onClose: () => void;
  onDelete?: (imagePath: string) => void;
  imageId?: string;
}

const MIN_ZOOM = 0.5;
const MAX_ZOOM = 3.0;

export default function Lightbox({ images, initialIndex, onClose, onDelete, imageId }: LightboxProps) {
  const { t } = useTranslation();
  const [index, setIndex] = useState(initialIndex);
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [isPanning, setIsPanning] = useState(false);
  const [showFolderSelector, setShowFolderSelector] = useState(false);
  const lastPoint = useRef({ x: 0, y: 0 });
  const containerRef = useRef<HTMLDivElement>(null);

  const currentPath = images[index];

  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    setZoom((prev) => {
      const next = prev - e.deltaY * 0.001;
      return Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, next));
    });
  }, []);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (zoom > 1) {
      setIsPanning(true);
      lastPoint.current = { x: e.clientX, y: e.clientY };
    }
  }, [zoom]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (!isPanning) return;
    const dx = e.clientX - lastPoint.current.x;
    const dy = e.clientY - lastPoint.current.y;
    lastPoint.current = { x: e.clientX, y: e.clientY };
    setPan((prev) => ({ x: prev.x + dx, y: prev.y + dy }));
  }, [isPanning]);

  const handleMouseUp = useCallback(() => {
    setIsPanning(false);
  }, []);

  const resetView = useCallback(() => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
  }, []);

  const goPrev = useCallback(() => {
    setIndex((prev) => (prev > 0 ? prev - 1 : images.length - 1));
    resetView();
  }, [images.length, resetView]);

  const goNext = useCallback(() => {
    setIndex((prev) => (prev < images.length - 1 ? prev + 1 : 0));
    resetView();
  }, [images.length, resetView]);

  const toggleZoom = useCallback(() => {
    if (zoom === 1) setZoom(2);
    else resetView();
  }, [zoom, resetView]);

  useEffect(() => {
    function handleKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
      else if (e.key === "ArrowLeft") goPrev();
      else if (e.key === "ArrowRight") goNext();
    }
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [onClose, goPrev, goNext]);

  const handleCopy = useCallback(async () => {
    try {
      await copyImageToClipboard(currentPath);
    } catch {
      // Toast notification handled by caller
    }
  }, [currentPath]);

  const handleDownload = useCallback(async () => {
    try {
      await saveImageToFile(currentPath);
    } catch {
      // Error handled by caller
    }
  }, [currentPath]);

  const handleDelete = useCallback(() => {
    if (onDelete && confirm(t("lightbox.deleteConfirm"))) {
      onDelete(currentPath);
    }
  }, [currentPath, onDelete]);

  return (
    <AnimatePresence>
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.2 }}
        className="fixed inset-0 z-50 flex flex-col bg-black/90 backdrop-blur-sm"
        onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
      >
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3">
          <button onClick={onClose} className="flex h-8 w-8 items-center justify-center rounded-[8px] text-white/70 hover:bg-white/10 hover:text-white transition-colors">
            <X size={18} />
          </button>
          <span className="text-[13px] text-white/50">{index + 1} / {images.length}</span>
          <div className="w-8" />
        </div>

        {/* Image area */}
        <div
          ref={containerRef}
          className="flex flex-1 items-center justify-center overflow-hidden"
          onWheel={handleWheel}
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
          onMouseLeave={handleMouseUp}
          style={{ cursor: zoom > 1 ? (isPanning ? "grabbing" : "grab") : "default" }}
        >
          <img
            src={toAssetUrl(currentPath)}
            alt={t("lightbox.preview")}
            className="max-h-[80vh] max-w-[80vw] object-contain select-none"
            style={{ transform: `scale(${zoom}) translate(${pan.x / zoom}px, ${pan.y / zoom}px)` }}
            onDoubleClick={toggleZoom}
            draggable={false}
          />
        </div>

        {/* Navigation arrows */}
        {images.length > 1 && (
          <>
            <button onClick={goPrev} className="absolute left-4 top-1/2 -translate-y-1/2 flex h-10 w-10 items-center justify-center rounded-full bg-black/40 text-white/70 hover:bg-black/60 hover:text-white transition-colors">
              <ChevronLeft size={20} />
            </button>
            <button onClick={goNext} className="absolute right-4 top-1/2 -translate-y-1/2 flex h-10 w-10 items-center justify-center rounded-full bg-black/40 text-white/70 hover:bg-black/60 hover:text-white transition-colors">
              <ChevronRight size={20} />
            </button>
          </>
        )}

        {/* Toolbar */}
        <div className="flex items-center justify-center gap-1 px-4 py-3">
          {[
            { icon: ZoomIn, label: t("lightbox.zoomIn"), onClick: () => setZoom((z) => Math.min(MAX_ZOOM, z + 0.25)) },
            { icon: ZoomOut, label: t("lightbox.zoomOut"), onClick: () => setZoom((z) => Math.max(MIN_ZOOM, z - 0.25)) },
            { icon: RotateCcw, label: t("lightbox.reset"), onClick: resetView },
            { icon: Copy, label: t("lightbox.copy"), onClick: handleCopy },
            { icon: Download, label: t("lightbox.download"), onClick: handleDownload },
            ...(onDelete ? [{ icon: Trash2, label: t("lightbox.delete"), onClick: handleDelete }] : []),
          ].map(({ icon: Icon, label, onClick }) => (
            <button
              key={label}
              onClick={onClick}
              title={label}
              className="flex h-9 w-9 items-center justify-center rounded-[8px] text-white/60 hover:bg-white/10 hover:text-white transition-colors"
            >
              <Icon size={16} strokeWidth={1.8} />
            </button>
          ))}
          {imageId && (
            <button
              onClick={() => setShowFolderSelector(true)}
              title="Add to folder"
              className="flex h-9 w-9 items-center justify-center rounded-[8px] text-white/60 hover:bg-white/10 hover:text-white transition-colors"
            >
              <FavoriteButton imageId={imageId} size={16} />
            </button>
          )}
        </div>
        {showFolderSelector && imageId && (
          <FolderSelector imageId={imageId} onClose={() => setShowFolderSelector(false)} />
        )}
      </motion.div>
    </AnimatePresence>
  );
}
