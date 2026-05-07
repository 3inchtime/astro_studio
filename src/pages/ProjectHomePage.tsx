import { useEffect, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import { archiveProject, getConversations, getProjects, renameProject, searchGenerations } from "../lib/api";
import { useLayoutContext } from "../components/layout/AppLayout";
import type { Conversation, GenerationResult, Project } from "../types";
import ProjectSummaryCards from "../components/projects/ProjectSummaryCards";
import ProjectImagePanel from "../components/projects/ProjectImagePanel";

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

  async function handleProjectAction(action: "rename" | "archive") {
    if (!project) return;

    if (action === "rename") {
      const name = window.prompt(t("sidebar.renameProject"), project.name);
      if (!name?.trim() || name.trim() === project.name) return;
      await renameProject(project.id, name.trim());
      const items = await getProjects(false);
      setProject(items.find((item) => item.id === project.id) ?? project);
      setShowActions(false);
      return;
    }

    await archiveProject(project.id);
    setShowActions(false);
    navigate("/projects");
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
          {showActions ? (
            <div className="absolute right-0 top-[44px] z-10 w-40 overflow-hidden rounded-[10px] border border-border-subtle bg-surface py-1 shadow-card">
              <button className="w-full px-3 py-2 text-left text-[12px]" onClick={() => void handleProjectAction("rename")}>
                {t("sidebar.rename")}
              </button>
              <button className="w-full px-3 py-2 text-left text-[12px]" onClick={() => void handleProjectAction("archive")}>
                {t("sidebar.archive")}
              </button>
            </div>
          ) : null}
          <button
            onClick={() => navigate("/generate")}
            className="rounded-[12px] bg-primary px-4 py-2 text-[12px] font-medium text-white"
          >
            {t("projects.newConversation")}
          </button>
        </div>
      </div>

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
    </div>
  );
}
