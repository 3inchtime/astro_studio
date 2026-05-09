import { useEffect } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ArrowLeft } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useLayoutContext } from "../components/layout/AppLayout";
import GeneratePage from "./GeneratePage";

export default function ProjectChatPage() {
  const { t } = useTranslation();
  const { projectId, conversationId } = useParams();
  const navigate = useNavigate();
  const { setActiveProjectId, setActiveConversationId } = useLayoutContext();

  useEffect(() => {
    if (projectId) {
      setActiveProjectId(projectId);
      setActiveConversationId(conversationId ?? null);
    }
  }, [projectId, conversationId, setActiveProjectId, setActiveConversationId]);

  if (!projectId) return null;

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-3 border-b border-border-subtle px-5 py-3">
        <button
          type="button"
          onClick={() => navigate("/projects")}
          className="inline-flex items-center gap-2 rounded-[10px] border border-border-subtle bg-surface px-3 py-2 text-[12px] font-medium text-foreground/80 transition-colors hover:bg-subtle hover:text-foreground"
          aria-label={t("projects.backToList")}
        >
          <ArrowLeft size={15} strokeWidth={1.9} />
          <span>{t("projects.backToList")}</span>
        </button>
        <span className="text-[13px] font-medium text-foreground">Chat</span>
      </div>
      <div className="flex-1 min-h-0">
        <GeneratePage />
      </div>
    </div>
  );
}
