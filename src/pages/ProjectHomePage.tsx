import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { AnimatePresence } from "framer-motion";
import { Image as ImageIcon, Pin, PinOff, Trash2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  archiveProject,
  createConversation,
  deleteConversation,
  deleteGeneration,
  deleteProject,
  getConversations,
  getProjects,
  pinConversation,
  pinProject,
  renameConversation,
  renameProject,
  searchGenerations,
  unpinConversation,
  toAssetUrl,
  unpinProject,
} from "../lib/api";
import { buildEditSource, savePendingEditSources } from "../lib/editSources";
import { useUIStore } from "../lib/store";
import { generationResultToLightboxImages } from "../lib/lightboxImages";
import { useLayoutContext } from "../components/layout/AppLayout";
import type { Conversation, GenerationResult, MessageImage, Project } from "../types";
import ProjectSummaryCards from "../components/projects/ProjectSummaryCards";
import ProjectImagePanel from "../components/projects/ProjectImagePanel";
import ProjectNameDialog from "../components/projects/ProjectNameDialog";
import ProjectActionsMenu from "../components/projects/ProjectActionsMenu";
import ConfirmDialog from "../components/common/ConfirmDialog";
import GenerationDetailPanel from "../components/gallery/GenerationDetailPanel";
import Lightbox from "../components/lightbox/Lightbox";
import FolderSelector from "../components/favorites/FolderSelector";

export default function ProjectHomePage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { projectId = "" } = useParams();
  const { setActiveConversationId, setActiveProjectId } = useLayoutContext();
  const [project, setProject] = useState<Project | null>(null);
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [results, setResults] = useState<GenerationResult[]>([]);
  const [total, setTotal] = useState(0);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [showActions, setShowActions] = useState(false);
  const [renameDialogOpen, setRenameDialogOpen] = useState(false);
  const [renameLoading, setRenameLoading] = useState(false);
  const [renameError, setRenameError] = useState<string | null>(null);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deleteLoading, setDeleteLoading] = useState(false);
  const [projectActionPending, setProjectActionPending] = useState(false);
  const projectActionPendingRef = useRef(false);
  const [projectActionError, setProjectActionError] = useState<string | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [selectedImage, setSelectedImage] = useState<GenerationResult | null>(null);
  const [conversationRenameTarget, setConversationRenameTarget] =
    useState<Conversation | null>(null);
  const [renameConversationLoading, setRenameConversationLoading] = useState(false);
  const [renameConversationError, setRenameConversationError] = useState<string | null>(null);
  const [conversationDeleteTarget, setConversationDeleteTarget] =
    useState<Conversation | null>(null);
  const [deleteConversationLoading, setDeleteConversationLoading] = useState(false);
  const [deleteConversationError, setDeleteConversationError] = useState<string | null>(null);
  const {
    lightbox,
    openLightbox,
    closeLightbox,
    folderSelectorImageId,
    openFolderSelector,
    closeFolderSelector,
  } = useUIStore();

  useEffect(() => {
    setActiveProjectId(projectId);
    setActiveConversationId(null);
  }, [projectId, setActiveConversationId, setActiveProjectId]);

  useEffect(() => {
    Promise.all([
      getProjects(false).then((items) => {
        setProject(items.find((item) => item.id === projectId && item.id !== "default") ?? null);
      }),
      getConversations(undefined, projectId, false).then(setConversations),
      searchGenerations(undefined, 1, false, {}, projectId).then((result) => {
        setResults(result.generations);
        setTotal(result.total);
        setPage(result.page);
        setPageSize(result.page_size);
      }),
    ]).catch(() => {
      setProject(null);
      setConversations([]);
      setResults([]);
      setTotal(0);
    });
  }, [projectId]);

  function handleRenameAction() {
    if (!project || projectActionPendingRef.current) return;

    setRenameError(null);
    setProjectActionError(null);
    setRenameDialogOpen(true);
    setShowActions(false);
  }

  async function handlePinProject() {
    if (!project || projectActionPendingRef.current) return;

    const currentProject = project;
    projectActionPendingRef.current = true;
    setProjectActionPending(true);
    setProjectActionError(null);
    try {
      await pinProject(currentProject.id);
      setProject({ ...currentProject, pinned_at: currentProject.pinned_at ?? new Date().toISOString() });
      setShowActions(false);
    } catch {
      setProjectActionError(t("projects.actionError"));
    } finally {
      projectActionPendingRef.current = false;
      setProjectActionPending(false);
    }
  }

  async function handleUnpinProject() {
    if (!project || projectActionPendingRef.current) return;

    const currentProject = project;
    projectActionPendingRef.current = true;
    setProjectActionPending(true);
    setProjectActionError(null);
    try {
      await unpinProject(currentProject.id);
      setProject({ ...currentProject, pinned_at: null });
      setShowActions(false);
    } catch {
      setProjectActionError(t("projects.actionError"));
    } finally {
      projectActionPendingRef.current = false;
      setProjectActionPending(false);
    }
  }

  async function handleArchiveProject() {
    if (!project || projectActionPendingRef.current) return;

    projectActionPendingRef.current = true;
    setProjectActionPending(true);
    setProjectActionError(null);
    try {
      await archiveProject(project.id);
      setShowActions(false);
      navigate("/projects");
    } catch {
      setProjectActionError(t("projects.actionError"));
    } finally {
      projectActionPendingRef.current = false;
      setProjectActionPending(false);
    }
  }

  function handleDeleteAction() {
    if (!project || projectActionPendingRef.current) return;

    setShowActions(false);
    setProjectActionError(null);
    setDeleteError(null);
    setDeleteDialogOpen(true);
  }

  async function handleDeleteProject() {
    if (!project || projectActionPendingRef.current) return;

    projectActionPendingRef.current = true;
    setDeleteLoading(true);
    setDeleteError(null);
    try {
      await deleteProject(project.id);
      setDeleteDialogOpen(false);
      navigate("/projects");
    } catch {
      setDeleteError(t("projects.deleteError"));
    } finally {
      projectActionPendingRef.current = false;
      setDeleteLoading(false);
    }
  }

  async function handleRenameProject(name: string) {
    if (!project) return;
    if (name === project.name) {
      setRenameDialogOpen(false);
      setRenameError(null);
      return;
    }

    setRenameLoading(true);
    setRenameError(null);
    try {
      await renameProject(project.id, name);
      setRenameDialogOpen(false);
      setProject((current) => (current ? { ...current, name } : current));
    } catch {
      setRenameError(t("projectDialog.renameError"));
    } finally {
      setRenameLoading(false);
    }
  }

  function handleConversationClick(conversation: Conversation) {
    navigate(`/projects/${project!.id}/chat/${conversation.id}`);
  }

  async function handleRenameConversation(name: string) {
    if (!conversationRenameTarget) return;
    if (name === conversationRenameTarget.title) {
      setConversationRenameTarget(null);
      setRenameConversationError(null);
      return;
    }

    setRenameConversationLoading(true);
    setRenameConversationError(null);
    try {
      await renameConversation(conversationRenameTarget.id, name);
      setConversationRenameTarget(null);
      setConversations((current) =>
        current.map((c) => (c.id === conversationRenameTarget.id ? { ...c, title: name } : c)),
      );
    } catch {
      setRenameConversationError(t("projects.renameConversationError"));
    } finally {
      setRenameConversationLoading(false);
    }
  }

  async function handlePinConversationAction(conversation: Conversation) {
    try {
      if (conversation.pinned_at) {
        await unpinConversation(conversation.id);
      } else {
        await pinConversation(conversation.id);
      }
      setConversations((current) =>
        current.map((c) =>
          c.id === conversation.id
            ? { ...c, pinned_at: conversation.pinned_at ? null : new Date().toISOString() }
            : c,
        ),
      );
    } catch {
      // silently fail for pin toggle
    }
  }

  async function handleDeleteConversationAction() {
    if (!conversationDeleteTarget) return;

    setDeleteConversationLoading(true);
    setDeleteConversationError(null);
    try {
      await deleteConversation(conversationDeleteTarget.id);
      setConversationDeleteTarget(null);
      setConversations((current) =>
        current.filter((c) => c.id !== conversationDeleteTarget.id),
      );
    } catch {
      setDeleteConversationError(t("projects.deleteConversationError"));
    } finally {
      setDeleteConversationLoading(false);
    }
  }

  async function handleDeleteImage(id: string) {
    await deleteGeneration(id);
    const result = await searchGenerations(undefined, page, false, {}, project!.id);
    setResults(result.generations);
    setTotal(result.total);
    if (selectedImage?.generation.id === id) setSelectedImage(null);
    if (lightbox?.images.some((image) => image.generationId === id)) {
      closeLightbox();
    }
  }

  const handleEditImage = useCallback(
    (imagePath: string, imageId: string, generationId: string) => {
      savePendingEditSources([buildEditSource(imagePath, imageId, generationId)]);
      navigate(`/projects/${project!.id}/chat`);
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
    searchGenerations(undefined, page, false, {}, project!.id).then((result) => {
      setResults(result.generations);
      setTotal(result.total);
    }).catch(() => {});
  }

  if (!project) {
    return <div className="p-8 text-[14px] text-muted">{t("projects.notFound")}</div>;
  }

  return (
    <div className="flex h-full">
      <div className="flex-1 overflow-y-auto px-8 py-8">
      <div className="flex items-start justify-between gap-4">
        <div>
          <div className="text-[11px] uppercase tracking-[0.08em] text-muted">{t("projects.directory")}</div>
          <h1 className="mt-2 text-[30px] font-semibold text-foreground">{project.name}</h1>
        </div>
        <div className="relative flex items-center gap-2">
          <button
            onClick={() => setShowActions((current) => !current)}
            aria-label={t("projects.manage")}
            className="rounded-[12px] border border-border-subtle px-4 py-2 text-[12px] font-medium text-foreground"
          >
            {t("projects.manage")}
          </button>
          <ProjectActionsMenu
            open={showActions}
            pinned={project.pinned_at !== null}
            disabled={projectActionPending}
            onRename={handleRenameAction}
            onPin={() => void handlePinProject()}
            onUnpin={() => void handleUnpinProject()}
            onArchive={() => void handleArchiveProject()}
            onDelete={handleDeleteAction}
          />
          <button
            onClick={() => {
              createConversation(undefined, project.id)
                .then((conversation) => {
                  setConversations((current) => [conversation, ...current]);
                  navigate(`/projects/${project.id}/chat/${conversation.id}`);
                })
                .catch(() => {
                  navigate(`/projects/${project.id}/chat`);
                });
            }}
            className="rounded-[12px] bg-primary px-4 py-2 text-[12px] font-medium text-white"
          >
            {t("projects.newConversation")}
          </button>
        </div>
      </div>
      {projectActionError ? (
        <div
          role="alert"
          className="mt-4 rounded-[12px] border border-error/15 bg-error/8 px-4 py-3 text-[13px] text-error"
        >
          {projectActionError}
        </div>
      ) : null}

      <div className="mt-6">
        <ProjectSummaryCards project={project} />
      </div>

      <section className="mt-8">
        <h2 className="text-[18px] font-semibold text-foreground">{t("projects.conversations")}</h2>
        <div className="mt-4 grid gap-2">
          {conversations.length === 0 ? (
            <div className="rounded-[14px] border border-border-subtle bg-surface p-4 text-[13px] text-muted">
              {t("projects.emptyConversations")}
            </div>
          ) : (
            conversations.map((conversation) => (
              <div
                key={conversation.id}
                className="group flex items-center gap-3 rounded-[12px] border border-border-subtle bg-surface p-3 transition-shadow hover:border-border hover:shadow-sm"
              >
                <button
                  onClick={() => handleConversationClick(conversation)}
                  className="flex min-w-0 flex-1 items-center gap-3 text-left"
                >
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center overflow-hidden rounded-[8px] bg-subtle border border-border-subtle">
                    {conversation.latest_thumbnail ? (
                      <img
                        src={toAssetUrl(conversation.latest_thumbnail)}
                        alt=""
                        className="h-full w-full object-cover"
                        loading="lazy"
                      />
                    ) : (
                      <ImageIcon size={14} className="text-muted/30" />
                    )}
                  </div>
                  <div className="min-w-0">
                    <div className="flex items-center gap-1.5">
                      {conversation.pinned_at && (
                        <Pin size={10} className="shrink-0 text-primary" />
                      )}
                      <p className="truncate text-[13px] font-medium text-foreground">
                        {conversation.title}
                      </p>
                    </div>
                    <p className="mt-0.5 text-[11px] text-muted">
                      {t("projects.conversationImageCount", { count: conversation.generation_count })}
                    </p>
                  </div>
                </button>
                <div className="flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
                  <button
                    onClick={() => setConversationRenameTarget(conversation)}
                    className="flex h-8 items-center gap-1 rounded-[8px] px-2 text-[11px] text-muted transition-colors hover:bg-subtle hover:text-foreground"
                    aria-label={t("sidebar.renameConversation")}
                  >
                    {t("sidebar.rename")}
                  </button>
                  <button
                    onClick={() => void handlePinConversationAction(conversation)}
                    className="flex h-8 w-8 items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-subtle hover:text-foreground"
                    aria-label={conversation.pinned_at ? t("sidebar.unpin") : t("sidebar.pin")}
                  >
                    {conversation.pinned_at ? <PinOff size={13} /> : <Pin size={13} />}
                  </button>
                  <button
                    onClick={() => setConversationDeleteTarget(conversation)}
                    className="flex h-8 w-8 items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-error/6 hover:text-error"
                    aria-label={t("sidebar.deleteConversationConfirm")}
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              </div>
            ))
          )}
        </div>
      </section>

      <section className="mt-8">
        <ProjectImagePanel
          results={results}
          total={total}
          page={page}
          pageSize={pageSize}
          onSearch={async (query, filters, nextPage) => {
            const result = await searchGenerations(query || undefined, nextPage, false, filters, project.id);
            setResults(result.generations);
            setTotal(result.total);
            setPage(result.page);
            setPageSize(result.page_size);
          }}
          onSelect={setSelectedImage}
          onPreview={openResultLightbox}
          onManageFolders={openFolderSelector}
        />
      </section>
      </div>

      <AnimatePresence>
        {selectedImage && (
          <GenerationDetailPanel
            result={selectedImage}
            title={t("projects.imagesTitle")}
            onClose={() => setSelectedImage(null)}
            onDelete={(id) => void handleDeleteImage(id)}
            onEditImage={handleEditImage}
            onPreview={(imageIndex) => openResultLightbox(selectedImage, imageIndex)}
            onManageFolders={openFolderSelector}
          />
        )}
      </AnimatePresence>

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

      {folderSelectorImageId && (
        <FolderSelector
          imageId={folderSelectorImageId}
          onClose={handleFolderSelectorClose}
        />
      )}
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
      <ProjectNameDialog
        open={conversationRenameTarget !== null}
        title={t("sidebar.renameConversation")}
        label={t("projectDialog.nameLabel")}
        initialName={conversationRenameTarget?.title ?? ""}
        submitLabel={t("projectDialog.renameSubmit")}
        cancelLabel={t("projectDialog.cancel")}
        requiredMessage={t("projectDialog.nameRequired")}
        error={renameConversationError}
        loading={renameConversationLoading}
        onSubmit={(name) => void handleRenameConversation(name)}
        onCancel={() => {
          if (!renameConversationLoading) {
            setConversationRenameTarget(null);
            setRenameConversationError(null);
          }
        }}
      />
      <ConfirmDialog
        open={conversationDeleteTarget !== null}
        title={t("sidebar.deleteConversationConfirm")}
        confirmLabel={t("projects.deleteConfirmAction")}
        cancelLabel={t("projects.deleteCancel")}
        loading={deleteConversationLoading}
        error={deleteConversationError}
        onConfirm={() => void handleDeleteConversationAction()}
        onCancel={() => {
          if (!deleteConversationLoading) {
            setConversationDeleteTarget(null);
          }
        }}
      />
    </div>
  );
}
