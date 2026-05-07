import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import {
  archiveProject,
  deleteProject,
  getConversations,
  getProjects,
  pinProject,
  renameProject,
  searchGenerations,
  unpinProject,
} from "../lib/api";
import { useLayoutContext } from "../components/layout/AppLayout";
import type { Conversation, GenerationResult, Project } from "../types";
import ProjectSummaryCards from "../components/projects/ProjectSummaryCards";
import ProjectImagePanel from "../components/projects/ProjectImagePanel";
import ProjectNameDialog from "../components/projects/ProjectNameDialog";
import ProjectActionsMenu from "../components/projects/ProjectActionsMenu";
import ConfirmDialog from "../components/common/ConfirmDialog";

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

  useEffect(() => {
    setActiveProjectId(projectId);
    setActiveConversationId(null);
  }, [projectId, setActiveConversationId, setActiveProjectId]);

  useEffect(() => {
    getProjects(false).then((items) => {
      setProject(items.find((item) => item.id === projectId && item.id !== "default") ?? null);
    }).catch(() => {
      setProject(null);
    });
    getConversations(undefined, projectId, false).then(setConversations).catch(() => {
      setConversations([]);
    });
    searchGenerations(undefined, 1, false, {}, projectId).then((result) => {
      setResults(result.generations);
      setTotal(result.total);
      setPage(result.page);
      setPageSize(result.page_size);
    }).catch(() => {
      setResults([]);
      setTotal(0);
    });
  }, [projectId]);

  const recentModels = useMemo(
    () => Array.from(new Set(results.map((result) => result.generation.engine))).slice(0, 2),
    [results],
  );

  function refreshProject(currentProject: Project) {
    getProjects(false)
      .then((items) => {
        setProject(items.find((item) => item.id === currentProject.id && item.id !== "default") ?? currentProject);
      })
      .catch((error) => {
        console.error(error);
      });
  }

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
      refreshProject(currentProject);
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
      refreshProject(currentProject);
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
      refreshProject({ ...project, name });
    } catch {
      setRenameError(t("projectDialog.renameError"));
    } finally {
      setRenameLoading(false);
    }
  }

  if (!project) {
    return <div className="p-8 text-[14px] text-muted">{t("projects.notFound")}</div>;
  }

  return (
    <div className="h-full overflow-y-auto px-8 py-8">
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
            onClick={() => navigate("/generate")}
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
        <ProjectSummaryCards project={project} recentModels={recentModels} />
      </div>

      <section className="mt-8">
        <h2 className="text-[18px] font-semibold text-foreground">{t("projects.recentConversations")}</h2>
        <div className="mt-4 grid gap-3">
          {conversations.length === 0 ? (
            <div className="rounded-[14px] border border-border-subtle bg-surface p-4 text-[13px] text-muted">
              {t("projects.emptyConversations")}
            </div>
          ) : (
            conversations.slice(0, 6).map((conversation) => (
              <button
                key={conversation.id}
                onClick={() => {
                  setActiveProjectId(project.id);
                  setActiveConversationId(conversation.id);
                  navigate("/generate");
                }}
                className="rounded-[14px] border border-border-subtle bg-surface p-4 text-left"
              >
                <div className="text-[14px] font-medium text-foreground">{conversation.title}</div>
                <div className="mt-1 text-[12px] text-muted">{conversation.generation_count} images</div>
              </button>
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
        />
      </section>
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
