import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import {
  Archive,
  Image as ImageIcon,
  MessageSquare,
  MoreHorizontal,
  Pencil,
  Pin,
  PinOff,
  Plus,
  Search,
  Trash2,
} from "lucide-react";
import {
  archiveProject,
  createProject,
  deleteProject,
  getProjects,
  pinProject,
  renameProject,
  searchGenerations,
  toAssetUrl,
  unpinProject,
} from "../lib/api";
import type { Project } from "../types";
import CollectionStatus from "../components/gallery/CollectionStatus";
import ProjectNameDialog from "../components/projects/ProjectNameDialog";
import ConfirmDialog from "../components/common/ConfirmDialog";
import { formatConversationTime } from "../lib/utils";
import { ROUTES } from "../app/routes/contracts";

type Filter = "all" | "pinned" | "mostImages" | "recent";

interface ProjectWithImages extends Project {
  previewImages: string[];
}

export default function ProjectsPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [projects, setProjects] = useState<ProjectWithImages[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState(false);
  const [filter, setFilter] = useState<Filter>("all");
  const [searchQuery, setSearchQuery] = useState("");
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [createLoading, setCreateLoading] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);

  // Project action state
  const [actionProjectId, setActionProjectId] = useState<string | null>(null);
  const [renameDialogOpen, setRenameDialogOpen] = useState(false);
  const [renameLoading, setRenameLoading] = useState(false);
  const [renameError, setRenameError] = useState<string | null>(null);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deleteLoading, setDeleteLoading] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [actionPending, setActionPending] = useState(false);
  const [actionError, setActionError] = useState(false);

  const [reloadVersion, setReloadVersion] = useState(0);
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setLoadError(false);
    getProjects(false)
      .then(async (items) => {
        const filtered = items.filter((p) => p.id !== "default");
        // Fetch preview images for each project
        const withImages = await Promise.all(
          filtered.map(async (project) => {
            try {
              const result = await searchGenerations(
                undefined,
                1,
                false,
                {},
                project.id,
              );
              const paths = result.generations
                .flatMap((g) => g.images)
                .slice(0, 5)
                .map((img) => img.thumbnail_path);
              return { ...project, previewImages: paths };
            } catch {
              return { ...project, previewImages: [] };
            }
          }),
        );
        if (cancelled) return;
        setProjects(withImages);
        setLoadError(false);
      })
      .catch(() => {
        if (cancelled) return;
        setProjects([]);
        setLoadError(true);
      })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [reloadVersion]);

  const filteredProjects = useMemo(() => {
    let list = [...projects];

    // Search
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      list = list.filter((p) => p.name.toLowerCase().includes(q));
    }

    // Filter
    switch (filter) {
      case "pinned":
        list = list.filter((p) => p.pinned_at);
        break;
      case "mostImages":
        list.sort((a, b) => b.image_count - a.image_count);
        break;
      case "recent":
        list.sort(
          (a, b) =>
            new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime(),
        );
        break;
      case "all":
      default:
        // Pinned first, then by updated_at
        list.sort((a, b) => {
          if (a.pinned_at && !b.pinned_at) return -1;
          if (!a.pinned_at && b.pinned_at) return 1;
          return (
            new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime()
          );
        });
        break;
    }

    return list;
  }, [projects, filter, searchQuery]);

  const stats = useMemo(() => {
    const totalImages = projects.reduce((sum, p) => sum + p.image_count, 0);
    const totalChats = projects.reduce(
      (sum, p) => sum + p.conversation_count,
      0,
    );
    const pinned = projects.filter((p) => p.pinned_at).length;
    return { totalProjects: projects.length, totalImages, totalChats, pinned };
  }, [projects]);

  async function handleCreateProject(name: string) {
    setCreateLoading(true);
    setCreateError(null);
    try {
      const project = await createProject(name);
      setCreateDialogOpen(false);
      navigate(ROUTES.projectHome(project.id));
    } catch {
      setCreateError(t("projectDialog.createError"));
    } finally {
      setCreateLoading(false);
    }
  }

  const actionProject = projects.find((p) => p.id === actionProjectId) ?? null;

  async function handleRenameProject(name: string) {
    if (!actionProject) return;
    if (name === actionProject.name) {
      setRenameDialogOpen(false);
      setRenameError(null);
      return;
    }
    setRenameLoading(true);
    setRenameError(null);
    try {
      await renameProject(actionProject.id, name);
      setRenameDialogOpen(false);
      setProjects((prev) =>
        prev.map((p) => (p.id === actionProject.id ? { ...p, name } : p)),
      );
    } catch {
      setRenameError(t("projectDialog.renameError"));
    } finally {
      setRenameLoading(false);
    }
  }

  async function handleTogglePin(
    project: ProjectWithImages | null = actionProject,
  ) {
    if (!project || actionPending) return;
    setActionPending(true);
    setActionError(false);
    try {
      if (project.pinned_at) {
        await unpinProject(project.id);
        setProjects((prev) =>
          prev.map((p) =>
            p.id === project.id ? { ...p, pinned_at: null } : p,
          ),
        );
      } else {
        await pinProject(project.id);
        setProjects((prev) =>
          prev.map((p) =>
            p.id === project.id
              ? { ...p, pinned_at: new Date().toISOString() }
              : p,
          ),
        );
      }
    } catch {
      setActionError(true);
    } finally {
      setActionPending(false);
      setActionProjectId(null);
    }
  }

  async function handleArchive(
    project: ProjectWithImages | null = actionProject,
  ) {
    if (!project || actionPending) return;
    setActionPending(true);
    setActionError(false);
    try {
      await archiveProject(project.id);
      setProjects((prev) => prev.filter((p) => p.id !== project.id));
    } catch {
      setActionError(true);
    } finally {
      setActionPending(false);
      setActionProjectId(null);
    }
  }

  async function handleDeleteProject() {
    if (!actionProject) return;
    setDeleteLoading(true);
    setDeleteError(null);
    try {
      await deleteProject(actionProject.id);
      setDeleteDialogOpen(false);
      setProjects((prev) => prev.filter((p) => p.id !== actionProject.id));
      setActionProjectId(null);
    } catch {
      setDeleteError(t("projects.deleteError"));
    } finally {
      setDeleteLoading(false);
    }
  }

  return (
    <div className="flex h-full flex-col overflow-hidden bg-background">
      <div className="flex items-center justify-between px-6 pt-6 pb-4">
        <div><h1 className="text-xl font-semibold tracking-tight text-foreground">{t("studio.project.title", { defaultValue: "Projects" })}</h1>
        <p className="mt-1 text-sm text-muted">{t("studio.project.subtitle", { defaultValue: "Keep your conversations and images together." })}</p></div>
      </div>
      {/* Filter bar */}
      <div className="studio-toolbar flex shrink-0 flex-wrap items-center gap-2 px-6 py-2.5">
        {(["all", "pinned", "mostImages", "recent"] as Filter[]).map((f) => (
          <button
            key={f}
            aria-pressed={filter === f}
            onClick={() => setFilter(f)}
            className={`focus-ring cursor-pointer rounded-[8px] border px-3 py-2 text-xs font-medium transition-colors ${
              filter === f
                ? "border-primary/14 bg-primary/10 text-primary"
                : "border-transparent text-muted hover:border-border-subtle hover:bg-surface/70 hover:text-foreground"
            }`}
          >
            {t(`projects.filter.${f}`)}
          </button>
        ))}
        <div className="flex-1" />
        <div className="relative">
          <Search
            size={13}
            className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted"
          />
          <input
            aria-label={t("projects.searchPlaceholder")}
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder={t("projects.searchPlaceholder")}
            className="studio-input focus-ring h-9 w-48 rounded-[8px] pl-7 pr-2.5 text-xs placeholder:text-muted"
          />
        </div>
        <button
          onClick={() => {
            setCreateError(null);
            setCreateDialogOpen(true);
          }}
          className="studio-control-primary focus-ring flex h-9 items-center gap-1.5 rounded-[8px] px-3 text-xs font-medium"
        >
          <Plus size={13} />
          {t("sidebar.newProject")}
        </button>
      </div>

      {actionError && <p role="alert" className="mx-6 mt-3 rounded-lg border border-error/20 bg-error/5 px-3 py-2 text-sm text-error">{t("studio.project.actionFailed", { defaultValue: "Could not update this project. Try the action again." })}</p>}

      {/* Content */}
      <div className="flex-1 overflow-y-auto px-6 py-5">
        {loading || loadError ? (
          <CollectionStatus errorTitle={t("projects.loadError")} loading={loading} error={loadError} onRetry={() => setReloadVersion((value) => value + 1)} />
        ) : filteredProjects.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 gap-3">
            <div className="flex h-14 w-14 items-center justify-center rounded-[12px] bg-primary/8">
              <ImageIcon size={24} className="text-primary" />
            </div>
            <div className="text-[20px] font-semibold text-foreground">
              {searchQuery ? t("projects.noResults") : t("projects.emptyTitle")}
            </div>
            <div className="text-xs text-muted max-w-[260px] text-center leading-relaxed">
              {searchQuery
                ? t("projects.noResultsHint")
                : t("projects.emptyHint")}
            </div>
            {!searchQuery && (
              <button
                onClick={() => {
                  setCreateError(null);
                  setCreateDialogOpen(true);
                }}
                className="studio-control-primary focus-ring mt-1 flex items-center gap-1.5 rounded-[8px] px-4 py-2 text-xs font-medium"
              >
                <Plus size={13} />
                {t("sidebar.newProject")}
              </button>
            )}
          </div>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(260px,1fr))] gap-3.5">
            {filteredProjects.map((project) => (
              <ProjectCard
                key={project.id}
                project={project}
                busy={actionPending}
                onOpen={() => navigate(ROUTES.projectHome(project.id))}
                onAction={(action) => {
                  setActionProjectId(project.id);
                  if (action === "rename") {
                    setRenameError(null);
                    setRenameDialogOpen(true);
                  } else if (action === "delete") {
                    setDeleteError(null);
                    setDeleteDialogOpen(true);
                  } else if (action === "pin") {
                    void handleTogglePin(project);
                  } else if (action === "archive") {
                    void handleArchive(project);
                  }
                }}
                t={t}
              />
            ))}
          </div>
        )}
      </div>

      {/* Stats footer */}
      {!loading && !loadError && projects.length > 0 && (
        <div className="studio-status-bar flex shrink-0 items-center gap-5 px-6 py-2 text-xs">
          <span>
            <span className="font-semibold text-muted tabular-nums font-[family-name:var(--font-mono)]">
              {stats.totalProjects}
            </span>{" "}
            {t("projects.stats.projects")}
          </span>
          <span>
            <span className="font-semibold text-muted tabular-nums font-[family-name:var(--font-mono)]">
              {stats.totalImages}
            </span>{" "}
            {t("projects.stats.images")}
          </span>
          <span>
            <span className="font-semibold text-muted tabular-nums font-[family-name:var(--font-mono)]">
              {stats.totalChats}
            </span>{" "}
            {t("projects.stats.conversations")}
          </span>
        </div>
      )}

      {/* Dialogs */}
      <ProjectNameDialog
        open={createDialogOpen}
        title={t("projectDialog.createTitle")}
        label={t("projectDialog.nameLabel")}
        submitLabel={t("projectDialog.createSubmit")}
        cancelLabel={t("projectDialog.cancel")}
        requiredMessage={t("projectDialog.nameRequired")}
        error={createError}
        loading={createLoading}
        onSubmit={(name) => void handleCreateProject(name)}
        onCancel={() => {
          if (!createLoading) {
            setCreateDialogOpen(false);
            setCreateError(null);
          }
        }}
      />
      <ProjectNameDialog
        open={renameDialogOpen}
        title={t("projectDialog.renameTitle")}
        label={t("projectDialog.nameLabel")}
        initialName={actionProject?.name ?? ""}
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

/* ============================================================
   Project Card Component
   ============================================================ */

interface ProjectCardProps {
  project: ProjectWithImages;
  busy: boolean;
  onOpen: () => void;
  onAction: (action: "rename" | "pin" | "archive" | "delete") => void;
  t: (key: string, opts?: Record<string, unknown>) => string;
}

function ProjectCard({
  project,
  busy,
  onOpen,
  onAction,
  t,
}: ProjectCardProps) {
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

  const images = project.previewImages;

  return (
    <article className="studio-card group relative rounded-xl transition-colors duration-[140ms] hover:border-border">
      <button type="button" aria-label={project.name} onClick={onOpen} className="focus-ring block w-full rounded-xl text-left">
        <div className="aspect-[16/10] overflow-hidden rounded-t-xl bg-subtle">
          {images[0] ? (
            <img src={toAssetUrl(images[0])} alt="" className="h-full w-full object-cover" loading="lazy" />
          ) : (
            <div className="flex h-full items-center justify-center gap-2 text-sm text-muted"><ImageIcon size={20} />{t("projects.noImages")}</div>
          )}
        </div>
        <div className="space-y-2 px-4 pt-4">
          <div className="flex min-w-0 items-center gap-2">
            <h2 className="min-w-0 flex-1 truncate text-base font-semibold text-foreground">{project.name}</h2>
            {project.pinned_at && <span className="inline-flex shrink-0 items-center gap-1 text-xs text-primary"><Pin size={12} />{t("projects.pinned")}</span>}
          </div>
          <div className="flex flex-wrap items-center gap-4 text-xs text-muted">
            <span className="flex items-center gap-1.5"><ImageIcon size={13} /><span className="tabular-nums">{project.image_count}</span>{t("projects.stats.images")}</span>
            <span className="flex items-center gap-1.5"><MessageSquare size={13} /><span className="tabular-nums">{project.conversation_count}</span>{t("projects.stats.chats")}</span>
          </div>
        </div>
      </button>
      <div className="flex min-w-0 items-center justify-between gap-2 px-4 pb-3 pt-2">
        <time dateTime={project.updated_at} className="min-w-0 truncate text-xs text-muted">{t("studio.project.updated", { defaultValue: "Updated {{date}}", date: formatConversationTime(project.updated_at) })}</time>
        <div ref={actionsContainerRef} className="relative shrink-0">
          <button ref={actionsTriggerRef} type="button" onClick={() => setShowActions((current) => !current)} disabled={busy} aria-label={t("projects.manage")} aria-expanded={showActions} className="focus-ring flex h-9 w-9 items-center justify-center rounded-lg text-muted transition-colors duration-[140ms] hover:bg-subtle hover:text-foreground disabled:opacity-50"><MoreHorizontal size={17} /></button>
          {showActions && (
            <div
              className="studio-floating-panel absolute right-0 bottom-10 z-20 w-40 overflow-hidden rounded-[8px] py-1"
              onClick={(e) => e.stopPropagation()}
            >
              <button
                disabled={busy}
                onClick={() => {
                  setShowActions(false);
                  onAction("rename");
                }}
                className="focus-ring flex w-full items-center gap-2 px-3 py-2 text-left text-xs text-foreground transition-colors hover:bg-subtle hover:text-foreground"
              >
                <Pencil size={13} />
                <span>{t("sidebar.rename")}</span>
              </button>
              <button
                disabled={busy}
                onClick={() => {
                  setShowActions(false);
                  onAction("pin");
                }}
                className="focus-ring flex w-full items-center gap-2 px-3 py-2 text-left text-xs text-foreground transition-colors hover:bg-subtle hover:text-foreground"
              >
                {project.pinned_at ? <PinOff size={13} /> : <Pin size={13} />}
                <span>
                  {project.pinned_at ? t("projects.unpin") : t("projects.pin")}
                </span>
              </button>
              <button
                disabled={busy}
                onClick={() => {
                  setShowActions(false);
                  onAction("archive");
                }}
                className="focus-ring flex w-full items-center gap-2 px-3 py-2 text-left text-xs text-foreground transition-colors hover:bg-subtle hover:text-foreground"
              >
                <Archive size={13} />
                <span>{t("sidebar.archive")}</span>
              </button>
              <button
                disabled={busy}
                onClick={() => {
                  setShowActions(false);
                  onAction("delete");
                }}
                className="focus-ring flex w-full items-center gap-2 px-3 py-2 text-left text-xs text-error transition-colors hover:bg-error/8"
              >
                <Trash2 size={13} />
                <span>{t("sidebar.delete")}</span>
              </button>
            </div>
          )}
        </div>
      </div>
    </article>
  );
}
