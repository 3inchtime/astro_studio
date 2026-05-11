import { useEffect } from "react";
import { useParams } from "react-router-dom";
import { useLayoutContext } from "../components/layout/AppLayout";
import GeneratePage from "./GeneratePage";

export default function ProjectChatPage() {
  const { projectId, conversationId } = useParams();
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
      <div className="flex-1 min-h-0">
        <GeneratePage />
      </div>
    </div>
  );
}
