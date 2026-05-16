import { FilePlus2, PanelLeft } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { CanvasDocument } from "../../types";

interface CanvasAssetSidebarProps {
  documents: CanvasDocument[];
  selectedDocumentId: string | null;
  onSelectDocument: (documentId: string) => void;
  onCreateDocument: () => void;
}

export default function CanvasAssetSidebar({
  documents,
  selectedDocumentId,
  onSelectDocument,
  onCreateDocument,
}: CanvasAssetSidebarProps) {
  const { t } = useTranslation();

  return (
    <aside className="min-h-0 border-r border-border-subtle bg-surface/82 backdrop-blur-xl">
      <div className="border-b border-border-subtle px-4 py-4">
        <div className="flex items-center justify-between gap-3">
          <div className="min-w-0">
            <div className="flex items-center gap-2 text-[13px] font-semibold text-foreground">
              <PanelLeft size={14} strokeWidth={1.8} />
              <span>{t("canvas.assetsTitle")}</span>
            </div>
            <div className="mt-1 text-[11px] text-muted">
              {t("canvas.assetCount", { count: documents.length })}
            </div>
          </div>
          <button
            type="button"
            onClick={onCreateDocument}
            className="focus-ring flex h-8 w-8 cursor-pointer items-center justify-center rounded-[8px] border border-border-subtle bg-surface text-muted shadow-card transition-colors hover:bg-subtle hover:text-foreground"
            aria-label={t("canvas.newCanvas")}
            title={t("canvas.newCanvas")}
          >
            <FilePlus2 size={16} strokeWidth={1.8} />
          </button>
        </div>
      </div>

      <div className="flex h-[calc(100%-69px)] flex-col overflow-y-auto p-3">
        {documents.length === 0 ? (
          <div className="flex flex-1 items-center justify-center px-4 text-center text-[13px] text-muted">
            {t("canvas.assetsEmpty")}
          </div>
        ) : (
          <div className="space-y-2">
            {documents.map((document) => (
              <button
                key={document.id}
                type="button"
                onClick={() => onSelectDocument(document.id)}
                aria-label={document.name}
                className={`focus-ring flex w-full cursor-pointer items-center justify-between gap-2 rounded-[12px] border px-3 py-3 text-left text-[13px] transition-colors ${
                  selectedDocumentId === document.id
                    ? "border-primary/22 bg-primary/10 text-primary shadow-card"
                    : "border-transparent bg-surface/56 text-foreground/80 hover:border-border-subtle hover:bg-surface hover:text-foreground"
                }`}
              >
                <span className="truncate">{document.name}</span>
                <span className="shrink-0 text-[11px] text-muted">
                  {document.width}x{document.height}
                </span>
              </button>
            ))}
          </div>
        )}
      </div>
    </aside>
  );
}
