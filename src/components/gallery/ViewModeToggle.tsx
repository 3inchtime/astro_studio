import { LayoutGrid, List } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { GalleryViewMode } from "../../lib/galleryViewMode";
import { cn } from "../../lib/utils";

interface ViewModeToggleProps {
  value: GalleryViewMode;
  onChange: (mode: GalleryViewMode) => void;
}

export default function ViewModeToggle({
  value,
  onChange,
}: ViewModeToggleProps) {
  const { t } = useTranslation();

  const options: Array<{
    value: GalleryViewMode;
    label: string;
    icon: typeof LayoutGrid;
  }> = [
    {
      value: "masonry",
      label: t("gallery.viewModeMasonry"),
      icon: LayoutGrid,
    },
    {
      value: "list",
      label: t("gallery.viewModeList"),
      icon: List,
    },
  ];

  return (
    <div
      aria-label={t("gallery.viewMode")}
      className="studio-control flex h-[34px] shrink-0 items-center rounded-[10px] p-0.5"
      role="group"
    >
      {options.map((option) => {
        const Icon = option.icon;
        const active = value === option.value;

        return (
          <button
            key={option.value}
            type="button"
            aria-label={option.label}
            aria-pressed={active}
            title={option.label}
            onClick={() => onChange(option.value)}
            className={cn(
              "focus-ring flex h-8 w-8 cursor-pointer items-center justify-center rounded-[8px] text-muted transition-all hover:text-foreground",
              active &&
                "bg-surface text-foreground shadow-[0_4px_14px_rgba(15,23,42,0.08)]",
            )}
          >
            <Icon size={14} strokeWidth={2} />
          </button>
        );
      })}
    </div>
  );
}
