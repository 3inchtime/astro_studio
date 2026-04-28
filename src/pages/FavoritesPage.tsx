import { useEffect, useState, useCallback } from "react";
import { AnimatePresence } from "framer-motion";
import {
  Copy,
  Folder,
  Image as ImageIcon,
  MessageSquareText,
  Search,
  Trash2,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  deleteGeneration,
  deletePromptFavorite,
  getFavoriteImages,
  getPromptFavorites,
} from "../lib/api";
import { formatLocalDateTime } from "../lib/utils";
import { useFolders } from "../hooks/useFolders";
import { usePromptFolders } from "../hooks/usePromptFolders";
import type { GenerationResult, PromptFavorite } from "../types";
import FolderSelector from "../components/favorites/FolderSelector";
import PromptFolderSelector from "../components/favorites/PromptFolderSelector";
import EmptyCollectionState from "../components/gallery/EmptyCollectionState";
import GenerationDetailPanel from "../components/gallery/GenerationDetailPanel";
import GenerationGrid from "../components/gallery/GenerationGrid";
import PaginationControls from "../components/gallery/PaginationControls";

type FavoriteKind = "images" | "prompts";

export default function FavoritesPage() {
  const { t } = useTranslation();
  const { folders, reload: reloadFolders } = useFolders();
  const { folders: promptFolders, reload: reloadPromptFolders } =
    usePromptFolders();
  const [activeKind, setActiveKind] = useState<FavoriteKind>("images");

  const [imageResults, setImageResults] = useState<GenerationResult[]>([]);
  const [imageQuery, setImageQuery] = useState("");
  const [selectedImageFolderId, setSelectedImageFolderId] = useState("");
  const [imageTotal, setImageTotal] = useState(0);
  const [imagePage, setImagePage] = useState(1);
  const [imagePageSize] = useState(20);
  const [selectedImage, setSelectedImage] = useState<GenerationResult | null>(
    null,
  );
  const [folderSelectorImageId, setFolderSelectorImageId] = useState<
    string | null
  >(null);

  const [promptFavorites, setPromptFavorites] = useState<PromptFavorite[]>([]);
  const [promptQuery, setPromptQuery] = useState("");
  const [selectedPromptFolderId, setSelectedPromptFolderId] = useState("");
  const [promptFolderSelectorFavoriteId, setPromptFolderSelectorFavoriteId] =
    useState<string | null>(null);
  const [copiedPromptId, setCopiedPromptId] = useState<string | null>(null);

  const loadImageFavorites = useCallback(
    async (page: number, query?: string, folderId?: string) => {
      const result = await getFavoriteImages(
        folderId || selectedImageFolderId || undefined,
        query?.trim() || imageQuery.trim() || undefined,
        page,
      );
      setImageResults(result.generations);
      setImageTotal(result.total);
      setImagePage(result.page);
    },
    [imageQuery, selectedImageFolderId],
  );

  const loadPromptFavorites = useCallback(
    async (query?: string, folderId?: string) => {
      const favorites = await getPromptFavorites(
        query?.trim() || promptQuery.trim() || undefined,
        folderId || selectedPromptFolderId || undefined,
      );
      setPromptFavorites(favorites);
    },
    [promptQuery, selectedPromptFolderId],
  );

  useEffect(() => {
    loadImageFavorites(1).catch(() => {});
    loadPromptFavorites().catch(() => {});
  }, []);

  function handleSearch() {
    if (activeKind === "images") {
      loadImageFavorites(1, imageQuery, selectedImageFolderId).catch(() => {});
      return;
    }
    loadPromptFavorites(promptQuery, selectedPromptFolderId).catch(() => {});
  }

  function handleFolderFilterChange(folderId: string) {
    if (activeKind === "images") {
      setSelectedImageFolderId(folderId);
      setSelectedImage(null);
      loadImageFavorites(1, imageQuery, folderId).catch(() => {});
      return;
    }

    setSelectedPromptFolderId(folderId);
    loadPromptFavorites(promptQuery, folderId).catch(() => {});
  }

  async function handleDeleteImage(id: string) {
    await deleteGeneration(id);
    await loadImageFavorites(imagePage, imageQuery, selectedImageFolderId);
    if (selectedImage?.generation.id === id) setSelectedImage(null);
  }

  async function handleDeletePromptFavorite(id: string) {
    await deletePromptFavorite(id);
    setPromptFavorites((current) =>
      current.filter((favorite) => favorite.id !== id),
    );
  }

  async function handleCopyPrompt(favorite: PromptFavorite) {
    await navigator.clipboard.writeText(favorite.prompt).catch(() => {});
    setCopiedPromptId(favorite.id);
    window.setTimeout(() => {
      setCopiedPromptId((current) =>
        current === favorite.id ? null : current,
      );
    }, 1200);
  }

  function handleImageFolderSelectorClose() {
    setFolderSelectorImageId(null);
    reloadFolders();
    loadImageFavorites(imagePage, imageQuery, selectedImageFolderId).catch(
      () => {},
    );
  }

  function handlePromptFolderSelectorClose() {
    setPromptFolderSelectorFavoriteId(null);
    reloadPromptFolders();
    loadPromptFavorites(promptQuery, selectedPromptFolderId).catch(() => {});
  }

  const activeTotal =
    activeKind === "images" ? imageTotal : promptFavorites.length;
  const activeFolderId =
    activeKind === "images" ? selectedImageFolderId : selectedPromptFolderId;
  const activeFolders = activeKind === "images" ? folders : promptFolders;
  const activeQuery = activeKind === "images" ? imageQuery : promptQuery;
  const imageTotalPages = Math.ceil(imageTotal / imagePageSize);

  return (
    <div className="flex h-full">
      <div className="flex flex-1 flex-col">
        <div className="flex flex-col gap-3 border-b border-border-subtle px-6 py-4 xl:flex-row xl:items-center xl:justify-between">
          <div className="flex min-w-0 flex-wrap items-center gap-3">
            <h2 className="text-[15px] font-semibold text-foreground tracking-tight">
              {t("favorites.title")}
            </h2>
            {activeTotal > 0 && (
              <span className="rounded-[6px] bg-subtle px-2 py-0.5 text-[10px] font-medium text-muted tabular-nums">
                {activeTotal}
              </span>
            )}
            <div className="flex rounded-[8px] border border-border-subtle bg-subtle/40 p-0.5">
              <button
                onClick={() => setActiveKind("images")}
                className={`flex h-7 items-center gap-1.5 rounded-[6px] px-2.5 text-[12px] font-medium transition-colors ${
                  activeKind === "images"
                    ? "bg-surface text-foreground shadow-sm"
                    : "text-muted hover:text-foreground"
                }`}
              >
                <ImageIcon size={13} />
                {t("favorites.imagesTab")}
              </button>
              <button
                onClick={() => setActiveKind("prompts")}
                className={`flex h-7 items-center gap-1.5 rounded-[6px] px-2.5 text-[12px] font-medium transition-colors ${
                  activeKind === "prompts"
                    ? "bg-surface text-foreground shadow-sm"
                    : "text-muted hover:text-foreground"
                }`}
              >
                <MessageSquareText size={13} />
                {t("favorites.promptsTab")}
              </button>
            </div>
          </div>

          <div className="flex w-full min-w-0 items-center gap-2 xl:w-auto">
            <div className="relative min-w-0 flex-1 xl:flex-none">
              <Folder
                size={13}
                className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted/60"
                strokeWidth={2}
              />
              <select
                value={activeFolderId}
                onChange={(event) =>
                  handleFolderFilterChange(event.target.value)
                }
                className="h-[30px] w-full appearance-none rounded-[8px] border border-border-subtle bg-subtle/40 pl-7 pr-7 text-[12px] text-foreground transition-colors focus:border-border focus:bg-surface focus:outline-none xl:w-40"
                title={t("favorites.folderFilter")}
                aria-label={t("favorites.folderFilter")}
              >
                <option value="">
                  {activeKind === "images"
                    ? t("favorites.allFolders")
                    : t("favorites.allPromptFolders")}
                </option>
                {activeFolders.map((folder) => (
                  <option key={folder.id} value={folder.id}>
                    {folder.name}
                  </option>
                ))}
              </select>
            </div>

            <div className="relative min-w-0 flex-1 xl:flex-none">
              <Search
                size={13}
                className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted/60"
                strokeWidth={2}
              />
              <input
                value={activeQuery}
                onChange={(event) =>
                  activeKind === "images"
                    ? setImageQuery(event.target.value)
                    : setPromptQuery(event.target.value)
                }
                onKeyDown={(event) => {
                  if (event.key === "Enter") handleSearch();
                }}
                placeholder={
                  activeKind === "images"
                    ? t("favorites.search")
                    : t("favorites.searchPrompts")
                }
                className="h-[30px] w-full rounded-[8px] border border-border-subtle bg-subtle/40 pl-7 pr-3 text-[12px] text-foreground transition-colors placeholder:text-muted/50 focus:border-border focus:bg-surface focus:outline-none xl:w-52"
              />
            </div>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto p-5">
          {activeKind === "images" ? (
            <>
              {imageResults.length === 0 ? (
                <EmptyCollectionState
                  title={t("favorites.noImages")}
                  subtitle={t("favorites.emptyHint")}
                />
              ) : (
                <GenerationGrid
                  results={imageResults}
                  onSelect={setSelectedImage}
                />
              )}

              <PaginationControls
                page={imagePage}
                totalPages={imageTotalPages}
                onPageChange={(page) =>
                  loadImageFavorites(
                    page,
                    imageQuery,
                    selectedImageFolderId,
                  )
                }
              />
            </>
          ) : (
            <PromptFavoritesList
              favorites={promptFavorites}
              copiedPromptId={copiedPromptId}
              onCopy={(favorite) => void handleCopyPrompt(favorite)}
              onManageFolders={setPromptFolderSelectorFavoriteId}
              onDelete={(id) => void handleDeletePromptFavorite(id)}
            />
          )}
        </div>
      </div>

      <AnimatePresence>
        {selectedImage && (
          <GenerationDetailPanel
            result={selectedImage}
            title={t("favorites.detail")}
            onClose={() => setSelectedImage(null)}
            onDelete={(id) => void handleDeleteImage(id)}
            onManageFolders={setFolderSelectorImageId}
          />
        )}
      </AnimatePresence>

      {folderSelectorImageId && (
        <FolderSelector
          imageId={folderSelectorImageId}
          onClose={handleImageFolderSelectorClose}
        />
      )}

      {promptFolderSelectorFavoriteId && (
        <PromptFolderSelector
          favoriteId={promptFolderSelectorFavoriteId}
          onClose={handlePromptFolderSelectorClose}
        />
      )}
    </div>
  );
}

interface PromptFavoritesListProps {
  favorites: PromptFavorite[];
  copiedPromptId: string | null;
  onCopy: (favorite: PromptFavorite) => void;
  onManageFolders: (favoriteId: string) => void;
  onDelete: (favoriteId: string) => void;
}

function PromptFavoritesList({
  favorites,
  copiedPromptId,
  onCopy,
  onManageFolders,
  onDelete,
}: PromptFavoritesListProps) {
  const { t } = useTranslation();

  if (favorites.length === 0) {
    return (
      <EmptyCollectionState
        title={t("favorites.noPrompts")}
        subtitle={t("favorites.promptEmptyHint")}
      />
    );
  }

  return (
    <div className="grid gap-3 xl:grid-cols-2">
      {favorites.map((favorite) => (
        <article
          key={favorite.id}
          className="rounded-[8px] border border-border-subtle bg-surface p-4 shadow-sm"
        >
          <p className="max-h-36 overflow-hidden whitespace-pre-wrap break-words text-[13px] leading-relaxed text-foreground/85">
            {favorite.prompt}
          </p>

          <div className="mt-4 flex items-center justify-between gap-3">
            <span className="min-w-0 truncate text-[11px] text-muted/60">
              {formatLocalDateTime(favorite.updated_at)}
            </span>

            <div className="flex shrink-0 items-center gap-1">
              <button
                onClick={() => onCopy(favorite)}
                className="flex h-8 items-center gap-1.5 rounded-[8px] px-2 text-[12px] font-medium text-muted transition-colors hover:bg-subtle hover:text-foreground"
              >
                <Copy size={13} />
                {copiedPromptId === favorite.id
                  ? t("favorites.copiedPrompt")
                  : t("favorites.copyPrompt")}
              </button>
              <button
                onClick={() => onManageFolders(favorite.id)}
                className="flex h-8 items-center gap-1.5 rounded-[8px] px-2 text-[12px] font-medium text-muted transition-colors hover:bg-subtle hover:text-foreground"
              >
                <Folder size={13} />
                {t("favorites.manageFolders")}
              </button>
              <button
                onClick={() => onDelete(favorite.id)}
                className="flex h-8 w-8 items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-error/6 hover:text-error"
                aria-label={t("favorites.deletePrompt")}
                title={t("favorites.deletePrompt")}
              >
                <Trash2 size={13} />
              </button>
            </div>
          </div>
        </article>
      ))}
    </div>
  );
}
