import { useDialogFocus } from "../../hooks/useDialogFocus";
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
  Wand2,
} from "lucide-react";
import {
  toAssetUrl,
  copyImageToClipboard,
  saveImageToFile,
} from "../../lib/api";
import { useTranslation } from "react-i18next";
import FavoriteButton from "../favorites/FavoriteButton";
import FolderSelector from "../favorites/FolderSelector";
import type { MessageImage } from "../../types";

interface LightboxProps {
  images: MessageImage[];
  initialIndex: number;
  onClose: () => void;
  onEditImage?: (image: MessageImage) => void;
  onDelete?: (generationId: string) => void;
}

const MIN_ZOOM = 0.5;
const MAX_ZOOM = 3.0;

export default function Lightbox({
  images,
  initialIndex,
  onClose,
  onEditImage,
  onDelete,
}: LightboxProps) {
  const { t } = useTranslation();
  const [selectedIndex, setIndex] = useState(initialIndex);
  const index = Math.max(0, Math.min(selectedIndex, images.length - 1));
  const [actionError, setActionError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const dialogRef = useDialogFocus({ open: images.length > 0, onClose });
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [isPanning, setIsPanning] = useState(false);
  const [showFolderSelector, setShowFolderSelector] = useState(false);
  const lastPoint = useRef({ x: 0, y: 0 });
  const containerRef = useRef<HTMLDivElement>(null);

  const currentImage = images[index];
  const currentPath = currentImage?.path;
  const imageEpoch = useRef(0);
  const transitionEase: [number, number, number, number] = [0.22, 1, 0.36, 1];

  useEffect(() => {
    setCopied(false);
    setActionError(null);
    return () => {
      imageEpoch.current += 1;
    };
  }, [currentPath, t]);

  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    setZoom((prev) => {
      const next = prev - e.deltaY * 0.001;
      return Math.max(MIN_ZOOM, Math.min(MAX_ZOOM, next));
    });
  }, []);

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      if (zoom > 1) {
        setIsPanning(true);
        lastPoint.current = { x: e.clientX, y: e.clientY };
      }
    },
    [zoom],
  );

  const handleMouseMove = useCallback(
    (e: React.MouseEvent) => {
      if (!isPanning) return;
      const dx = e.clientX - lastPoint.current.x;
      const dy = e.clientY - lastPoint.current.y;
      lastPoint.current = { x: e.clientX, y: e.clientY };
      setPan((prev) => ({ x: prev.x + dx, y: prev.y + dy }));
    },
    [isPanning],
  );

  const handleMouseUp = useCallback(() => {
    setIsPanning(false);
  }, []);

  const resetView = useCallback(() => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
  }, []);

  const goPrev = useCallback(() => {
    if (!images.length) return;
    setIndex(index > 0 ? index - 1 : images.length - 1);
    resetView();
  }, [index, images.length, resetView]);

  const goNext = useCallback(() => {
    if (!images.length) return;
    setIndex(index < images.length - 1 ? index + 1 : 0);
    resetView();
  }, [index, images.length, resetView]);

  const toggleZoom = useCallback(() => {
    if (zoom === 1) setZoom(2);
    else resetView();
  }, [zoom, resetView]);

  useEffect(() => {
    function handleKey(e: KeyboardEvent) {
      if (
        showFolderSelector ||
        e.defaultPrevented ||
        (e.target instanceof HTMLElement &&
          e.target.closest("input,textarea,select"))
      )
        return;
      if (e.key === "ArrowLeft") goPrev();
      else if (e.key === "ArrowRight") goNext();
    }
    document.addEventListener("keydown", handleKey);
    return () => document.removeEventListener("keydown", handleKey);
  }, [showFolderSelector, goPrev, goNext]);

  const handleCopy = useCallback(async () => {
    if (!currentPath) return;
    const operationEpoch = imageEpoch.current;
    try {
      setActionError(null);
      await copyImageToClipboard(currentPath);
      if (operationEpoch !== imageEpoch.current) return;
      setCopied(true);
    } catch {
      if (operationEpoch !== imageEpoch.current) return;
      setCopied(false);
      setActionError(
        t("studio.copyFailed", { defaultValue: "Could not copy. Try again." }),
      );
    }
  }, [currentPath, t]);

  const handleDownload = useCallback(async () => {
    if (!currentPath) return;
    const operationEpoch = imageEpoch.current;
    try {
      setActionError(null);
      await saveImageToFile(currentPath);
    } catch {
      if (operationEpoch !== imageEpoch.current) return;
      setActionError(
        t("studio.saveImageFailed", {
          defaultValue: "Could not save image. Try again.",
        }),
      );
    }
  }, [currentPath, t]);

  const handleDelete = useCallback(() => {
    if (currentImage?.generationId && onDelete) {
      onDelete(currentImage.generationId);
    }
  }, [currentImage, onDelete]);

  if (!currentImage || !currentPath) return null;

  return (
    <motion.div
      ref={dialogRef}
      role="dialog"
      aria-modal="true"
      aria-label={t("lightbox.preview")}
      tabIndex={-1}
      initial={{ opacity: 0, backdropFilter: "blur(0px)" }}
      animate={{ opacity: 1, backdropFilter: "blur(10px)" }}
      exit={{ opacity: 0, backdropFilter: "blur(0px)" }}
      transition={{ duration: 0.28, ease: transitionEase }}
      className="fixed inset-0 z-50 flex max-h-none flex-col bg-[#111318] text-white outline-none"
      style={{ maxHeight: "none" }}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      {/* Header */}
      <motion.div
        initial={{ opacity: 0, y: -16 }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0, y: -12 }}
        transition={{ duration: 0.3, ease: transitionEase }}
        className="relative flex items-center justify-between px-4 py-3"
      >
        <button
          onClick={onClose}
          aria-label={t("update.close")}
          className="flex h-8 w-8 items-center justify-center rounded-[8px] text-white/70 hover:bg-white/10 hover:text-white transition-colors"
        >
          <X size={18} />
        </button>
        <span className="text-[13px] text-white/80">
          {index + 1} / {images.length}
        </span>
        <div className="w-8" />
      </motion.div>

      {/* Image area */}
      <div
        ref={containerRef}
        data-testid="image-preview-viewport"
        className="relative flex flex-1 items-center justify-center overflow-hidden px-5 pb-2"
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onClick={onClose}
        style={{
          cursor: zoom > 1 ? (isPanning ? "grabbing" : "grab") : "default",
        }}
      >
        <AnimatePresence>
          <motion.div
            key={currentPath}
            initial={{ opacity: 0, scale: 0.97 }}
            animate={{ opacity: 1, scale: 1 }}
            exit={{ opacity: 0, scale: 1.02 }}
            transition={{ duration: 0.3, ease: transitionEase }}
            className="absolute inset-0 flex items-center justify-center"
          >
            <img
              src={toAssetUrl(currentPath)}
              alt={t("lightbox.preview")}
              className="max-h-full max-w-full origin-center object-contain select-none will-change-transform"
              style={{
                transform: `scale(${zoom}) translate(${pan.x / zoom}px, ${pan.y / zoom}px)`,
              }}
              onDoubleClick={toggleZoom}
              onClick={(e) => e.stopPropagation()}
              draggable={false}
            />
          </motion.div>
        </AnimatePresence>
      </div>

      {/* Navigation arrows */}
      {images.length > 1 && (
        <>
          <motion.button
            initial={{ opacity: 0, x: -12 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: -12 }}
            transition={{ duration: 0.28, ease: transitionEase }}
            onClick={goPrev}
            aria-label={t("studio.previousImage", {
              defaultValue: "Previous image",
            })}
            className="absolute left-4 top-1/2 -translate-y-1/2 flex h-10 w-10 items-center justify-center rounded-full bg-black/40 text-white/70 hover:bg-black/60 hover:text-white transition-colors"
          >
            <ChevronLeft size={20} />
          </motion.button>
          <motion.button
            initial={{ opacity: 0, x: 12 }}
            animate={{ opacity: 1, x: 0 }}
            exit={{ opacity: 0, x: 12 }}
            transition={{ duration: 0.28, ease: transitionEase }}
            onClick={goNext}
            aria-label={t("studio.nextImage", { defaultValue: "Next image" })}
            className="absolute right-4 top-1/2 -translate-y-1/2 flex h-10 w-10 items-center justify-center rounded-full bg-black/40 text-white/70 hover:bg-black/60 hover:text-white transition-colors"
          >
            <ChevronRight size={20} />
          </motion.button>
        </>
      )}

      {actionError && (
        <p role="alert" className="px-5 text-center text-sm text-red-200">
          {actionError}
        </p>
      )}
      {copied && !actionError && (
        <p role="status" className="px-5 text-center text-sm text-white">
          {t("studio.copied", { defaultValue: "Copied" })}
        </p>
      )}
      {/* Toolbar */}
      <motion.div
        initial={{ opacity: 0, y: 18 }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0, y: 14 }}
        transition={{ duration: 0.32, ease: transitionEase }}
        className="relative flex items-center justify-center gap-1 px-4 py-3"
      >
        {[
          {
            icon: ZoomIn,
            label: t("lightbox.zoomIn"),
            onClick: () => setZoom((z) => Math.min(MAX_ZOOM, z + 0.25)),
          },
          {
            icon: ZoomOut,
            label: t("lightbox.zoomOut"),
            onClick: () => setZoom((z) => Math.max(MIN_ZOOM, z - 0.25)),
          },
          { icon: RotateCcw, label: t("lightbox.reset"), onClick: resetView },
          { icon: Copy, label: t("lightbox.copy"), onClick: handleCopy },
          {
            icon: Download,
            label: t("lightbox.download"),
            onClick: handleDownload,
          },
          ...(onEditImage
            ? [
                {
                  icon: Wand2,
                  label: t("lightbox.edit"),
                  onClick: () => onEditImage(currentImage),
                },
              ]
            : []),
          ...(onDelete
            ? [
                {
                  icon: Trash2,
                  label: t("lightbox.delete"),
                  onClick: handleDelete,
                },
              ]
            : []),
        ].map(({ icon: Icon, label, onClick }) => (
          <button
            key={label}
            onClick={onClick}
            title={label}
            aria-label={label}
            className="flex h-9 w-9 items-center justify-center rounded-[8px] text-white/80 hover:bg-white/10 hover:text-white transition-colors"
          >
            <Icon size={16} strokeWidth={1.8} />
          </button>
        ))}
        {currentImage?.imageId && (
          <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-white/90">
            <FavoriteButton
              imageId={currentImage.imageId}
              size={16}
              onClick={() => setShowFolderSelector(true)}
            />
          </div>
        )}
      </motion.div>
      {showFolderSelector && currentImage?.imageId && (
        <FolderSelector
          imageId={currentImage.imageId}
          onClose={() => setShowFolderSelector(false)}
        />
      )}
    </motion.div>
  );
}
