import { useTranslation } from "react-i18next";

interface PaginationControlsProps {
  page: number;
  totalPages: number;
  onPageChange: (page: number) => void;
}

export default function PaginationControls({ page, totalPages, onPageChange }: PaginationControlsProps) {
  const { t } = useTranslation();

  if (totalPages <= 1) return null;

  return (
    <div className="mt-6 flex items-center justify-center gap-2">
      <button
        onClick={() => onPageChange(page - 1)}
        disabled={page <= 1}
        className="studio-control focus-ring h-[28px] rounded-[8px] px-3 text-[11px] hover:studio-control-hover disabled:opacity-30"
      >
        {t("gallery.prev")}
      </button>
      <span className="px-2 text-[11px] text-muted tabular-nums">
        {page} / {totalPages}
      </span>
      <button
        onClick={() => onPageChange(page + 1)}
        disabled={page >= totalPages}
        className="studio-control focus-ring h-[28px] rounded-[8px] px-3 text-[11px] hover:studio-control-hover disabled:opacity-30"
      >
        {t("gallery.next")}
      </button>
    </div>
  );
}
