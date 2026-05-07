import type { Project } from "../../types";

export default function ProjectSummaryCards({
  project,
  recentModels,
}: {
  project: Project;
  recentModels: string[];
}) {
  return (
    <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
      <div className="rounded-[14px] border border-border-subtle bg-surface p-4">
        <div className="text-[10px] uppercase tracking-[0.08em] text-muted">Scope</div>
        <div className="mt-2 text-[24px] font-semibold text-foreground">{project.conversation_count}</div>
        <div className="text-[12px] text-muted">active conversations</div>
      </div>
      <div className="rounded-[14px] border border-border-subtle bg-surface p-4">
        <div className="text-[10px] uppercase tracking-[0.08em] text-muted">Output</div>
        <div className="mt-2 text-[24px] font-semibold text-foreground">{project.image_count}</div>
        <div className="text-[12px] text-muted">saved images</div>
      </div>
      <div className="rounded-[14px] border border-border-subtle bg-surface p-4">
        <div className="text-[10px] uppercase tracking-[0.08em] text-muted">Models</div>
        <div className="mt-2 text-[14px] font-semibold text-foreground">
          {recentModels.join(", ") || "None yet"}
        </div>
      </div>
      <div className="rounded-[14px] border border-border-subtle bg-surface p-4">
        <div className="text-[10px] uppercase tracking-[0.08em] text-muted">Updated</div>
        <div className="mt-2 text-[14px] font-semibold text-foreground">{project.updated_at}</div>
      </div>
    </div>
  );
}
