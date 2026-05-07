import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { createProject, getProjects } from "../lib/api";
import type { Project } from "../types";
import ProjectNameDialog from "../components/projects/ProjectNameDialog";

export default function ProjectsPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [projects, setProjects] = useState<Project[]>([]);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [createLoading, setCreateLoading] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);

  useEffect(() => {
    getProjects(false)
      .then((items) => {
        setProjects(items.filter((project) => project.id !== "default"));
      })
      .catch(() => {
        setProjects([]);
      });
  }, []);

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

  return (
    <div className="h-full overflow-y-auto px-8 py-8">
      <div className="flex items-center justify-between gap-4">
        <h1 className="text-[28px] font-semibold text-foreground">{t("projects.title")}</h1>
        <button
          onClick={() => {
            setCreateError(null);
            setCreateDialogOpen(true);
          }}
          className="rounded-[12px] bg-primary px-4 py-2 text-[12px] font-medium text-white"
        >
          {t("sidebar.newProject")}
        </button>
      </div>
      <div className="mt-6 grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        {projects.map((project) => (
          <button
            key={project.id}
            onClick={() => navigate(`/projects/${project.id}`)}
            className={`rounded-[14px] border p-5 text-left shadow-card transition-transform hover:-translate-y-0.5 ${
              project.pinned_at ? "border-primary/30 bg-primary/6" : "border-border-subtle bg-surface"
            }`}
          >
            <div className="text-[16px] font-semibold text-foreground">{project.name}</div>
            <div className="mt-2 text-[12px] text-muted">
              {project.conversation_count} conversations
            </div>
            <div className="mt-1 text-[12px] text-muted">
              {project.image_count} images
            </div>
          </button>
        ))}
      </div>
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
    </div>
  );
}
