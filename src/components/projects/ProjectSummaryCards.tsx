import { useTranslation } from "react-i18next";
import type { Project } from "../../types";

export default function ProjectSummaryCards({
  project,
}: {
  project: Project;
}) {
  const { t } = useTranslation();

  return (
    <div className="grid gap-4 md:grid-cols-2">
      <div className="rounded-[14px] border border-border-subtle bg-surface p-4">
        <div className="text-[10px] uppercase tracking-[0.08em] text-muted">
          {t("projects.conversationCount")}
        </div>
        <div className="mt-2 text-[24px] font-semibold text-foreground">
          {project.conversation_count}
        </div>
      </div>
      <div className="rounded-[14px] border border-border-subtle bg-surface p-4">
        <div className="text-[10px] uppercase tracking-[0.08em] text-muted">
          {t("projects.imageCount")}
        </div>
        <div className="mt-2 text-[24px] font-semibold text-foreground">
          {project.image_count}
        </div>
      </div>
    </div>
  );
}
