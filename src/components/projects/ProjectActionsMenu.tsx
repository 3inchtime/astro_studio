import { Archive, Pencil, Pin, PinOff, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";

interface ProjectActionsMenuProps {
  open: boolean;
  pinned: boolean;
  disabled?: boolean;
  onRename: () => void;
  onPin: () => void;
  onUnpin: () => void;
  onArchive: () => void;
  onDelete: () => void;
}

export default function ProjectActionsMenu({
  open,
  pinned,
  disabled = false,
  onRename,
  onPin,
  onUnpin,
  onArchive,
  onDelete,
}: ProjectActionsMenuProps) {
  const { t } = useTranslation();

  if (!open) {
    return null;
  }

  const actionClass =
    "flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] font-medium text-foreground transition-colors hover:bg-subtle";
  const dangerClass =
    "flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] font-medium text-error transition-colors hover:bg-error/8";

  return (
    <div className="absolute right-0 top-[44px] z-10 w-44 overflow-hidden rounded-[10px] border border-border-subtle bg-surface py-1 shadow-card">
      <button type="button" className={actionClass} onClick={onRename} disabled={disabled}>
        <Pencil size={14} aria-hidden="true" />
        <span>{t("sidebar.rename")}</span>
      </button>
      {pinned ? (
        <button type="button" className={actionClass} onClick={onUnpin} disabled={disabled}>
          <PinOff size={14} aria-hidden="true" />
          <span>{t("projects.unpin")}</span>
        </button>
      ) : (
        <button type="button" className={actionClass} onClick={onPin} disabled={disabled}>
          <Pin size={14} aria-hidden="true" />
          <span>{t("projects.pin")}</span>
        </button>
      )}
      <button type="button" className={actionClass} onClick={onArchive} disabled={disabled}>
        <Archive size={14} aria-hidden="true" />
        <span>{t("sidebar.archive")}</span>
      </button>
      <button type="button" className={dangerClass} onClick={onDelete} disabled={disabled}>
        <Trash2 size={14} aria-hidden="true" />
        <span>{t("sidebar.delete")}</span>
      </button>
    </div>
  );
}
