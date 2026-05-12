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
  onPreview?: (imageIndex: number) => void;
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
  onPreview,
  onManageFolders,
  onRestore,
  deleteLabel,
  restoreLabel,
  deletedAtLabel,
}: GenerationDetailPanelProps) {
  const { t } = useTranslation();
  const [selectedIndex, setSelectedIndex] = useState(0);
  const image = result.images[selectedIndex] ?? result.images[0];
  const generation = result.generation;

  useEffect(() => {
    setSelectedIndex(0);
  }, [result.generation.id]);

  return (
    <motion.div
      initial={{ x: 340, opacity: 0 }}
      animate={{ x: 0, opacity: 1 }}
      exit={{ x: 340, opacity: 0 }}
      transition={{ duration: 0.25, ease: [0.22, 1, 0.36, 1] }}
      className="studio-panel-strong h-full w-[340px] shrink-0 overflow-y-auto overflow-hidden rounded-l-[16px] border-y-0 border-r-0"
    >
      <div className="p-5">
        <div className="mb-4 flex items-center justify-between">
          <h3 className="text-[13px] font-semibold text-foreground tracking-tight">
            {title}
          </h3>
          <button
            onClick={onClose}
            className="focus-ring flex h-6 w-6 cursor-pointer items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-subtle"
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
              {onPreview ? (
                <button
                  type="button"
                  onClick={() => onPreview(selectedIndex)}
                  aria-label={`Preview ${result.generation.prompt}`}
                  className="focus-ring block w-full cursor-zoom-in text-left"
                >
                  <img
                    src={toAssetUrl(image.file_path)}
                    alt={result.generation.prompt}
                    className="w-full"
                  />
                </button>
              ) : (
                <img
                  src={toAssetUrl(image.file_path)}
                  alt={result.generation.prompt}
                  className="w-full"
                />
              )}
            </motion.div>

            {result.images.length > 1 && (
              <div className="mb-4 grid grid-cols-4 gap-2">
                {result.images.map((item, index) => (
                  <button
                    key={item.id}
                    onClick={() => setSelectedIndex(index)}
                    aria-label={`${t("lightbox.preview")} ${index + 1}`}
                    className={`focus-ring overflow-hidden rounded-[10px] border transition-all ${
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
              {generation.prompt}
            </p>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <MetaRow label={t("gallery.model")} value={generation.engine} />
            <MetaRow label={t("gallery.requestKind")} value={generation.request_kind} />
            <MetaRow label={t("gallery.size")} value={generation.size} />
            <MetaRow label={t("gallery.quality")} value={generation.quality} />
            <MetaRow label={t("gallery.background")} value={generation.background} />
            <MetaRow label={t("gallery.format")} value={generation.output_format} />
            <MetaRow label={t("gallery.moderation")} value={generation.moderation} />
            <MetaRow label={t("gallery.fidelity")} value={generation.input_fidelity} />
            <MetaRow
              label={t("gallery.imageCount")}
              value={String(generation.image_count)}
            />
            <MetaRow
              label={t("gallery.sourceCount")}
              value={String(generation.source_image_count)}
            />
            <MetaRow
              label={t("gallery.compression")}
              value={String(generation.output_compression)}
            />
            <MetaRow
              label={t("gallery.status")}
              value={generation.status}
            />
          </div>
          <div className="flex items-center gap-1.5">
            <Calendar size={11} className="text-muted/40" />
            <span className="text-[11px] text-muted/60">
              {formatLocalDateTime(generation.created_at)}
            </span>
          </div>
          {generation.deleted_at && (
            <div className="flex items-center gap-1.5">
              <Trash2 size={11} className="text-muted/40" />
              <span className="text-[11px] text-muted/60">
                {deletedAtLabel || t("trash.deletedAt")}:{" "}
                {formatLocalDateTime(generation.deleted_at)}
              </span>
            </div>
          )}
          {generation.source_image_paths.length > 0 && (
            <div className="rounded-[12px] border border-border-subtle bg-subtle/25 p-3">
              <span className="text-[10px] font-medium uppercase tracking-wider text-muted/50">
                {t("gallery.sourceImages")}
              </span>
              <div className="mt-2 space-y-1.5">
                {generation.source_image_paths.map((path) => (
                  <p
                    key={path}
                    className="truncate text-[11px] text-foreground/70"
                    title={path}
                  >
                    {path}
                  </p>
                ))}
              </div>
            </div>
          )}
        </div>

        <div className="mt-6 space-y-2">
          {image && onEditImage && (
            <button
              onClick={() =>
                onEditImage(image.file_path, image.id, generation.id)
              }
              className="focus-ring flex w-full cursor-pointer items-center justify-center gap-2 rounded-[10px] border border-primary/20 py-2.5 text-[12px] font-medium text-primary transition-all hover:border-primary/30 hover:bg-primary/6"
            >
              <Wand2 size={13} />
              {t("gallery.editImage")}
            </button>
          )}
          {showManageFolders && image && (
            <button
              onClick={() => onManageFolders?.(image.id)}
              className="studio-control focus-ring flex w-full items-center justify-center gap-2 rounded-[10px] py-2.5 text-[12px] font-medium hover:studio-control-hover"
            >
              {t("favorites.manageFolders")}
            </button>
          )}
          {showSaveButton && image && (
            <button
              onClick={() => void saveImageToFile(image.file_path)}
              className="studio-control focus-ring flex w-full items-center justify-center gap-2 rounded-[10px] py-2.5 text-[12px] font-medium hover:studio-control-hover"
            >
              <Download size={13} />
              {t("gallery.saveImage")}
            </button>
          )}
          {onRestore && (
            <button
              onClick={() => onRestore(generation.id)}
              className="studio-control focus-ring flex w-full items-center justify-center gap-2 rounded-[10px] py-2.5 text-[12px] font-medium hover:studio-control-hover"
            >
              <RotateCcw size={13} />
              {restoreLabel || t("trash.restore")}
            </button>
          )}
          <button
            onClick={() => onDelete(generation.id)}
            className="studio-control-danger focus-ring flex w-full items-center justify-center gap-2 rounded-[10px] py-2.5 text-[12px] font-medium"
          >
            <Trash2 size={13} />
            {deleteLabel || t("gallery.delete")}
          </button>
        </div>
      </div>
    </motion.div>
  );
}

function MetaRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-[10px] border border-border-subtle bg-subtle/20 px-3 py-2">
      <span className="text-[10px] font-medium uppercase tracking-wider text-muted/45">
        {label}
      </span>
      <p className="mt-0.5 truncate text-[12px] text-foreground/80">{value}</p>
    </div>
  );
}
