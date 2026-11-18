import { useLocation } from "react-router-dom";
import { getRouteContext, ROUTES } from "../app/routes/contracts";
import { useLayoutContext } from "../components/layout/AppLayout";
import GenerationSession from "../features/generation/GenerationSession";
import { conversationDraftKey } from "../lib/providerSettingsDraft";

export default function GeneratePage() {
  const layout = useLayoutContext();
  const location = useLocation();
  const route = getRouteContext(location.pathname ?? "");
  const project = route.kind === "chat" ? route.projectId : location.pathname === ROUTES.generate ? null : layout.activeProjectId;
  const launch = location.state as { activateConversationId?: string } | null;
  const sameProject = (layout.activeProjectId ?? "default") === (project ?? "default");
  const selectedConversation = sameProject ? layout.activeConversationId : null;
  const conversation = route.kind === "chat"
    ? route.conversationId ?? selectedConversation
    : launch?.activateConversationId ?? selectedConversation;
  const scope = conversationDraftKey(project, conversation);
  return <GenerationSession key={scope} scope={scope} activeProjectId={project} activeConversationId={conversation} />;
}
