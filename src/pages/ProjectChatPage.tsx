import { useParams } from "react-router-dom";
import GeneratePage from "./GeneratePage";

export default function ProjectChatPage() {
  const { projectId } = useParams();

  if (!projectId) return null;

  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 min-h-0">
        <GeneratePage />
      </div>
    </div>
  );
}
