import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import {
  FolderKanban,
  Heart,
  Image,
  MessageSquareText,
  Palette,
  Settings,
  Sparkles,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { useTheme } from "../../hooks/useTheme";
import { useResizable } from "../../hooks/useResizable";
import appLogo from "../../assets/logo.png";
import { ResizeHandle } from "./ResizeHandle";
import ConversationList from "../sidebar/ConversationList";
import ProjectsSidebar from "../projects/ProjectsSidebar";
import { createConversation } from "../../lib/api";
import { ThemeCardPicker } from "../theme/ThemeCardPicker";
import { getThemeName } from "../../lib/themes";

interface LayoutContextType {
  activeProjectId: string | null;
  setActiveProjectId: (id: string | null) => void;
  activeConversationId: string | null;
  setActiveConversationId: (id: string | null) => void;
  refreshConversations: () => void;
}

export const LayoutContext = createContext<LayoutContextType>({
  activeProjectId: null,
  setActiveProjectId: () => {},
  activeConversationId: null,
  setActiveConversationId: () => {},
  refreshConversations: () => {},
});

export function useLayoutContext() {
  return useContext(LayoutContext);
}

const navItems = [
  { to: "/generate", icon: Sparkles, labelKey: "nav.generate" },
  { to: "/extract", icon: MessageSquareText, labelKey: "nav.extract" },
  { to: "/projects", icon: FolderKanban, labelKey: "nav.projects" },
  { to: "/gallery", icon: Image, labelKey: "nav.gallery" },
  { to: "/favorites", icon: Heart, labelKey: "nav.favorites" },
];

const NAV_RAIL_WIDTH = 64;
const PANEL_CONFIGS = [
  { min: 180, default: 260, max: 400 },
  { min: 400, default: 600, max: null },
];

export default function AppLayout() {
  const location = useLocation();
  const navigate = useNavigate();
  const { theme, themeMeta, setThemeWithEvent } = useTheme();
  const { t } = useTranslation();
  const { widths, onHandleDown } = useResizable(PANEL_CONFIGS);
  const [activeProjectId, setActiveProjectId] = useState<string | null>(null);
  const [activeConversationId, setActiveConversationId] = useState<string | null>(null);
  const [conversationRefreshKey, setConversationRefreshKey] = useState(0);
  const [themePickerOpen, setThemePickerOpen] = useState(false);
  const themeButtonRef = useRef<HTMLButtonElement | null>(null);
  const themePanelRef = useRef<HTMLDivElement | null>(null);

  const isProjectListRoute = useMemo(
    () => location.pathname === "/projects",
    [location.pathname],
  );
  const isProjectDetailRoute = useMemo(
    () => /^\/projects\/[^/]+$/.test(location.pathname),
    [location.pathname],
  );
  const shouldHideSidebar = useMemo(
    () =>
      location.pathname === "/settings" ||
      location.pathname === "/extract" ||
      location.pathname === "/gallery" ||
      location.pathname === "/favorites",
    [location.pathname],
  );
  const routeProjectId = useMemo(() => {
    if (!location.pathname.startsWith("/projects/")) {
      return null;
    }
    const rest = location.pathname.slice("/projects/".length);
    const firstSegment = rest.split("/")[0];
    return decodeURIComponent(firstSegment) || null;
  }, [location.pathname]);

  const isProjectChatRoute = useMemo(
    () => /^\/projects\/[^/]+\/chat/.test(location.pathname),
    [location.pathname],
  );
  const pendingGenerateConversationId = useMemo(() => {
    const state = location.state as { activateConversationId?: string } | null;
    return typeof state?.activateConversationId === "string"
      ? state.activateConversationId
      : null;
  }, [location.state]);

  // Synchronously derived from route — prevents stale project ID during navigation
  // (e.g. navigating from a project page to /generate would briefly show project
  // conversations before the useEffect clears activeProjectId)
  const sidebarConversationProjectId = useMemo(() => {
    if (location.pathname === "/generate") return null;
    if (isProjectDetailRoute || isProjectChatRoute) return routeProjectId;
    return activeProjectId;
  }, [location.pathname, activeProjectId, isProjectDetailRoute, isProjectChatRoute, routeProjectId]);

  useEffect(() => {
    if (location.pathname === "/projects") {
      setActiveProjectId(null);
      setActiveConversationId(null);
      return;
    }

    // Project chat route — set project but let ProjectChatPage control the conversation
    if (isProjectChatRoute && routeProjectId) {
      setActiveProjectId(routeProjectId);
      return;
    }

    if (routeProjectId) {
      setActiveProjectId(routeProjectId);
      setActiveConversationId(null);
      return;
    }

    // /generate always uses the default project — no cross-contamination
    if (location.pathname === "/generate") {
      setActiveProjectId(null);
      setActiveConversationId(pendingGenerateConversationId);
      return;
    }

    setActiveProjectId(null);
  }, [location.pathname, routeProjectId, isProjectChatRoute, pendingGenerateConversationId]);

  useEffect(() => {
    if (!themePickerOpen) {
      return;
    }

    function handlePointerDown(event: MouseEvent) {
      const target = event.target as Node;

      if (
        themePanelRef.current?.contains(target) ||
        themeButtonRef.current?.contains(target)
      ) {
        return;
      }

      setThemePickerOpen(false);
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setThemePickerOpen(false);
      }
    }

    document.addEventListener("mousedown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [themePickerOpen]);

  const refreshConversations = useCallback(() => {
    setConversationRefreshKey((key) => key + 1);
  }, []);

  const selectConversation = useCallback(
    (id: string) => {
      setActiveConversationId(id);
      if (activeProjectId && activeProjectId !== "default") {
        navigate(`/projects/${activeProjectId}/chat/${id}`);
      } else {
        navigate("/generate");
      }
    },
    [navigate, activeProjectId],
  );

  const selectProject = useCallback(
    (id: string | null) => {
      setActiveProjectId(id);
      setActiveConversationId(null);
      navigate(id ? `/projects/${id}` : "/projects");
    },
    [navigate],
  );

  const selectCreatedProject = useCallback(
    (id: string) => {
      setActiveProjectId(id);
      setActiveConversationId(null);
      navigate(`/projects/${id}`);
    },
    [navigate],
  );

  const selectInitialConversation = useCallback((id: string) => {
    setActiveConversationId((current) => current ?? id);
  }, []);

  const createNewConversation = useCallback(() => {
    createConversation(undefined, activeProjectId)
      .then((conversation) => {
        setActiveProjectId(conversation.project_id);
        setActiveConversationId(conversation.id);
        refreshConversations();
        if (conversation.project_id && conversation.project_id !== "default") {
          navigate(`/projects/${conversation.project_id}/chat/${conversation.id}`);
        } else {
          navigate("/generate");
        }
      })
      .catch(() => {
        setActiveConversationId(null);
        if (activeProjectId && activeProjectId !== "default") {
          navigate(`/projects/${activeProjectId}/chat`);
        } else {
          navigate("/generate");
        }
      });
  }, [activeProjectId, navigate, refreshConversations]);

  return (
    <LayoutContext.Provider
      value={{
        activeProjectId,
        setActiveProjectId,
        activeConversationId,
        setActiveConversationId,
        refreshConversations,
      }}
    >
      <div className="relative flex h-screen overflow-hidden bg-background gradient-mesh">
        <aside
          className="flex shrink-0 flex-col items-center border-r border-border-subtle py-6"
          style={{ width: NAV_RAIL_WIDTH }}
        >
          <NavLink to="/generate" className="mb-10 group">
            <div className="relative h-9 w-9 overflow-hidden rounded-[10px] shadow-button transition-transform duration-200 group-hover:scale-105">
              <img src={appLogo} alt="Astro Studio" className="h-full w-full object-cover" />
            </div>
          </NavLink>

          <nav className="flex flex-1 flex-col items-center gap-3">
            {navItems.map(({ to, icon: Icon, labelKey }) => (
              <NavLink
                key={to}
                to={to}
                title={t(labelKey)}
                className={({ isActive }) =>
                  `relative flex h-10 w-10 items-center justify-center rounded-[10px] transition-all duration-200 ${
                    isActive
                      ? "text-primary bg-primary/6 shadow-card"
                      : "text-muted hover:text-foreground hover:bg-subtle"
                  }`
                }
              >
                {({ isActive }) => (
                  <>
                    <Icon size={20} strokeWidth={isActive ? 2 : 1.6} />
                    {isActive && (
                      <motion.div
                        layoutId="nav-indicator"
                        className="absolute -left-[8px] top-1/2 h-5 w-[3px] -translate-y-1/2 rounded-full gradient-primary"
                        transition={{ type: "spring", stiffness: 500, damping: 35 }}
                      />
                    )}
                  </>
                )}
              </NavLink>
            ))}
          </nav>

          <div className="mt-auto flex flex-col items-center gap-3">
            <button
              ref={themeButtonRef}
              type="button"
              title={t("theme.openPicker")}
              aria-label={t("theme.openPicker")}
              aria-haspopup="dialog"
              aria-expanded={themePickerOpen}
              onClick={() => setThemePickerOpen((open) => !open)}
              className={`flex h-10 w-10 items-center justify-center rounded-[10px] transition-colors ${
                themePickerOpen
                  ? "bg-primary/8 text-primary"
                  : "text-muted hover:text-foreground hover:bg-subtle"
              }`}
            >
              <motion.div
                animate={{ rotate: themePickerOpen ? 12 : 0, scale: themePickerOpen ? 1.04 : 1 }}
                transition={{ type: "spring", stiffness: 360, damping: 22 }}
              >
                <Palette size={18} strokeWidth={1.8} />
              </motion.div>
            </button>
            <NavLink
              to="/settings"
              title={t("nav.settings")}
              className={({ isActive }) =>
                `flex h-10 w-10 items-center justify-center rounded-[10px] transition-all duration-200 ${
                  isActive
                    ? "text-primary bg-primary/6 shadow-card"
                    : "text-muted hover:text-foreground hover:bg-subtle"
                }`
              }
            >
              <Settings size={20} strokeWidth={1.6} />
            </NavLink>
          </div>
        </aside>

        {!shouldHideSidebar && (
          <>
            <aside className="flex shrink-0 flex-col border-r border-border-subtle" style={{ width: widths[0] }}>
              {isProjectListRoute ? (
                <ProjectsSidebar
                  activeProjectId={activeProjectId}
                  onSelectProject={selectProject}
                  onProjectCreated={selectCreatedProject}
                />
              ) : (
                <ConversationList
                  activeProjectId={sidebarConversationProjectId}
                  activeConversationId={activeConversationId}
                  refreshKey={conversationRefreshKey}
                  onSelectProject={selectProject}
                  onProjectCreated={selectCreatedProject}
                  onSelectConversation={selectConversation}
                  onInitialConversation={selectInitialConversation}
                  onNewConversation={createNewConversation}
                />
              )}
            </aside>

            <ResizeHandle onMouseDown={onHandleDown(0)} />
          </>
        )}

        <main className="relative flex-1 overflow-hidden" style={{ minWidth: widths[1] }}>
          <AnimatePresence mode="wait">
            <motion.div
              key={location.pathname}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.15 }}
              className="h-full"
            >
              <Outlet />
            </motion.div>
          </AnimatePresence>
        </main>

        <AnimatePresence>
          {themePickerOpen && (
            <motion.div
              ref={themePanelRef}
              role="dialog"
              aria-modal="false"
              initial={{ opacity: 0, x: -8, y: 8 }}
              animate={{ opacity: 1, x: 0, y: 0 }}
              exit={{ opacity: 0, x: -8, y: 8 }}
              transition={{ duration: 0.16 }}
              className="absolute bottom-6 left-[78px] z-30 w-[360px] rounded-[18px] border border-border-subtle bg-surface/96 p-4 shadow-float backdrop-blur-xl"
            >
              <div className="mb-3 flex items-start justify-between gap-3">
                <div>
                  <h2 className="text-[14px] font-semibold text-foreground">
                    {t("theme.title")}
                  </h2>
                  <p className="mt-1 text-[11px] leading-relaxed text-muted/65">
                    {getThemeName(themeMeta, t)} · {t("settings.themeDesc")}
                  </p>
                </div>
              </div>
              <div className="max-h-[420px] overflow-y-auto pr-1">
                <ThemeCardPicker
                  selectedThemeId={theme}
                  activeLabel={t("theme.active")}
                  t={t}
                  compact
                  onSelect={(themeId, event) => {
                    setThemeWithEvent(themeId, event);
                    setThemePickerOpen(false);
                  }}
                />
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>
    </LayoutContext.Provider>
  );
}
