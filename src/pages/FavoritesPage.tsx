import { useEffect, useRef, useState, useCallback } from "react";
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
  deletePromptFavorite,
  getFavoriteImages,
  getPromptFavorites,
} from "../lib/api";
import { buildEditSource, savePendingEditSources } from "../lib/editSources";
import { getPromptFolderDisplayName } from "../lib/promptFolders";
import { cn, formatLocalDateTime } from "../lib/utils";
import { useUIStore, type PreviewSource } from "../lib/store";
import { useFolders } from "../hooks/useFolders";
import { usePromptFolders } from "../hooks/usePromptFolders";
import { useLayoutContext } from "../components/layout/AppLayout";
import type { GenerationResult, MessageImage, PromptFavorite } from "../types";
import FolderSelector from "../components/favorites/FolderSelector";
import PromptFolderSelector from "../components/favorites/PromptFolderSelector";
import ConfirmDialog from "../components/common/ConfirmDialog";
import CollectionStatus from "../components/gallery/CollectionStatus";
import EmptyCollectionState from "../components/gallery/EmptyCollectionState";
import GenerationDetailPanel from "../components/gallery/GenerationDetailPanel";
import CollectionBatchActions from "../components/gallery/CollectionBatchActions";
import GenerationDeleteDialog from "../components/gallery/GenerationDeleteDialog";
import { useCollectionSelection } from "../components/gallery/useCollectionSelection";
import GenerationGrid from "../components/gallery/GenerationGrid";
import ViewModeToggle from "../components/gallery/ViewModeToggle";
import Lightbox from "../components/lightbox/Lightbox";
import { generationResultToLightboxImages } from "../lib/lightboxImages";
import { useInfiniteScroll } from "../hooks/useInfiniteScroll";
import { useStoredGalleryViewMode } from "../hooks/useStoredGalleryViewMode";
import { ROUTES } from "../app/routes/contracts";

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
  const selection = useCollectionSelection(imageResults.map((row) => row.generation.id));
  const [deleteImageTarget, setDeleteImageTarget] = useState<string | null>(null);
  const [imageQuery, setImageQuery] = useState("");
  const [selectedImageFolderId, setSelectedImageFolderId] = useState("");
  const [imageTotal, setImageTotal] = useState(0);
  const [imagePage, setImagePage] = useState(1);
  const [imagePageSize, setImagePageSize] = useState(20);
  const [isLoadingImages, setIsLoadingImages] = useState(true);
  const [isLoadingPrompts, setIsLoadingPrompts] = useState(true);
  const [imageLoadError, setImageLoadError] = useState(false);
  const [promptLoadError, setPromptLoadError] = useState(false);
  const [imageViewMode, setImageViewMode] = useStoredGalleryViewMode(
    "astro-favorites-image-view-mode",
  );
  const [selectedImage, setSelectedImage] = useState<GenerationResult | null>(
    null,
  );
  const imageFavoritesRequestIdRef = useRef(0);
  const appliedImageSearch = useRef({ query: "", folderId: "" });
  const promptFavoritesRequestIdRef = useRef(0);
  const {
    lightbox,
    openLightbox,
    closeLightbox,
    folderSelectorImageId,
    openFolderSelector,
    closeFolderSelector,
  } = useUIStore();

  const [promptFavorites, setPromptFavorites] = useState<PromptFavorite[]>([]);
  const [promptQuery, setPromptQuery] = useState("");
  const [selectedPromptFolderId, setSelectedPromptFolderId] = useState("");
  const [promptFolderSelectorFavoriteId, setPromptFolderSelectorFavoriteId] =
    useState<string | null>(null);
  const [promptCopyError, setPromptCopyError] = useState<string | null>(null);
  const [copiedPromptId, setCopiedPromptId] = useState<string | null>(null);
  const [deletePromptTarget, setDeletePromptTarget] = useState<string | null>(null);
  const [deletePromptBusy, setDeletePromptBusy] = useState(false);
  const [deletePromptError, setDeletePromptError] = useState<string | null>(null);
  const deletePromptBusyRef = useRef(false);

  const loadImageFavorites = useCallback(
    async (
      page: number,
      query?: string,
      folderId?: string,
      mode: "replace" | "append" = "replace",
    ) => {
      const requestId = ++imageFavoritesRequestIdRef.current;
      if (mode === "replace") appliedImageSearch.current = { query: query ?? imageQuery, folderId: folderId ?? selectedImageFolderId };
      setIsLoadingImages(true);
      setImageLoadError(false);
      try {
        const result = await getFavoriteImages(
          (folderId ?? selectedImageFolderId) || undefined,
          (query ?? imageQuery).trim() || undefined,
          page,
        );
        if (requestId !== imageFavoritesRequestIdRef.current) return;
        setImageResults((current) =>
          mode === "append"
            ? [...current, ...result.generations.filter((row) => !current.some((entry) => entry.generation.id === row.generation.id))]
            : result.generations,
        );
        setImageTotal(result.total);
        setImagePage(result.page);
        setImagePageSize(result.page_size);
        if (mode === "replace") {
          setSelectedImage(null);
        }
      } catch {
        if (requestId === imageFavoritesRequestIdRef.current) setImageLoadError(true);
      } finally {
        if (requestId === imageFavoritesRequestIdRef.current) {
          setIsLoadingImages(false);
        }
      }
    },
    [imageQuery, selectedImageFolderId],
  );

  const loadPromptFavorites = useCallback(
    async (query?: string, folderId?: string) => {
      const requestId = ++promptFavoritesRequestIdRef.current;
      setIsLoadingPrompts(true);
      setPromptLoadError(false);
      try {
        const favorites = await getPromptFavorites(
          (query ?? promptQuery).trim() || undefined,
          (folderId ?? selectedPromptFolderId) || undefined,
        );
        if (requestId !== promptFavoritesRequestIdRef.current) return;
        setPromptFavorites(favorites);
      } catch {
        if (requestId === promptFavoritesRequestIdRef.current) setPromptLoadError(true);
      } finally {
        if (requestId === promptFavoritesRequestIdRef.current) setIsLoadingPrompts(false);
      }
    },
    [promptQuery, selectedPromptFolderId],
  );

  useEffect(() => {
    loadImageFavorites(1).catch(() => {});
    loadPromptFavorites().catch(() => {});
    return () => { imageFavoritesRequestIdRef.current++; promptFavoritesRequestIdRef.current++; };
  }, []);

  function handleSearch() {
    selection.clear();
    if (activeKind === "images") {
      loadImageFavorites(1, imageQuery, selectedImageFolderId).catch(() => {});
      return;
    }
    loadPromptFavorites(promptQuery, selectedPromptFolderId).catch(() => {});
  }

  function handleFolderFilterChange(folderId: string) {
    selection.clear();
    if (activeKind === "images") {
      setSelectedImageFolderId(folderId);
      setSelectedImage(null);
      loadImageFavorites(1, imageQuery, folderId).catch(() => {});
      return;
    }

    setSelectedPromptFolderId(folderId);
    loadPromptFavorites(promptQuery, folderId).catch(() => {});
  }

  function handleDeletedImage(id: string) {
    setImageResults((current) => current.filter((row) => row.generation.id !== id));
    setImageTotal((current) => Math.max(0, current - 1));
    selection.remove([id]);
    if (selectedImage?.generation.id === id) setSelectedImage(null);
    if (lightbox?.images.some((image) => image.generationId === id)) {
      closeLightbox();
    }
  }

  const handleEditImage = useCallback(
    (imagePath: string, imageId: string, generationId: string) => {
      savePendingEditSources([
        buildEditSource(imagePath, imageId, generationId),
      ]);
      setActiveConversationId(null);
      navigate(ROUTES.generate);
    },
    [navigate, setActiveConversationId],
  );

  const handleEditLightboxImage = useCallback(
    (image: MessageImage) => {
      handleEditImage(image.path, image.imageId, image.generationId);
      closeLightbox();
    },
    [handleEditImage, closeLightbox],
  );

  async function handleDeletePromptFavorite() {
    if (!deletePromptTarget || deletePromptBusyRef.current) return;
    deletePromptBusyRef.current = true;
    setDeletePromptBusy(true);
    setDeletePromptError(null);
    try {
      await deletePromptFavorite(deletePromptTarget);
      setPromptFavorites((current) => current.filter((favorite) => favorite.id !== deletePromptTarget));
      setDeletePromptTarget(null);
    } catch {
      setDeletePromptError(t("studio.collection.actionError", { defaultValue: "The change could not be saved. Please try again." }));
    } finally {
      deletePromptBusyRef.current = false;
      setDeletePromptBusy(false);
    }
  }

  async function handleCopyPrompt(favorite: PromptFavorite) {
    setPromptCopyError(null);
    setCopiedPromptId(null);
    try { await navigator.clipboard.writeText(favorite.prompt); }
    catch { setPromptCopyError(t("studio.copyFailed", { defaultValue: "Could not copy. Try again." })); return; }
    setCopiedPromptId(favorite.id);
    window.setTimeout(() => {
      setCopiedPromptId((current) =>
        current === favorite.id ? null : current,
      );
    }, 1200);
  }

  function handleImageFolderSelectorClose() {
    closeFolderSelector();
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

  const openResultLightbox = useCallback(
    (result: GenerationResult, index: number, source?: PreviewSource) => {
      openLightbox(generationResultToLightboxImages(result), index, source);
    },
    [openLightbox],
  );

  const activeTotal =
    activeKind === "images" ? imageTotal : promptFavorites.length;
  const activeFolderId =
    activeKind === "images" ? selectedImageFolderId : selectedPromptFolderId;
  const activeFolders = activeKind === "images" ? folders : promptFolders;
  const activeQuery = activeKind === "images" ? imageQuery : promptQuery;
  const hasMoreImages = imageResults.length < imageTotal;
  const loadMoreImagesRef = useInfiniteScroll({
    enabled: activeKind === "images" && imageResults.length > 0 && !imageLoadError && !lightbox,
    hasMore: hasMoreImages,
    isLoading: isLoadingImages,
    onLoadMore: () => {
      void loadImageFavorites(
        Math.min(imagePage + 1, Math.floor(imageResults.length / imagePageSize) + 1),
        appliedImageSearch.current.query,
        appliedImageSearch.current.folderId,
        "append",
      );
    },
  });

  return (
    <div className="flex h-full overflow-hidden bg-background">
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="studio-toolbar flex flex-col gap-3 px-6 py-4 xl:flex-row xl:items-center xl:justify-between">
          <div className="flex min-w-0 flex-wrap items-center gap-3">
            <h2 className="text-[20px] font-semibold text-foreground tracking-tight">
              {t("favorites.title")}
            </h2>
            {activeTotal > 0 && (
              <span className="rounded-[6px] bg-subtle px-2 py-0.5 text-xs font-medium text-muted tabular-nums">
                {activeTotal}
              </span>
            )}
            <div className="studio-control flex rounded-[8px] p-0.5">
              <button
                aria-pressed={activeKind === "images"}
                onClick={() => { setSelectedImage(null); setActiveKind("images"); }}
                className={`focus-ring flex h-7 cursor-pointer items-center gap-1.5 rounded-[6px] px-2.5 text-xs font-medium transition-colors ${
                  activeKind === "images"
                    ? "bg-surface text-foreground shadow-sm"
                    : "text-muted hover:text-foreground"
                }`}
              >
                <ImageIcon size={13} />
                {t("favorites.imagesTab")}
              </button>
              <button
                aria-pressed={activeKind === "prompts"}
                onClick={() => { setSelectedImage(null); setActiveKind("prompts"); }}
                className={`focus-ring flex h-7 cursor-pointer items-center gap-1.5 rounded-[6px] px-2.5 text-xs font-medium transition-colors ${
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
            {activeKind === "images" && (
              <ViewModeToggle
                value={imageViewMode}
                onChange={setImageViewMode}
              />
            )}

            <div className="relative min-w-0 flex-1 xl:flex-none">
              <Folder
                size={13}
                className={cn(
                  "pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 transition-colors",
                  activeFolderId ? "text-primary" : "text-muted",
                )}
                strokeWidth={2}
              />
              <select
                value={activeFolderId}
                onChange={(event) =>
                  handleFolderFilterChange(event.target.value)
                }
                className={cn(
                  "select-control studio-input focus-ring h-9 w-full rounded-[8px] pl-7 pr-8 text-xs font-medium xl:w-44",
                  activeFolderId
                    ? "border-primary/20 bg-surface text-foreground shadow-[0_10px_24px_rgba(79,106,255,0.08)]"
                    : "border-border-subtle bg-subtle/55 text-muted hover:border-border hover:bg-surface/92 hover:text-foreground",
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
                className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"
                strokeWidth={2}
              />
              <input
                aria-label={activeKind === "images" ? t("favorites.search") : t("favorites.searchPrompts")}
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
                className="studio-input focus-ring h-9 w-full rounded-[8px] pl-7 pr-3 text-sm placeholder:text-muted xl:w-52"
              />
            </div>
          </div>
        </div>

        {activeKind === "images" && <CollectionBatchActions rows={imageResults} selectedIds={selection.selectedIds} onSelectLoaded={selection.selectLoaded} onClear={selection.clear} onSucceeded={(ids, kind) => {
          selection.remove(ids);
          if (kind === "delete") ids.forEach(handleDeletedImage);
          else void reloadFolders();
        }} />}
        <div className="flex-1 overflow-y-auto p-5">
          {activeKind === "prompts" && promptCopyError && <p role="alert" className="mb-4 text-sm text-error">{promptCopyError}</p>}
          <CollectionStatus
            loading={activeKind === "images" ? isLoadingImages : isLoadingPrompts}
            error={activeKind === "images" ? imageLoadError : promptLoadError}
            onRetry={handleSearch}
          />
          {activeKind === "images" ? (
            <>
              {imageResults.length === 0 ? (!isLoadingImages && !imageLoadError && (
                <EmptyCollectionState
                  title={t("favorites.noImages")}
                  subtitle={t("favorites.emptyHint")}
                />
              )) : (
                <GenerationGrid
                  results={imageResults}
                  selectedIds={selection.selectedIds}
                  onToggleSelection={selection.toggle}
                  viewMode={imageViewMode}
                  onSelect={setSelectedImage}
                  onPreview={openResultLightbox}
                />
              )}
              <div ref={loadMoreImagesRef} aria-hidden="true" className="h-1" />
            </>
          ) : (!isLoadingPrompts && !promptLoadError && (
            <PromptFavoritesList
              favorites={promptFavorites}
              copiedPromptId={copiedPromptId}
              onCopy={(favorite) => void handleCopyPrompt(favorite)}
              onManageFolders={setPromptFolderSelectorFavoriteId}
              onDelete={(id) => { setDeletePromptError(null); setDeletePromptTarget(id); }}
            />
          ))}
        </div>
      </div>

      <AnimatePresence>
        {selectedImage && (
          <GenerationDetailPanel
            result={selectedImage}
            title={t("favorites.detail")}
            onClose={() => setSelectedImage(null)}
            onDelete={setDeleteImageTarget}
            onEditImage={handleEditImage}
            onPreview={(imageIndex, source) =>
              openResultLightbox(selectedImage, imageIndex, source)
            }
            onManageFolders={openFolderSelector}
          />
        )}
      </AnimatePresence>

      <AnimatePresence>
        {lightbox && (
          <Lightbox
            source={lightbox.source}
            images={lightbox.images}
            initialIndex={lightbox.index}
            onClose={closeLightbox}
            onEditImage={handleEditLightboxImage}
            onDelete={setDeleteImageTarget}
          />
        )}
      </AnimatePresence>

      {deleteImageTarget && <GenerationDeleteDialog generationId={deleteImageTarget} onClose={() => setDeleteImageTarget(null)} onDeleted={handleDeletedImage} />}
      {folderSelectorImageId && (
        <FolderSelector
          imageId={folderSelectorImageId}
          onClose={handleImageFolderSelectorClose}
        />
      )}

      <ConfirmDialog
        open={deletePromptTarget !== null}
        title={t("studio.collection.deletePrompt", { defaultValue: "Permanently remove this saved prompt?" })}
        confirmLabel={t("favorites.deletePrompt")}
        cancelLabel={t("projects.deleteCancel")}
        loading={deletePromptBusy}
        error={deletePromptError}
        onConfirm={() => void handleDeletePromptFavorite()}
        onCancel={() => { if (!deletePromptBusy) setDeletePromptTarget(null); }}
      />
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
        <article key={favorite.id} className="studio-card rounded-[12px] p-5">
          <p className="max-h-36 overflow-hidden whitespace-pre-wrap break-words text-sm leading-relaxed text-foreground">
            {favorite.prompt}
          </p>

          <div className="mt-4 flex items-center justify-between gap-3">
            <span className="min-w-0 truncate text-xs text-muted">
              {formatLocalDateTime(favorite.updated_at)}
            </span>

            <div className="flex shrink-0 items-center gap-1">
              <button
                onClick={() => onCopy(favorite)}
                className="studio-control-subtle focus-ring flex h-8 items-center gap-1.5 rounded-[8px] px-2 text-xs font-medium hover:bg-subtle hover:text-foreground"
              >
                <Copy size={13} />
                {copiedPromptId === favorite.id
                  ? t("favorites.copiedPrompt")
                  : t("favorites.copyPrompt")}
              </button>
              <button
                onClick={() => onManageFolders(favorite.id)}
                className="studio-control-subtle focus-ring flex h-8 items-center gap-1.5 rounded-[8px] px-2 text-xs font-medium hover:bg-subtle hover:text-foreground"
              >
                <Folder size={13} />
                {t("favorites.manageFolders")}
              </button>
              <button
                onClick={() => onDelete(favorite.id)}
                className="studio-control-subtle focus-ring flex h-8 w-8 items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-error/6 hover:text-error"
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
