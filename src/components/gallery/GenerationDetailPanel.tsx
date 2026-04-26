import { motion } from "framer-motion";
import { useEffect, useState } from "react";
import { Calendar, Download, RotateCcw, Trash2, Wand2, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { saveImageToFile, toAssetUrl } from "../../lib/api";
import { formatLocalDateTime } from "../../lib/utils";
import type { GenerationResult } from "../../types";

interface GenerationDetailPanelProps {
  result: GenerationResult;
  title: string;
  showSaveButton?: boolean;
  showManageFolders?: boolean;
  onClose: () => void;
  onDelete: (generationId: string) => void;
  onEditImage?: (
    imagePath: string,
    imageId: string,
    generationId: string,
  ) => void;
  onManageFolders?: (imageId: string) => void;
  onRestore?: (generationId: string) => void;
  deleteLabel?: string;
  restoreLabel?: string;
  deletedAtLabel?: string;
}

export default function GenerationDetailPanel({
  result,
  title,
  showSaveButton = false,
  showManageFolders = true,
  onClose,
  onDelete,
  onEditImage,
  onManageFolders,
  onRestore,
  deleteLabel,
  restoreLabel,
  deletedAtLabel,
}: GenerationDetailPanelProps) {
  const { t } = useTranslation();
  const [selectedIndex, setSelectedIndex] = useState(0);
  const image = result.images[selectedIndex] ?? result.images[0];

  useEffect(() => {
    setSelectedIndex(0);
  }, [result.generation.id]);

  return (
    <motion.div
      initial={{ width: 0, opacity: 0 }}
      animate={{ width: 340, opacity: 1 }}
      exit={{ width: 0, opacity: 0 }}
      transition={{ duration: 0.25, ease: [0.22, 1, 0.36, 1] }}
      className="w-[340px] h-full overflow-y-auto overflow-hidden border-l border-border-subtle bg-surface"
    >
      <div className="p-5">
        <div className="mb-4 flex items-center justify-between">
          <h3 className="text-[13px] font-semibold text-foreground tracking-tight">
            {title}
          </h3>
          <button
            onClick={onClose}
            className="flex h-6 w-6 items-center justify-center rounded-[8px] text-muted hover:bg-subtle transition-colors"
          >
            <X size={14} />
          </button>
        </div>

        {image && (
          <>
            <motion.div
              initial={{ opacity: 0, scale: 0.96 }}
              animate={{ opacity: 1, scale: 1 }}
              transition={{ duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
              className="mb-3 overflow-hidden rounded-[12px] border border-border-subtle"
            >
              <img
                src={toAssetUrl(image.file_path)}
                alt={result.generation.prompt}
                className="w-full"
              />
            </motion.div>

            {result.images.length > 1 && (
              <div className="mb-4 grid grid-cols-4 gap-2">
                {result.images.map((item, index) => (
                  <button
                    key={item.id}
                    onClick={() => setSelectedIndex(index)}
                    className={`overflow-hidden rounded-[10px] border transition-all ${
                      index === selectedIndex
                        ? "border-primary shadow-card"
                        : "border-border-subtle hover:border-border"
                    }`}
                  >
                    <img
                      src={toAssetUrl(item.thumbnail_path || item.file_path)}
                      alt=""
                      className="aspect-square w-full object-cover"
                    />
                  </button>
                ))}
              </div>
            )}
          </>
        )}

        <div className="space-y-3">
          <div>
            <span className="text-[10px] font-medium uppercase tracking-wider text-muted/50">
              {t("gallery.prompt")}
            </span>
            <p className="mt-1 text-[13px] leading-relaxed text-foreground/80">
              {result.generation.prompt}
            </p>
          </div>
          <div className="flex gap-5">
            <div>
              <span className="text-[10px] font-medium uppercase tracking-wider text-muted/50">
                {t("gallery.size")}
              </span>
              <p className="mt-0.5 text-[13px] text-foreground/80">
                {result.generation.size}
              </p>
            </div>
            <div>
              <span className="text-[10px] font-medium uppercase tracking-wider text-muted/50">
                {t("gallery.quality")}
              </span>
              <p className="mt-0.5 text-[13px] text-foreground/80">
                {result.generation.quality}
              </p>
            </div>
          </div>
          <div className="flex items-center gap-1.5">
            <Calendar size={11} className="text-muted/40" />
            <span className="text-[11px] text-muted/60">
              {formatLocalDateTime(result.generation.created_at)}
            </span>
          </div>
          {result.generation.deleted_at && (
            <div className="flex items-center gap-1.5">
              <Trash2 size={11} className="text-muted/40" />
              <span className="text-[11px] text-muted/60">
                {deletedAtLabel || t("trash.deletedAt")}:{" "}
                {formatLocalDateTime(result.generation.deleted_at)}
              </span>
            </div>
          )}
        </div>

        <div className="mt-6 space-y-2">
          {image && onEditImage && (
            <button
              onClick={() =>
                onEditImage(image.file_path, image.id, result.generation.id)
              }
              className="flex w-full items-center justify-center gap-2 rounded-[10px] border border-primary/20 py-2.5 text-[12px] font-medium text-primary transition-all hover:border-primary/30 hover:bg-primary/6"
            >
              <Wand2 size={13} />
              {t("gallery.editImage")}
            </button>
          )}
          {showManageFolders && image && (
            <button
              onClick={() => onManageFolders?.(image.id)}
              className="flex w-full items-center justify-center gap-2 rounded-[10px] border border-border-subtle py-2.5 text-[12px] font-medium text-foreground/70 transition-all hover:border-border hover:bg-subtle hover:text-foreground"
            >
              {t("favorites.manageFolders")}
            </button>
          )}
          {showSaveButton && image && (
            <button
              onClick={() => void saveImageToFile(image.file_path)}
              className="flex w-full items-center justify-center gap-2 rounded-[10px] border border-border-subtle py-2.5 text-[12px] font-medium text-foreground/70 transition-all hover:border-border hover:bg-subtle hover:text-foreground"
            >
              <Download size={13} />
              {t("gallery.saveImage")}
            </button>
          )}
          {onRestore && (
            <button
              onClick={() => onRestore(result.generation.id)}
              className="flex w-full items-center justify-center gap-2 rounded-[10px] border border-border-subtle py-2.5 text-[12px] font-medium text-foreground/70 transition-all hover:border-border hover:bg-subtle hover:text-foreground"
            >
              <RotateCcw size={13} />
              {restoreLabel || t("trash.restore")}
            </button>
          )}
          <button
            onClick={() => onDelete(result.generation.id)}
            className="flex w-full items-center justify-center gap-2 rounded-[10px] border border-error/10 py-2.5 text-[12px] font-medium text-error/60 transition-all hover:border-error/20 hover:bg-error/4 hover:text-error"
          >
            <Trash2 size={13} />
            {deleteLabel || t("gallery.delete")}
          </button>
        </div>
      </div>
    </motion.div>
  );
}
