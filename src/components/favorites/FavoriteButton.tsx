import { Star } from "lucide-react";
import { cn } from "../../lib/utils";
import { useFavoriteFolders } from "../../hooks/useFavoriteFolders";

interface FavoriteButtonProps {
  imageId: string;
  className?: string;
  size?: number;
  /** If provided, clicking the star calls this. Otherwise clicking opens the FolderSelector. */
  onClick?: () => void;
  /** If true, clicking the star opens the folder selector directly (used when onClick is not provided) */
  openSelector?: () => void;
}

export default function FavoriteButton({ imageId, className, size = 16, onClick, openSelector }: FavoriteButtonProps) {
  const { folderIds, loading } = useFavoriteFolders(imageId);
  const isFavorited = folderIds.length > 0;

  const handleClick = () => {
    if (onClick) {
      onClick();
    } else if (openSelector) {
      openSelector();
    }
  };

  if (loading) {
    return (
      <div className={cn("h-5 w-5 rounded-full bg-subtle animate-pulse", className)} />
    );
  }

  return (
    <Star
      size={size}
      className={cn(
        "cursor-pointer transition-all duration-200",
        isFavorited
          ? "fill-primary text-primary"
          : "text-muted hover:text-foreground hover:scale-110",
        className
      )}
      onClick={handleClick}
    />
  );
}
