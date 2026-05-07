import { useEffect, useState, useCallback } from "react";
import { AnimatePresence } from "framer-motion";
import { useNavigate } from "react-router-dom";
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
import { savePendingEditSources } from "../lib/editSources";
import { getPromptFolderDisplayName } from "../lib/promptFolders";
import { cn, formatLocalDateTime } from "../lib/utils";
import { useFolders } from "../hooks/useFolders";
import { usePromptFolders } from "../hooks/usePromptFolders";
import { useLayoutContext } from "../components/layout/AppLayout";
import type { GenerationResult, MessageImage, PromptFavorite } from "../types";
import FolderSelector from "../components/favorites/FolderSelector";
import PromptFolderSelector from "../components/favorites/PromptFolderSelector";
import EmptyCollectionState from "../components/gallery/EmptyCollectionState";
import GenerationDetailPanel from "../components/gallery/GenerationDetailPanel";
import GenerationGrid from "../components/gallery/GenerationGrid";
import Lightbox from "../components/lightbox/Lightbox";
import { generationResultToLightboxImages } from "../lib/lightboxImages";
import { useInfiniteScroll } from "../hooks/useInfiniteScroll";

type FavoriteKind = "images" | "prompts";

export default function FavoritesPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { setActiveConversationId } = useLayoutContext();
  const { folders, reload: reloadFolders } = useFolders();
  const { folders: promptFolders, reload: reloadPromptFolders } =
    usePromptFolders();
  const [activeKind, setActiveKind] = useState<FavoriteKind>("images");

  const [imageResults, setImageResults] = useState<GenerationResult[]>([]);
  const [imageQuery, setImageQuery] = useState("");
  const [selectedImageFolderId, setSelectedImageFolderId] = useState("");
  const [imageTotal, setImageTotal] = useState(0);
  const [imagePage, setImagePage] = useState(1);
  const [imagePageSize, setImagePageSize] = useState(20);
  const [isLoadingImages, setIsLoadingImages] = useState(false);
  const [selectedImage, setSelectedImage] = useState<GenerationResult | null>(
    null,
  );
  const [folderSelectorImageId, setFolderSelectorImageId] = useState<
    string | null
  >(null);
  const [lightboxState, setLightboxState] = useState<{
    images: MessageImage[];
    index: number;
  } | null>(null);

  const [promptFavorites, setPromptFavorites] = useState<PromptFavorite[]>([]);
  const [promptQuery, setPromptQuery] = useState("");
  const [selectedPromptFolderId, setSelectedPromptFolderId] = useState("");
  const [promptFolderSelectorFavoriteId, setPromptFolderSelectorFavoriteId] =
    useState<string | null>(null);
  const [copiedPromptId, setCopiedPromptId] = useState<string | null>(null);

  const loadImageFavorites = useCallback(
    async (
      page: number,
      query?: string,
      folderId?: string,
      mode: "replace" | "append" = "replace",
    ) => {
      setIsLoadingImages(true);
      const result = await getFavoriteImages(
        folderId || selectedImageFolderId || undefined,
        query?.trim() || imageQuery.trim() || undefined,
        page,
      );
      setImageResults((current) =>
        mode === "append"
          ? [...current, ...result.generations]
          : result.generations,
      );
      setImageTotal(result.total);
      setImagePage(result.page);
      setImagePageSize(result.page_size);
      if (mode === "replace") {
        setSelectedImage(null);
      }
      setIsLoadingImages(false);
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
    setLightboxState((current) => {
      if (!current) return null;
      return current.images.some((image) => image.generationId === id)
        ? null
        : current;
    });
  }

  function handleEditImage(
    imagePath: string,
    imageId: string,
    generationId: string,
  ) {
    const normalizedPath = imagePath.replace(/\\/g, "/");
    const fileName = normalizedPath.split("/").pop() || "source-image";

    savePendingEditSources([
      {
        id: `${imageId}:${normalizedPath}`,
        path: imagePath,
        label: fileName,
        imageId,
        generationId,
      },
    ]);
    setActiveConversationId(null);
    navigate("/generate");
  }

  const handleEditLightboxImage = useCallback(
    (image: MessageImage) => {
      handleEditImage(image.path, image.imageId, image.generationId);
      setLightboxState(null);
    },
    [],
  );

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

  const openLightbox = useCallback(
    (result: GenerationResult, index: number) => {
      setLightboxState({
        images: generationResultToLightboxImages(result),
        index,
      });
    },
    [],
  );

  const activeTotal =
    activeKind === "images" ? imageTotal : promptFavorites.length;
  const activeFolderId =
    activeKind === "images" ? selectedImageFolderId : selectedPromptFolderId;
  const activeFolders = activeKind === "images" ? folders : promptFolders;
  const activeQuery = activeKind === "images" ? imageQuery : promptQuery;
  const hasMoreImages = imagePage * imagePageSize < imageTotal;
  const loadMoreImagesRef = useInfiniteScroll({
    enabled: activeKind === "images" && imageResults.length > 0,
    hasMore: hasMoreImages,
    isLoading: isLoadingImages,
    onLoadMore: () => {
      void loadImageFavorites(
        imagePage + 1,
        imageQuery,
        selectedImageFolderId,
        "append",
      );
    },
  });

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
                className={cn(
                  "pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 transition-colors",
                  activeFolderId ? "text-primary/80" : "text-muted/60",
                )}
                strokeWidth={2}
              />
              <select
                value={activeFolderId}
                onChange={(event) =>
                  handleFolderFilterChange(event.target.value)
                }
                className={cn(
                  "select-control h-[34px] w-full rounded-[10px] border pl-7 pr-8 text-[12px] font-medium transition-all outline-none xl:w-44",
                  "shadow-[inset_0_1px_0_rgba(255,255,255,0.65)]",
                  activeFolderId
                    ? "border-primary/20 bg-surface text-foreground shadow-[0_10px_24px_rgba(79,106,255,0.08)]"
                    : "border-border-subtle bg-subtle/55 text-muted hover:border-border hover:bg-surface/92 hover:text-foreground",
                  "focus:border-primary/30 focus:bg-surface focus:text-foreground focus:shadow-[0_0_0_4px_rgba(79,106,255,0.12)]",
                )}
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
                    {activeKind === "prompts"
                      ? getPromptFolderDisplayName(folder)
                      : folder.name}
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
                  onPreview={openLightbox}
                />
              )}
              <div ref={loadMoreImagesRef} aria-hidden="true" className="h-1" />
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
            onEditImage={handleEditImage}
            onPreview={(imageIndex) => openLightbox(selectedImage, imageIndex)}
            onManageFolders={setFolderSelectorImageId}
          />
        )}
      </AnimatePresence>

      <AnimatePresence>
        {lightboxState && (
          <Lightbox
            images={lightboxState.images}
            initialIndex={lightboxState.index}
            onClose={() => setLightboxState(null)}
            onEditImage={handleEditLightboxImage}
            onDelete={(id) => void handleDeleteImage(id)}
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
