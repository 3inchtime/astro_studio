import { Star } from "lucide-react";
import { cn } from "../../lib/utils";
import { useFavoriteFolders } from "../../hooks/useFavoriteFolders";

interface FavoriteButtonProps {
  imageId: string;
  className?: string;
  size?: number;
  onClick?: () => void;
}

export default function FavoriteButton({ imageId, className, size = 16, onClick }: FavoriteButtonProps) {
  const { folderIds, loading } = useFavoriteFolders(imageId);
  const isFavorited = folderIds.length > 0;

  if (loading) {
    return (
      <div className={cn("h-5 w-5 rounded-full bg-subtle animate-pulse", className)} />
    );
  }

  if (onClick) {
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
        onClick={onClick}
      />
    );
  }

  return (
    <Star
      size={size}
      className={cn(
        "transition-all duration-200",
        isFavorited
          ? "fill-primary text-primary"
          : "text-muted hover:text-foreground hover:scale-110",
        className
      )}
    />
  );
}
