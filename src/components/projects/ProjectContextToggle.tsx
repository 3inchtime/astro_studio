import { PanelLeftOpen } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useLayoutContext } from "../layout/AppLayout";

export default function ProjectContextToggle() {
  const { t } = useTranslation();
  const { toggleContextSidebar, contextSidebarCollapsed, activeWorkspaceDrawer } = useLayoutContext();
  if (!toggleContextSidebar) return null;
  return (
    <button
      type="button"
      aria-label={t("studio.openSidebar", { defaultValue: "Open sidebar" })}
      aria-controls="workspace-sidebar"
      aria-expanded={activeWorkspaceDrawer === "context"}
      className={`studio-control-subtle focus-ring -ml-2 flex h-9 w-9 shrink-0 items-center justify-center rounded-lg ${contextSidebarCollapsed ? "" : "min-[1440px]:hidden"}`}
      onClick={toggleContextSidebar}
    >
      <PanelLeftOpen size={17} />
    </button>
  );
}
