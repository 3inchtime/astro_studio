import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { motion } from "framer-motion";
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
import ProjectNameDialog from "../components/projects/ProjectNameDialog";
import ConfirmDialog from "../components/common/ConfirmDialog";

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

  useEffect(() => {
    getProjects(false)
      .then(async (items) => {
        const filtered = items.filter((p) => p.id !== "default");
        // Fetch preview images for each project
        const withImages = await Promise.all(
          filtered.map(async (project) => {
            try {
              const result = await searchGenerations(undefined, 1, false, {}, project.id);
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
        setProjects(withImages);
        setLoadError(false);
      })
      .catch(() => {
        setProjects([]);
        setLoadError(true);
      })
      .finally(() => setLoading(false));
  }, []);

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
        list.sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime());
        break;
      case "all":
      default:
        // Pinned first, then by updated_at
        list.sort((a, b) => {
          if (a.pinned_at && !b.pinned_at) return -1;
          if (!a.pinned_at && b.pinned_at) return 1;
          return new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime();
        });
        break;
    }

    return list;
  }, [projects, filter, searchQuery]);

  const stats = useMemo(() => {
    const totalImages = projects.reduce((sum, p) => sum + p.image_count, 0);
    const totalChats = projects.reduce((sum, p) => sum + p.conversation_count, 0);
    const pinned = projects.filter((p) => p.pinned_at).length;
    return { totalProjects: projects.length, totalImages, totalChats, pinned };
  }, [projects]);

  async function handleCreateProject(name: string) {
    setCreateLoading(true);
    setCreateError(null);
    try {
      const project = await createProject(name);
      setCreateDialogOpen(false);
      navigate(`/projects/${project.id}`);
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

  async function handleTogglePin() {
    if (!actionProject || actionPending) return;
    setActionPending(true);
    try {
      if (actionProject.pinned_at) {
        await unpinProject(actionProject.id);
        setProjects((prev) =>
          prev.map((p) =>
            p.id === actionProject.id ? { ...p, pinned_at: null } : p,
          ),
        );
      } else {
        await pinProject(actionProject.id);
        setProjects((prev) =>
          prev.map((p) =>
            p.id === actionProject.id
              ? { ...p, pinned_at: new Date().toISOString() }
              : p,
          ),
        );
      }
    } catch {
      // ignore
    } finally {
      setActionPending(false);
      setActionProjectId(null);
    }
  }

  async function handleArchive() {
    if (!actionProject || actionPending) return;
    setActionPending(true);
    try {
      await archiveProject(actionProject.id);
      setProjects((prev) => prev.filter((p) => p.id !== actionProject.id));
    } catch {
      // ignore
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

  function getMosaicLayout(index: number, imageCount: number) {
    if (imageCount === 0) return "empty";
    const layouts = ["layout-0", "layout-1", "layout-2", "layout-3"] as const;
    return layouts[index % 4];
  }

  return (
    <div className="flex h-full flex-col overflow-hidden bg-transparent">
      {/* Filter bar */}
      <div className="studio-toolbar flex shrink-0 items-center gap-2 px-6 py-2.5">
        {(["all", "pinned", "mostImages", "recent"] as Filter[]).map((f) => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            className={`focus-ring cursor-pointer rounded-[20px] border px-3 py-1 text-[12px] font-medium transition-colors ${
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
          <Search size={13} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder={t("projects.searchPlaceholder")}
            className="studio-input focus-ring h-7 w-48 rounded-[8px] pl-7 pr-2.5 text-[12px] placeholder:text-muted"
          />
        </div>
        <button
          onClick={() => {
            setCreateError(null);
            setCreateDialogOpen(true);
          }}
          className="studio-control-primary focus-ring flex h-7 items-center gap-1.5 rounded-[8px] px-3 text-[12px] font-medium"
        >
          <Plus size={13} />
          {t("sidebar.newProject")}
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto px-6 py-5">
        {loading ? (
          <div className="flex items-center justify-center py-16 text-[13px] text-muted">
            {t("projects.loading")}
          </div>
        ) : loadError ? (
          <div className="flex items-center justify-center py-16 text-[13px] text-error">
            {t("projects.loadError")}
          </div>
        ) : filteredProjects.length === 0 ? (
          <div className="flex flex-col items-center justify-center py-16 gap-3">
            <div className="flex h-14 w-14 items-center justify-center rounded-[14px] bg-primary/8">
              <ImageIcon size={24} className="text-primary" />
            </div>
            <div className="text-[15px] font-semibold text-foreground">
              {searchQuery ? t("projects.noResults") : t("projects.emptyTitle")}
            </div>
            <div className="text-[12px] text-muted max-w-[260px] text-center leading-relaxed">
              {searchQuery ? t("projects.noResultsHint") : t("projects.emptyHint")}
            </div>
            {!searchQuery && (
              <button
                onClick={() => {
                  setCreateError(null);
                  setCreateDialogOpen(true);
                }}
                className="studio-control-primary focus-ring mt-1 flex items-center gap-1.5 rounded-[10px] px-4 py-2 text-[12px] font-medium"
              >
                <Plus size={13} />
                {t("sidebar.newProject")}
              </button>
            )}
          </div>
        ) : (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(260px,1fr))] gap-3.5">
            {filteredProjects.map((project, index) => (
              <ProjectCard
                key={project.id}
                project={project}
                index={index}
                mosaicLayout={getMosaicLayout(index, project.previewImages.length)}
                onOpen={() => navigate(`/projects/${project.id}`)}
                onAction={(action) => {
                  setActionProjectId(project.id);
                  if (action === "rename") {
                    setRenameError(null);
                    setRenameDialogOpen(true);
                  } else if (action === "delete") {
                    setDeleteError(null);
                    setDeleteDialogOpen(true);
                  } else if (action === "pin") {
                    handleTogglePin();
                  } else if (action === "archive") {
                    handleArchive();
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
        <div className="studio-status-bar flex shrink-0 items-center gap-5 px-6 py-2 text-[11px]">
          <span>
            <span className="font-semibold text-foreground/70 tabular-nums font-[family-name:var(--font-mono)]">
              {stats.totalProjects}
            </span>{" "}
            {t("projects.stats.projects")}
          </span>
          <span>
            <span className="font-semibold text-foreground/70 tabular-nums font-[family-name:var(--font-mono)]">
              {stats.totalImages}
            </span>{" "}
            {t("projects.stats.images")}
          </span>
          <span>
            <span className="font-semibold text-foreground/70 tabular-nums font-[family-name:var(--font-mono)]">
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
  index: number;
  mosaicLayout: string;
  onOpen: () => void;
  onAction: (action: "rename" | "pin" | "archive" | "delete") => void;
  t: (key: string, opts?: Record<string, unknown>) => string;
}

function ProjectCard({ project, index, mosaicLayout, onOpen, onAction, t }: ProjectCardProps) {
  const [showActions, setShowActions] = useState(false);
  const images = project.previewImages;

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: index * 0.03, duration: 0.3, ease: [0.22, 1, 0.36, 1] }}
      className="studio-card group relative cursor-pointer overflow-hidden rounded-[14px] hover:studio-card-hover"
      onClick={onOpen}
    >
      {/* Mosaic Preview */}
      <div className={`mosaic-preview mosaic-${mosaicLayout}`}>
        {images.length === 0 ? (
          <div className="col-span-full flex items-center justify-center gap-1.5 text-[11px] text-muted">
            <ImageIcon size={14} className="opacity-40" />
            {t("projects.noImages")}
          </div>
        ) : (
          images.slice(0, 5).map((path, i) => (
            <div key={i} className="mosaic-cell overflow-hidden">
              <img
                src={toAssetUrl(path)}
                alt=""
                className="h-full w-full object-cover"
                loading="lazy"
              />
            </div>
          ))
        )}
      </div>

      {/* Card Body */}
      <div className="px-4 py-3">
        <div className="flex items-center gap-2 mb-2">
          <div className="flex-1 min-w-0 text-[14px] font-semibold text-foreground truncate">
            {project.name}
          </div>
          {project.pinned_at && (
            <span className="inline-flex shrink-0 items-center gap-1 rounded-[5px] bg-primary/8 px-1.5 py-px text-[10px] font-medium text-primary">
              <Pin size={9} />
              {t("projects.pinned")}
            </span>
          )}
        </div>
        <div className="flex items-center gap-3.5 text-[11px] text-muted">
          <span className="flex items-center gap-1">
            <ImageIcon size={12} />
            <span className="font-semibold text-foreground/60 tabular-nums font-[family-name:var(--font-mono)]">
              {project.image_count}
            </span>
            {t("projects.stats.images")}
          </span>
          <span className="flex items-center gap-1">
            <MessageSquare size={12} />
            <span className="font-semibold text-foreground/60 tabular-nums font-[family-name:var(--font-mono)]">
              {project.conversation_count}
            </span>
            {t("projects.stats.chats")}
          </span>
        </div>
      </div>

      {/* Action Tray */}
      <div className="absolute bottom-3 right-3 z-10 flex items-center justify-center gap-2 opacity-0 transition-opacity duration-200 group-hover:opacity-100 focus-within:opacity-100">
        <button
          onClick={(e) => {
            e.stopPropagation();
            onOpen();
          }}
          className="studio-control-primary focus-ring flex items-center gap-1.5 rounded-[8px] px-4 py-2 text-[12px] font-medium"
        >
          {t("projects.open")}
        </button>
        <div className="relative">
          <button
            onClick={(e) => {
              e.stopPropagation();
              setShowActions((c) => !c);
            }}
            className="studio-floating-panel focus-ring flex h-8 w-8 items-center justify-center rounded-[8px] text-foreground/80 transition-colors hover:text-foreground"
          >
            <MoreHorizontal size={15} />
          </button>
          {showActions && (
            <div
              className="studio-floating-panel absolute right-0 top-10 z-20 w-40 overflow-hidden rounded-[10px] py-1"
              onClick={(e) => e.stopPropagation()}
            >
              <button
                onClick={() => {
                  setShowActions(false);
                  onAction("rename");
                }}
                className="focus-ring flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] text-foreground/75 transition-colors hover:bg-subtle hover:text-foreground"
              >
                <Pencil size={13} />
                <span>{t("sidebar.rename")}</span>
              </button>
              <button
                onClick={() => {
                  setShowActions(false);
                  onAction("pin");
                }}
                className="focus-ring flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] text-foreground/75 transition-colors hover:bg-subtle hover:text-foreground"
              >
                {project.pinned_at ? <PinOff size={13} /> : <Pin size={13} />}
                <span>{project.pinned_at ? t("projects.unpin") : t("projects.pin")}</span>
              </button>
              <button
                onClick={() => {
                  setShowActions(false);
                  onAction("archive");
                }}
                className="focus-ring flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] text-foreground/75 transition-colors hover:bg-subtle hover:text-foreground"
              >
                <Archive size={13} />
                <span>{t("sidebar.archive")}</span>
              </button>
              <button
                onClick={() => {
                  setShowActions(false);
                  onAction("delete");
                }}
                className="focus-ring flex w-full items-center gap-2 px-3 py-2 text-left text-[12px] text-error transition-colors hover:bg-error/8"
              >
                <Trash2 size={13} />
                <span>{t("sidebar.delete")}</span>
              </button>
            </div>
          )}
        </div>
      </div>
    </motion.div>
  );
}
