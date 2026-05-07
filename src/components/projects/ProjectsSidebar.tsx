import { useCallback, useEffect, useState } from "react";
import { FolderKanban, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";
import { createProject, getProjects } from "../../lib/api";
import type { Project } from "../../types";

interface ProjectsSidebarProps {
  activeProjectId: string | null;
  onSelectProject: (id: string | null) => void;
  onProjectCreated: (id: string) => void;
}

export default function ProjectsSidebar({
  activeProjectId,
  onSelectProject,
  onProjectCreated,
}: ProjectsSidebarProps) {
  const { t } = useTranslation();
  const [projects, setProjects] = useState<Project[]>([]);

  const loadProjects = useCallback(async () => {
    const items = await getProjects(false);
    setProjects(items.filter((project) => project.id !== "default"));
  }, []);

  useEffect(() => {
    loadProjects().catch(() => {});
  }, [loadProjects]);

  const handleCreateProject = useCallback(async () => {
    const name = window.prompt(t("sidebar.newProject"));
    if (!name?.trim()) return;
    const project = await createProject(name.trim());
    onProjectCreated(project.id);
    await loadProjects();
  }, [loadProjects, onProjectCreated, t]);

  return (
    <div className="flex h-full flex-col">
      <div className="border-b border-border-subtle px-4 py-4">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-2 text-[13px] font-semibold text-foreground">
            <FolderKanban size={13} strokeWidth={1.8} />
            <span>{t("projects.directory")}</span>
          </div>
          <button
            type="button"
            onClick={handleCreateProject}
            className="flex h-8 w-8 items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-subtle hover:text-foreground"
            aria-label={t("sidebar.newProject")}
            title={t("sidebar.newProject")}
          >
            <Plus size={16} strokeWidth={1.8} />
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-2">
        <button
          type="button"
          onClick={() => onSelectProject(null)}
          className={`mb-1 flex w-full items-center gap-2 rounded-[8px] px-3 py-2 text-left text-sm transition-colors ${
            activeProjectId === null
              ? "bg-primary/10 text-primary"
              : "text-muted hover:bg-subtle hover:text-foreground"
          }`}
        >
          <FolderKanban size={15} strokeWidth={1.8} />
          <span>{t("projects.directory")}</span>
        </button>

        <div className="mt-2 space-y-1">
          {projects.map((project) => (
            <button
              key={project.id}
              type="button"
              onClick={() => onSelectProject(project.id)}
              className={`flex w-full items-center gap-2 rounded-[8px] px-3 py-2 text-left text-sm transition-colors ${
                activeProjectId === project.id
                  ? "bg-primary/10 text-primary"
                  : "text-muted hover:bg-subtle hover:text-foreground"
              }`}
            >
              <span className="truncate">{project.name}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
