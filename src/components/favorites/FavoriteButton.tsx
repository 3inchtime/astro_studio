import { Star } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "../../lib/utils";
import { useFavoriteFolders } from "../../hooks/useFavoriteFolders";

interface FavoriteButtonProps {
  imageId: string;
  className?: string;
  size?: number;
  onClick?: () => void;
  openSelector?: () => void;
}

export default function FavoriteButton({
  imageId,
  className,
  size = 16,
  onClick,
  openSelector,
}: FavoriteButtonProps) {
  const { t } = useTranslation();
  const { folderIds, loading } = useFavoriteFolders(imageId);
  const isFavorited = folderIds.length > 0;
  const action = onClick ?? openSelector;
  const icon = (
    <Star
      size={size}
      aria-hidden="true"
      className={cn(
        "transition-colors duration-[var(--motion-feedback)]",
        isFavorited ? "fill-primary text-primary" : "text-current",
        loading && "opacity-40",
      )}
    />
  );

  if (!action) {
    return (
      <span
        role="img"
        aria-label={t(
          isFavorited ? "studio.favorite.saved" : "studio.favorite.unsaved",
          {
            defaultValue: isFavorited ? "Saved to favorites" : "Not favorited",
          },
        )}
        className={cn("inline-flex items-center justify-center", className)}
      >
        {icon}
      </span>
    );
  }
  const label = t(
    isFavorited ? "favorites.manageFolders" : "favorites.addToFolder",
  );
  return (
    <button
      type="button"
      onClick={action}
      disabled={loading}
      aria-busy={loading}
      aria-pressed={isFavorited}
      aria-label={label}
      title={label}
      className={cn(
        "focus-ring inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-muted transition-colors hover:bg-subtle hover:text-foreground disabled:cursor-wait",
        className,
      )}
    >
      {icon}
    </button>
  );
}
