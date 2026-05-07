import { useEffect } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ArrowLeft } from "lucide-react";
import { useLayoutContext } from "../components/layout/AppLayout";
import GeneratePage from "./GeneratePage";

export default function ProjectChatPage() {
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
          onClick={() => navigate(`/projects/${projectId}`)}
          className="flex h-8 w-8 items-center justify-center rounded-[8px] text-muted transition-colors hover:bg-subtle hover:text-foreground"
        >
          <ArrowLeft size={18} strokeWidth={1.8} />
        </button>
        <span className="text-[13px] font-medium text-foreground">Chat</span>
      </div>
      <div className="flex-1 min-h-0">
        <GeneratePage />
      </div>
    </div>
  );
}
