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
    <aside className="flex h-full min-h-0 flex-col border-r border-border-subtle bg-surface">
      <div className="border-b border-border-subtle px-4 py-4">
        <div className="flex items-center justify-between gap-3">
          <div className="min-w-0">
            <div className="flex items-center gap-2 text-sm font-semibold text-foreground">
              <PanelLeft size={14} strokeWidth={1.8} />
              <span>{t("canvas.assetsTitle")}</span>
            </div>
            <div className="mt-1 text-xs text-muted">
              {t("canvas.assetCount", { count: documents.length })}
            </div>
          </div>
          <button
            type="button"
            onClick={onCreateDocument}
            className="focus-ring flex h-9 w-9 cursor-pointer items-center justify-center rounded-[8px] border border-border-subtle bg-surface text-muted shadow-card transition-colors hover:bg-subtle hover:text-foreground"
            aria-label={t("canvas.newCanvas")}
            title={t("canvas.newCanvas")}
          >
            <FilePlus2 size={16} strokeWidth={1.8} />
          </button>
        </div>
      </div>

      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto p-3">
        {documents.length === 0 ? (
          <div className="flex flex-1 items-center justify-center px-4 text-center text-sm text-muted">
            {t("canvas.assetsEmpty")}
          </div>
        ) : (
          <div className="space-y-1">
            {documents.map((document) => (
              <button
                key={document.id}
                type="button"
                onClick={() => onSelectDocument(document.id)}
                aria-label={document.name}
                className={`focus-ring flex w-full cursor-pointer items-center justify-between gap-2 rounded-lg border px-3 py-2 text-left text-sm transition-colors ${
                  selectedDocumentId === document.id
                    ? "border-primary/22 bg-primary/10 text-primary shadow-card"
                    : "border-transparent bg-surface text-foreground hover:border-border-subtle hover:bg-surface hover:text-foreground"
                }`}
              >
                <span className="truncate">{document.name}</span>
                <span className="shrink-0 text-xs text-muted">
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
