import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { AnimatePresence } from "framer-motion";
import { Archive, ArrowRight, MessageSquare, Pencil, Pin, PinOff, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  archiveProject,
  deleteGeneration,
  deleteProject,
  getProjects,
  getConversations,
  pinProject,
  renameProject,
  searchGenerations,
  unpinProject,
} from "../lib/api";
import { buildEditSource, savePendingEditSources } from "../lib/editSources";
import { useUIStore } from "../lib/store";
import { generationResultToLightboxImages } from "../lib/lightboxImages";
import type {
  Conversation,
  GenerationResult,
  GenerationSearchFilters,
  MessageImage,
  Project,
} from "../types";
import CollectionStatus from "../components/gallery/CollectionStatus";
import ProjectImagePanel from "../components/projects/ProjectImagePanel";
import ProjectNameDialog from "../components/projects/ProjectNameDialog";
import ConfirmDialog from "../components/common/ConfirmDialog";
import GenerationDetailPanel from "../components/gallery/GenerationDetailPanel";
import Lightbox from "../components/lightbox/Lightbox";
import FolderSelector from "../components/favorites/FolderSelector";
import { ROUTES } from "../app/routes/contracts";

export default function ProjectHomePage() {
  const { t, i18n } = useTranslation();
  const navigate = useNavigate();
  const { projectId = "" } = useParams();
  const [project, setProject] = useState<Project | null>(null);
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [conversationsLoading, setConversationsLoading] = useState(true);
  const [conversationsError, setConversationsError] = useState(false);
  const conversationRequestId = useRef(0);
  const loadConversations = useCallback(async () => {
    const requestId = ++conversationRequestId.current;
    setConversationsLoading(true);
    setConversationsError(false);
    try {
      const items = await getConversations(undefined, projectId, false);
      if (requestId !== conversationRequestId.current) return;
      setConversations(items.filter((item) => !item.archived_at && !item.deleted_at)
        .sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime())
        .slice(0, 3));
    } catch {
      if (requestId === conversationRequestId.current) setConversationsError(true);
    } finally {
      if (requestId === conversationRequestId.current) setConversationsLoading(false);
    }
  }, [projectId]);
  const [results, setResults] = useState<GenerationResult[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [selectedImage, setSelectedImage] = useState<GenerationResult | null>(
    null,
  );

  // Project action dialogs
  const [renameDialogOpen, setRenameDialogOpen] = useState(false);
  const [renameLoading, setRenameLoading] = useState(false);
  const [renameError, setRenameError] = useState<string | null>(null);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deleteLoading, setDeleteLoading] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [projectActionPending, setProjectActionPending] = useState(false);
  const projectActionPendingRef = useRef(false);
  const [projectActionError, setProjectActionError] = useState<string | null>(
    null,
  );
  const [showActions, setShowActions] = useState(false);
  const actionsContainerRef = useRef<HTMLDivElement>(null);
  const actionsTriggerRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!showActions) return;
    const dismissOutside = (event: PointerEvent) => {
      if (!actionsContainerRef.current?.contains(event.target as Node)) setShowActions(false);
    };
    const dismissEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      setShowActions(false);
      actionsTriggerRef.current?.focus();
    };
    document.addEventListener("pointerdown", dismissOutside);
    document.addEventListener("keydown", dismissEscape);
    return () => {
      document.removeEventListener("pointerdown", dismissOutside);
      document.removeEventListener("keydown", dismissEscape);
    };
  }, [showActions]);


  const {
    lightbox,
    openLightbox,
    closeLightbox,
    folderSelectorImageId,
    openFolderSelector,
    closeFolderSelector,
  } = useUIStore();

  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState(false);
  const [reloadVersion, setReloadVersion] = useState(0);
  const [imagesLoading, setImagesLoading] = useState(false);
  const [imagesError, setImagesError] = useState(false);
  const imageRequestId = useRef(0);
  const projectEpoch = useRef(0);
  const activeSearch = useRef<{
    query: string;
    filters: GenerationSearchFilters;
  }>({ query: "", filters: {} });

  const loadImages = useCallback(
    async (
      query: string,
      filters: GenerationSearchFilters,
      nextPage: number,
    ) => {
      const requestId = ++imageRequestId.current;
      activeSearch.current = { query, filters };
      setImagesLoading(true);
      setImagesError(false);
      try {
        const result = await searchGenerations(
          query.trim() || undefined,
          nextPage,
          false,
          filters,
          projectId,
        );
        if (requestId !== imageRequestId.current) return;
        setResults(result.generations);
        setTotal(result.total);
        setPage(result.page);
        setPageSize(result.page_size);
        setSelectedImage(null);
      } catch {
        if (requestId === imageRequestId.current) setImagesError(true);
      } finally {
        if (requestId === imageRequestId.current) setImagesLoading(false);
      }
    },
    [projectId],
  );

  useEffect(() => {
    let cancelled = false;
    projectActionPendingRef.current = false;
    setProjectActionPending(false);
    setProjectActionError(null);
    setRenameDialogOpen(false);
    setRenameLoading(false);
    setRenameError(null);
    setDeleteDialogOpen(false);
    setDeleteLoading(false);
    setDeleteError(null);
    setShowActions(false);
    setLoading(true);
    setLoadError(false);
    setProject(null);
    setResults([]);
    setConversations([]);
    void loadConversations();
    setSelectedImage(null);
    getProjects(false)
      .then((items) => {
        if (!cancelled)
          setProject(
            items.find(
              (item) => item.id === projectId && item.id !== "default",
            ) ?? null,
          );
      })
      .catch(() => {
        if (!cancelled) setLoadError(true);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    void loadImages("", {}, 1);
    return () => {
      cancelled = true;
      imageRequestId.current++;
      conversationRequestId.current++;
      projectEpoch.current++;
    };
  }, [projectId, reloadVersion, loadImages, loadConversations]);

  async function handleRenameProject(name: string) {
    if (!project) return;
    if (name === project.name) {
      setRenameDialogOpen(false);
      setRenameError(null);
      return;
    }
    const operationEpoch = projectEpoch.current;
    setRenameLoading(true);
    setRenameError(null);
    try {
      await renameProject(project.id, name);
      if (operationEpoch !== projectEpoch.current) return;
      setRenameDialogOpen(false);
      setProject((current) => (current ? { ...current, name } : current));
    } catch {
      if (operationEpoch !== projectEpoch.current) return;
      setRenameError(t("projectDialog.renameError"));
    } finally {
      if (operationEpoch === projectEpoch.current) {
        setRenameLoading(false);
      }
    }
  }

  async function handlePinProject() {
    if (!project || projectActionPendingRef.current) return;
    const currentProject = project;
    const operationEpoch = projectEpoch.current;
    projectActionPendingRef.current = true;
    setProjectActionPending(true);
    setProjectActionError(null);
    try {
      await pinProject(currentProject.id);
      if (operationEpoch !== projectEpoch.current) return;
      setProject({
        ...currentProject,
        pinned_at: currentProject.pinned_at ?? new Date().toISOString(),
      });
      setShowActions(false);
    } catch {
      if (operationEpoch !== projectEpoch.current) return;
      setProjectActionError(t("projects.actionError"));
    } finally {
      if (operationEpoch === projectEpoch.current) {
        projectActionPendingRef.current = false;
        setProjectActionPending(false);
      }
    }
  }

  async function handleUnpinProject() {
    if (!project || projectActionPendingRef.current) return;
    const currentProject = project;
    const operationEpoch = projectEpoch.current;
    projectActionPendingRef.current = true;
    setProjectActionPending(true);
    setProjectActionError(null);
    try {
      await unpinProject(currentProject.id);
      if (operationEpoch !== projectEpoch.current) return;
      setProject({ ...currentProject, pinned_at: null });
      setShowActions(false);
    } catch {
      if (operationEpoch !== projectEpoch.current) return;
      setProjectActionError(t("projects.actionError"));
    } finally {
      if (operationEpoch === projectEpoch.current) {
        projectActionPendingRef.current = false;
        setProjectActionPending(false);
      }
    }
  }

  async function handleArchiveProject() {
    if (!project || projectActionPendingRef.current) return;
    const operationEpoch = projectEpoch.current;
    projectActionPendingRef.current = true;
    setProjectActionPending(true);
    setProjectActionError(null);
    try {
      await archiveProject(project.id);
      if (operationEpoch !== projectEpoch.current) return;
      setShowActions(false);
      navigate(ROUTES.projects);
    } catch {
      if (operationEpoch !== projectEpoch.current) return;
      setProjectActionError(t("projects.actionError"));
    } finally {
      if (operationEpoch === projectEpoch.current) {
        projectActionPendingRef.current = false;
        setProjectActionPending(false);
      }
    }
  }

  async function handleDeleteProject() {
    if (!project || projectActionPendingRef.current) return;
    const operationEpoch = projectEpoch.current;
    projectActionPendingRef.current = true;
    setDeleteLoading(true);
    setDeleteError(null);
    try {
      await deleteProject(project.id);
      if (operationEpoch !== projectEpoch.current) return;
      setDeleteDialogOpen(false);
      navigate(ROUTES.projects);
    } catch {
      if (operationEpoch !== projectEpoch.current) return;
      setDeleteError(t("projects.deleteError"));
    } finally {
      if (operationEpoch === projectEpoch.current) {
        projectActionPendingRef.current = false;
        setDeleteLoading(false);
      }
    }
  }

  async function handleDeleteImage(id: string) {
    await deleteGeneration(id);
    await loadImages(
      activeSearch.current.query,
      activeSearch.current.filters,
      1,
    );
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
      navigate(ROUTES.projectChat(project!.id));
    },
    [navigate, project],
  );

  const handleEditLightboxImage = useCallback(
    (image: MessageImage) => {
      handleEditImage(image.path, image.imageId, image.generationId);
      closeLightbox();
    },
    [handleEditImage, closeLightbox],
  );

  const openResultLightbox = useCallback(
    (result: GenerationResult, index: number) => {
      openLightbox(generationResultToLightboxImages(result), index);
    },
    [openLightbox],
  );

  function handleFolderSelectorClose() {
    closeFolderSelector();
    void loadImages(
      activeSearch.current.query,
      activeSearch.current.filters,
      page,
    );
  }

  if (loading || loadError) {
    return (
      <div className="p-6">
        <CollectionStatus
          loading={loading}
          error={loadError}
          onRetry={() => setReloadVersion((value) => value + 1)}
        />
      </div>
    );
  }

  if (!project) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-sm text-muted">{t("projects.notFound")}</p>
      </div>
    );
  }

  return (
    <div className="flex h-full min-w-0 overflow-hidden bg-background">
      <div className="flex min-w-0 flex-1 flex-col">
        {/* Project Header */}
        <div className="shrink-0 border-b border-border-subtle bg-surface/40 px-6 py-4">
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0">
              <div className="mb-1 flex items-center gap-2">
                {project.pinned_at && (
                  <span className="inline-flex items-center gap-1 rounded-[5px] bg-primary/8 px-1.5 py-px text-xs font-medium text-primary">
                    <Pin size={9} />
                    {t("projects.pinned")}
                  </span>
                )}
              </div>
              <h1 className="text-[20px] font-semibold text-foreground tracking-tight truncate">
                {project.name}
              </h1>
              <p className="mt-1 text-xs text-muted">
                <span>
                  {t("projects.imageCountValue", {
                    count: project.image_count,
                  })}
                </span>
                <span className="mx-2">·</span>
                {t("studio.project.sessions", {
                  defaultValue: "{{count}} conversations",
                  count: project.conversation_count,
                })}
              </p>
            </div>

            <div className="flex shrink-0 items-center gap-2">
              <button
                onClick={() => navigate(ROUTES.projectChat(project.id))}
                className="studio-control-primary focus-ring flex h-9 items-center gap-2 rounded-lg px-4 text-sm font-medium"
              >
                {t("studio.project.continue", {
                  defaultValue: "Continue creating",
                })}
                <ArrowRight size={15} />
              </button>
              <div ref={actionsContainerRef} className="relative">
                <button
                  ref={actionsTriggerRef}
                  aria-expanded={showActions}
                  onClick={() => setShowActions((c) => !c)}
                  className="flex h-8 w-8 items-center justify-center rounded-[8px] text-muted transition-all hover:bg-subtle hover:text-foreground"
                  aria-label={t("projects.manage")}
                >
                  <svg
                    width="15"
                    height="3"
                    viewBox="0 0 15 3"
                    fill="currentColor"
                  >
                    <circle cx="1.5" cy="1.5" r="1.5" />
                    <circle cx="7.5" cy="1.5" r="1.5" />
                    <circle cx="13.5" cy="1.5" r="1.5" />
                  </svg>
                </button>

                {showActions && (
                  <div className="absolute right-0 top-10 z-20 w-40 overflow-hidden rounded-[8px] border border-border-subtle bg-surface py-1 shadow-[0_14px_35px_rgba(0,0,0,0.15)]">
                    <button
                      onClick={() => {
                        setShowActions(false);
                        setRenameError(null);
                        setRenameDialogOpen(true);
                      }}
                      disabled={projectActionPending}
                      className="flex w-full items-center gap-2 px-3 py-2 text-left text-xs text-foreground hover:bg-subtle hover:text-foreground transition-colors disabled:opacity-50"
                    >
                      <Pencil size={13} />
                      <span>{t("sidebar.rename")}</span>
                    </button>
                    {project.pinned_at ? (
                      <button
                        onClick={() => void handleUnpinProject()}
                        disabled={projectActionPending}
                        className="flex w-full items-center gap-2 px-3 py-2 text-left text-xs text-foreground hover:bg-subtle hover:text-foreground transition-colors disabled:opacity-50"
                      >
                        <PinOff size={13} />
                        <span>{t("projects.unpin")}</span>
                      </button>
                    ) : (
                      <button
                        onClick={() => void handlePinProject()}
                        disabled={projectActionPending}
                        className="flex w-full items-center gap-2 px-3 py-2 text-left text-xs text-foreground hover:bg-subtle hover:text-foreground transition-colors disabled:opacity-50"
                      >
                        <Pin size={13} />
                        <span>{t("projects.pin")}</span>
                      </button>
                    )}
                    <button
                      onClick={() => void handleArchiveProject()}
                      disabled={projectActionPending}
                      className="flex w-full items-center gap-2 px-3 py-2 text-left text-xs text-foreground hover:bg-subtle hover:text-foreground transition-colors disabled:opacity-50"
                    >
                      <Archive size={13} />
                      <span>{t("sidebar.archive")}</span>
                    </button>
                    <button
                      onClick={() => {
                        setShowActions(false);
                        setDeleteError(null);
                        setDeleteDialogOpen(true);
                      }}
                      disabled={projectActionPending}
                      className="flex w-full items-center gap-2 px-3 py-2 text-left text-xs text-error hover:bg-error/8 transition-colors disabled:opacity-50"
                    >
                      <Trash2 size={13} />
                      <span>{t("sidebar.delete")}</span>
                    </button>
                  </div>
                )}
              </div>
            </div>
          </div>

          {projectActionError ? (
            <div
              role="alert"
              className="mt-3 rounded-[8px] border border-error/15 bg-error/8 px-3 py-2 text-xs text-error"
            >
              {projectActionError}
            </div>
          ) : null}
        </div>

        <section aria-label={t("studio.project.recentConversations", { defaultValue: "Recent conversations" })} className="shrink-0 border-b border-border-subtle px-6 py-4">
          <h2 className="mb-3 text-sm font-semibold text-foreground">{t("studio.project.recentConversations", { defaultValue: "Recent conversations" })}</h2>
          {conversationsLoading ? (
            <div role="status" aria-label={t("studio.project.loadingConversations", { defaultValue: "Loading conversations…" })} className="grid grid-cols-3 gap-3">
              {[0, 1, 2].map((key) => <div key={key} className="h-16 rounded-lg bg-subtle" />)}
            </div>
          ) : conversationsError ? (
            <div role="alert" className="flex flex-wrap items-center gap-3 text-sm text-error">
              <span>{t("studio.project.conversationsFailed", { defaultValue: "Could not load recent conversations." })}</span>
              <button type="button" onClick={() => void loadConversations()} className="studio-control focus-ring min-h-9 rounded-lg px-3 text-foreground">{t("studio.project.retryConversations", { defaultValue: "Retry conversations" })}</button>
            </div>
          ) : conversations.length === 0 ? (
            <p className="text-sm text-muted">{t("projects.emptyConversations")}</p>
          ) : (
            <div className="grid gap-3 sm:grid-cols-3">
              {conversations.map((conversation) => (
                <button key={conversation.id} type="button" onClick={() => navigate(ROUTES.projectChat(project.id, conversation.id))} className="studio-control focus-ring flex min-w-0 items-center gap-3 rounded-lg p-3 text-left hover:studio-control-hover">
                  <MessageSquare size={16} className="shrink-0 text-muted" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate text-sm font-medium text-foreground">{conversation.title}</span>
                    <time dateTime={conversation.updated_at} className="mt-1 block text-xs text-muted">{new Date(conversation.updated_at).toLocaleDateString(i18n.resolvedLanguage ?? i18n.language)}</time>
                  </span>
                </button>
              ))}
            </div>
          )}
        </section>

        {/* Image Gallery */}
        <div className="flex-1 min-h-0 overflow-y-auto">
          <ProjectImagePanel
            results={results}
            total={total}
            page={page}
            pageSize={pageSize}
            loading={imagesLoading}
            error={imagesError}
            onSearch={loadImages}
            onSelect={setSelectedImage}
            onPreview={openResultLightbox}
            onManageFolders={openFolderSelector}
          />
        </div>
      </div>

      {/* Slide-in Detail Panel */}
      <AnimatePresence>
        {selectedImage && (
          <GenerationDetailPanel
            result={selectedImage}
            title={t("projects.imagesTitle")}
            onClose={() => setSelectedImage(null)}
            onDelete={(id) => void handleDeleteImage(id)}
            onEditImage={handleEditImage}
            onPreview={(imageIndex) =>
              openResultLightbox(selectedImage, imageIndex)
            }
            onManageFolders={openFolderSelector}
          />
        )}
      </AnimatePresence>

      {/* Lightbox */}
      <AnimatePresence>
        {lightbox && (
          <Lightbox
            images={lightbox.images}
            initialIndex={lightbox.index}
            onClose={closeLightbox}
            onEditImage={handleEditLightboxImage}
            onDelete={(id) => void handleDeleteImage(id)}
          />
        )}
      </AnimatePresence>

      {/* Folder Selector */}
      {folderSelectorImageId && (
        <FolderSelector
          imageId={folderSelectorImageId}
          onClose={handleFolderSelectorClose}
        />
      )}

      {/* Dialogs */}
      <ProjectNameDialog
        open={renameDialogOpen}
        title={t("projectDialog.renameTitle")}
        label={t("projectDialog.nameLabel")}
        initialName={project.name}
        submitLabel={t("projectDialog.renameSubmit")}
        cancelLabel={t("projectDialog.cancel")}
        requiredMessage={t("projectDialog.nameRequired")}
        error={renameError}
        loading={renameLoading}
        onSubmit={(name) => void handleRenameProject(name)}
        onCancel={() => {
          if (!renameLoading) {
            setRenameDialogOpen(false);
            setRenameError(null);
          }
        }}
      />
      <ConfirmDialog
        open={deleteDialogOpen}
        title={t("projects.deleteConfirm")}
        confirmLabel={t("projects.deleteConfirmAction")}
        cancelLabel={t("projects.deleteCancel")}
        loading={deleteLoading}
        error={deleteError}
        onConfirm={() => void handleDeleteProject()}
        onCancel={() => {
          if (!deleteLoading) {
            setDeleteDialogOpen(false);
          }
        }}
      />
    </div>
  );
}
