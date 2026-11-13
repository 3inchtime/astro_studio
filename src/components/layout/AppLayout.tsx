import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import {
  FolderKanban,
  Heart,
  Image,
  LayoutTemplate,
  MessageSquareText,
  Palette,
  Settings,
  Sparkles,
  Trash2,
  PanelLeftClose,
  X,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { useTheme } from "../../hooks/useTheme";
import { useResizable } from "../../hooks/useResizable";
import appLogo from "../../assets/logo.png";
import { useDialogFocus } from "../../hooks/useDialogFocus";
import ConversationList from "../sidebar/ConversationList";
import ProjectsSidebar from "../projects/ProjectsSidebar";
import {
  createConversation,
  checkForUpdate,
  isUpdateSupported,
} from "../../lib/api";
import { ThemeCardPicker } from "../theme/ThemeCardPicker";
import { getThemeName } from "../../lib/themes";
import UpdateDialog from "../common/UpdateDialog";
import type { UpdateMetadata } from "../../lib/api";
import {
  ROUTES,
  MAIN_NAV_ROUTES,
  getRouteContext,
  getRouteLayout,
  shouldHideSidebarForContext,
} from "../../app/routes/contracts";

interface LayoutContextType {
  toggleContextSidebar?: () => void;
  contextSidebarCollapsed?: boolean;
  activeWorkspaceDrawer?: "context" | "parameters" | null;
  setWorkspaceDrawer?: (drawer: "context" | "parameters" | null) => void;
  activeProjectId: string | null;
  setActiveProjectId: (id: string | null) => void;
  activeConversationId: string | null;
  setActiveConversationId: (id: string | null) => void;
  refreshConversations: () => void;
  updateSupported: boolean | null;
  checkForUpdates: (options?: {
    silent?: boolean;
  }) => Promise<UpdateMetadata | null>;
}

export const LayoutContext = createContext<LayoutContextType>({
  activeProjectId: null,
  setActiveProjectId: () => {},
  activeConversationId: null,
  setActiveConversationId: () => {},
  refreshConversations: () => {},
  updateSupported: true,
  checkForUpdates: async () => null,
});

export function useLayoutContext() {
  return useContext(LayoutContext);
}

type MainNavKey = (typeof MAIN_NAV_ROUTES)[number]["key"];
const iconByKey: Record<MainNavKey, typeof Sparkles> = {
  generate: Sparkles,
  extract: MessageSquareText,
  canvas: LayoutTemplate,
  projects: FolderKanban,
  gallery: Image,
  favorites: Heart,
};

const navItems = MAIN_NAV_ROUTES.map(({ key, to, labelKey }) => {
  return { key, to, labelKey, icon: iconByKey[key] };
});

const NAV_RAIL_WIDTH = 80;
const PANEL_CONFIGS = [
  { min: 180, default: 260, max: 400 },
  { min: 400, default: 600, max: null },
];

export default function AppLayout() {
  const location = useLocation();
  const navigate = useNavigate();
  const { theme, themeMeta, setThemeWithEvent } = useTheme();
  const { t } = useTranslation();
  const { widths, onHandleDown, setPanelWidth } = useResizable(PANEL_CONFIGS);
  const [compact, setCompact] = useState(() => window.matchMedia("(max-width: 1439px)").matches);
  const [activeWorkspaceDrawer, setWorkspaceDrawer] = useState<"context" | "parameters" | null>(null);
  const sidebarOpen = activeWorkspaceDrawer === "context";
  const setSidebarOpen = (value: boolean | ((open: boolean) => boolean)) => {
    setWorkspaceDrawer(current => (typeof value === "function" ? value(current === "context") : value) ? "context" : null);
  };
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() => {
    try { return localStorage.getItem("astro-sidebar-collapsed-v1") === "true"; } catch { return false; }
  });
  useEffect(() => {
    const media = window.matchMedia("(max-width: 1439px)");
    const update = () => { setCompact(media.matches); setSidebarOpen(false); };
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);
  useEffect(() => { setSidebarOpen(false); }, [location.pathname]);
  const toggleSidebar = () => {
    if (compact) setSidebarOpen((open) => !open);
    else setSidebarCollapsed((collapsed) => {
      try { localStorage.setItem("astro-sidebar-collapsed-v1", String(!collapsed)); } catch { /* Session-only preference. */ }
      return !collapsed;
    });
  };
  const [activeProjectId, setActiveProjectId] = useState<string | null>(null);
  const [activeConversationId, setActiveConversationId] = useState<
    string | null
  >(null);
  const [conversationRefreshKey, setConversationRefreshKey] = useState(0);
  const [themePickerOpen, setThemePickerOpen] = useState(false);
  const themeButtonRef = useRef<HTMLButtonElement | null>(null);
  const themePanelRef = useRef<HTMLDivElement | null>(null);
  const [pendingUpdate, setPendingUpdate] = useState<UpdateMetadata | null>(
    null,
  );
  const [updateDialogOpen, setUpdateDialogOpen] = useState(false);
  const [updateSupported, setUpdateSupported] = useState<boolean | null>(null);

  const routeContext = useMemo(
    () => getRouteContext(location.pathname),
    [location.pathname],
  );
  const layout = getRouteLayout(routeContext.kind);
  const isProjectListRoute = layout.sidebar.type === "projects";
  const isProjectHomeRoute = routeContext.kind === "projectHome";
  const shouldHideSidebar = useMemo(
    () => shouldHideSidebarForContext(routeContext),
    [routeContext],
  );
  const routeProjectId = routeContext.projectId;
  const contextPanelRef = useDialogFocus({ open: compact && sidebarOpen && !shouldHideSidebar, onClose: () => setSidebarOpen(false) });
  const themeFocusRef = useDialogFocus({ open: themePickerOpen, onClose: () => setThemePickerOpen(false) });

  const isProjectChatRoute = routeContext.kind === "chat";
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
    if (layout.sidebar.type === "projects") return null;
    if (layout.sidebar.useProjectFromPath) return routeProjectId;
    if (routeContext.kind === "generate") return null;
    return activeProjectId;
  }, [
    routeContext.kind,
    activeProjectId,
    layout.sidebar.type,
    layout.sidebar.useProjectFromPath,
    routeProjectId,
  ]);

  useEffect(() => {
    // /generate always uses the default project — keep pending launch context.
    if (routeContext.kind === "generate") {
      setActiveProjectId(null);
      setActiveConversationId(pendingGenerateConversationId);
      return;
    }

    if (layout.clearProjectOnEnter) {
      setActiveProjectId(null);
      setActiveConversationId(null);
      return;
    }

    if (isProjectChatRoute && routeProjectId) {
      setActiveProjectId(routeProjectId);
      setActiveConversationId(routeContext.conversationId ?? null);
      return;
    }

    if (layout.sidebar.useProjectFromPath && routeProjectId) {
      setActiveProjectId(routeProjectId);
      if (isProjectHomeRoute) {
        setActiveConversationId(null);
      }
      return;
    }

    setActiveProjectId(null);
  }, [
    routeContext.kind,
    layout.clearProjectOnEnter,
    layout.sidebar.useProjectFromPath,
    routeProjectId,
    isProjectChatRoute,
    isProjectHomeRoute,
    pendingGenerateConversationId,
  ]);

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

  useEffect(() => {
    let cancelled = false;

    isUpdateSupported()
      .then((supported) => {
        if (cancelled) {
          return;
        }
        setUpdateSupported(supported);
        if (!supported) {
          setPendingUpdate(null);
          setUpdateDialogOpen(false);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setUpdateSupported(false);
          setPendingUpdate(null);
          setUpdateDialogOpen(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const checkForUpdates = useCallback(
    async (options: { silent?: boolean } = {}) => {
      if (updateSupported !== true) {
        return null;
      }

      try {
        const update = await checkForUpdate();
        setPendingUpdate(update);
        if (update) {
          setUpdateDialogOpen(true);
        }
        return update;
      } catch (error) {
        if (options.silent) {
          return null;
        }
        throw error;
      }
    },
    [updateSupported],
  );

  // Check for updates on app start (silently)
  useEffect(() => {
    if (updateSupported !== true) {
      return;
    }

    // Delay update check by 5 seconds to avoid impacting startup performance
    const timer = setTimeout(() => {
      void checkForUpdates({ silent: true });
    }, 5000);

    return () => clearTimeout(timer);
  }, [checkForUpdates, updateSupported]);

  const refreshConversations = useCallback(() => {
    setConversationRefreshKey((key) => key + 1);
  }, []);

  const selectConversation = useCallback(
    (id: string) => {
      setActiveConversationId(id);
      if (activeProjectId && activeProjectId !== "default") {
        navigate(ROUTES.projectChat(activeProjectId, id));
      } else {
        navigate(ROUTES.generate);
      }
    },
    [navigate, activeProjectId],
  );

  const selectProject = useCallback(
    (id: string | null) => {
      setActiveProjectId(id);
      setActiveConversationId(null);
      navigate(id ? ROUTES.projectHome(id) : ROUTES.projects);
    },
    [navigate],
  );

  const selectCreatedProject = useCallback(
    (id: string) => {
      setActiveProjectId(id);
      setActiveConversationId(null);
      navigate(ROUTES.projectHome(id));
    },
    [navigate],
  );

  const selectInitialConversation = useCallback((id: string) => {
    setActiveConversationId((current) => current ?? id);
  }, []);

  const clearActiveConversation = useCallback(() => {
    setActiveConversationId(null);
    if (activeProjectId && activeProjectId !== "default") {
      navigate(ROUTES.projectChat(activeProjectId));
    } else {
      navigate(ROUTES.generate);
    }
  }, [activeProjectId, navigate]);

  const createNewConversation = useCallback(() => {
    createConversation(undefined, activeProjectId)
      .then((conversation) => {
        setActiveProjectId(conversation.project_id);
        setActiveConversationId(conversation.id);
        refreshConversations();
        if (conversation.project_id && conversation.project_id !== "default") {
          navigate(
            ROUTES.projectChat(conversation.project_id, conversation.id),
          );
        } else {
          navigate(ROUTES.generate);
        }
      })
      .catch(() => {
        setActiveConversationId(null);
        if (activeProjectId && activeProjectId !== "default") {
          navigate(
            activeProjectId && activeProjectId !== "default"
              ? ROUTES.projectChat(activeProjectId)
              : ROUTES.generate,
          );
        }
      });
  }, [activeProjectId, navigate, refreshConversations]);

  return (
    <LayoutContext.Provider
      value={{
        toggleContextSidebar: toggleSidebar,
        contextSidebarCollapsed: sidebarCollapsed,
        activeWorkspaceDrawer,
        setWorkspaceDrawer,
        activeProjectId,
        setActiveProjectId,
        activeConversationId,
        setActiveConversationId,
        refreshConversations,
        updateSupported,
        checkForUpdates,
      }}
    >
      <div className="studio-app-shell relative flex h-screen overflow-hidden bg-background">
        <a href="#main-content" className="sr-only focus:not-sr-only focus:absolute focus:left-24 focus:top-2 focus:z-[100] focus:rounded-lg focus:bg-surface focus:p-3">{t("studio.skipContent", { defaultValue: "Skip to content" })}</a>
        <aside
          className="studio-nav-rail flex shrink-0 flex-col items-center border-r py-3"
          style={{ width: NAV_RAIL_WIDTH }}
        >
          <NavLink
            to={ROUTES.generate}
            className="focus-ring mb-4 rounded-[12px] border border-transparent p-1 transition-colors hover:border-border-subtle hover:bg-surface/70"
          >
            <div className="relative h-9 w-9 overflow-hidden rounded-[10px] shadow-button transition-shadow duration-200">
              <img
                src={appLogo}
                alt="Astro Studio"
                className="h-full w-full object-cover"
              />
            </div>
          </NavLink>

          <nav aria-label={t("studio.navigation", { defaultValue: "Main navigation" })} className="flex min-h-0 flex-1 flex-col items-center gap-1 overflow-y-auto px-1">
            {navItems.map(({ key, to, icon: Icon, labelKey }, index) => (
              <div key={to} className={index === 3 || index === 4 ? "mt-3 border-t border-border pt-3" : ""}>
                <NavLink to={to} title={t(labelKey)} aria-label={t(labelKey)} className="studio-nav-item focus-ring">
                  <Icon size={20} strokeWidth={1.8} aria-hidden="true" />
                  <span>{t(key === "extract" ? "studio.nav.extractShort" : labelKey)}</span>
                </NavLink>
              </div>
            ))}
            <NavLink to={ROUTES.trash} title={t("trash.title")} aria-label={t("trash.title")} className="studio-nav-item focus-ring">
              <Trash2 size={20} strokeWidth={1.8} aria-hidden="true" /><span>{t("trash.title")}</span>
            </NavLink>
          </nav>

          <div className="mt-3 flex shrink-0 flex-col items-center gap-2">
            <button
              ref={themeButtonRef}
              type="button"
              title={t("theme.openPicker")}
              aria-label={t("theme.openPicker")}
              aria-haspopup="dialog"
              aria-expanded={themePickerOpen}
              onClick={() => setThemePickerOpen((open) => !open)}
              className={`focus-ring flex h-10 w-10 cursor-pointer items-center justify-center rounded-[10px] border transition-colors ${
                themePickerOpen
                  ? "border-white/70 dark:border-border bg-surface text-primary dark:text-foreground shadow-float ring-1 ring-primary/12"
                  : "border-transparent text-muted dark:text-foreground hover:border-border-subtle hover:bg-surface/72 hover:text-foreground dark:hover:text-foreground hover:shadow-card"
              }`}
            >
              <motion.div
                animate={{
                  rotate: themePickerOpen ? 12 : 0,
                  scale: themePickerOpen ? 1.04 : 1,
                }}
                transition={{ type: "spring", stiffness: 360, damping: 22 }}
              >
                <Palette size={18} strokeWidth={1.8} />
              </motion.div>
            </button>
            <NavLink
              to={ROUTES.settings}
              title={t("nav.settings")}
              className={({ isActive }) =>
                `focus-ring flex h-10 w-10 cursor-pointer items-center justify-center rounded-[10px] border transition-all duration-200 ${
                  isActive
                    ? "border-white/70 dark:border-border bg-surface text-primary dark:text-foreground shadow-float ring-1 ring-primary/12"
                    : "border-transparent text-muted dark:text-foreground hover:border-border-subtle hover:bg-surface/72 hover:text-foreground dark:hover:text-foreground hover:shadow-card"
                }`
              }
            >
              <Settings size={20} strokeWidth={1.6} />
            </NavLink>
          </div>
        </aside>

        {!shouldHideSidebar && (
          <>
            {compact && sidebarOpen && <button type="button" aria-label={t("studio.closeSidebar", { defaultValue: "Close sidebar" })} className="absolute inset-0 left-20 z-30 bg-black/20" onClick={() => setSidebarOpen(false)} />}
            <aside ref={contextPanelRef} id="workspace-sidebar" role={compact && sidebarOpen ? "dialog" : undefined} aria-modal={compact && sidebarOpen || undefined}
              aria-label={t("studio.sidebar", { defaultValue: "Workspace sidebar" })} tabIndex={-1}
              data-open={sidebarOpen} data-collapsed={!compact && sidebarCollapsed}
              className="studio-context-panel flex shrink-0 flex-col border-r border-border-subtle"
              style={{ "--sidebar-width": `${widths[0]}px` } as React.CSSProperties}>
              <div className="flex h-12 shrink-0 items-center justify-between border-b border-border-subtle px-4">
                <span className="text-xs font-semibold text-muted">{t(isProjectListRoute ? "nav.projects" : "studio.conversations", { defaultValue: "Conversations" })}</span>
                <button type="button" onClick={compact ? () => setSidebarOpen(false) : toggleSidebar} aria-label={t("studio.closeSidebar", { defaultValue: "Close sidebar" })} className="studio-control-subtle focus-ring rounded-lg p-2"><PanelLeftClose size={16} /></button>
              </div>
              {isProjectListRoute ? <ProjectsSidebar activeProjectId={activeProjectId} onSelectProject={selectProject} onProjectCreated={selectCreatedProject} /> :
                <ConversationList activeProjectId={sidebarConversationProjectId} activeConversationId={activeConversationId} refreshKey={conversationRefreshKey}
                  onSelectProject={selectProject} onProjectCreated={selectCreatedProject}
                  onSelectConversation={(id) => { selectConversation(id); setSidebarOpen(false); }}
                  onInitialConversation={selectInitialConversation} onClearActiveConversation={clearActiveConversation} onNewConversation={createNewConversation} />}
            </aside>
            {!sidebarCollapsed && <div className="studio-context-resizer z-10 w-1 shrink-0 cursor-col-resize hover:bg-primary/20 focus-visible:bg-primary/20" role="separator" aria-orientation="vertical"
              aria-label={t("studio.resizeSidebar", { defaultValue: "Resize sidebar" })} tabIndex={0} aria-valuemin={180} aria-valuemax={400} aria-valuenow={widths[0]}
              onMouseDown={onHandleDown(0)} onKeyDown={(event) => {
                if (event.key === "ArrowLeft" || event.key === "ArrowRight") { event.preventDefault(); setPanelWidth(0, widths[0] + (event.key === "ArrowRight" ? 20 : -20)); }
                if (event.key === "Home" || event.key === "End") { event.preventDefault(); setPanelWidth(0, event.key === "Home" ? 180 : 400); }
              }} />}
          </>
        )}

        <main
          id="main-content" tabIndex={-1}
          className="studio-main-canvas relative flex min-w-0 flex-1 flex-col overflow-hidden"
        >

          <motion.div
              key={routeContext.pathname}
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ duration: 0.18 }}
              className="min-h-0 flex-1"
            >
              <Outlet />
          </motion.div>
        </main>

        <AnimatePresence>
          {themePickerOpen && (
            <motion.div
              ref={(node) => { themePanelRef.current = node; themeFocusRef.current = node; }}
              tabIndex={-1}
              role="dialog"
              aria-modal="true"
              aria-label={t("theme.title")}
              initial={{ opacity: 0, x: -8, y: 8 }}
              animate={{ opacity: 1, x: 0, y: 0 }}
              exit={{ opacity: 0, x: -8, y: 8 }}
              transition={{ duration: 0.16 }}
              className="studio-floating-panel absolute bottom-6 left-[88px] z-50 w-[360px] rounded-[16px] p-4"
            >
              <div className="mb-3 flex items-start justify-between gap-3">
                <div>
                  <h2 className="text-sm font-semibold text-foreground">
                    {t("theme.title")}
                  </h2>
                  <p className="mt-1 text-xs leading-relaxed text-muted">
                    {getThemeName(themeMeta, t)} · {t("settings.themeDesc")}
                  </p>
                </div>
              </div>
              <button type="button" onClick={() => setThemePickerOpen(false)} aria-label={t("update.close")} className="absolute right-3 top-3 rounded-lg p-2 hover:bg-subtle"><X size={16} /></button>
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

        <UpdateDialog
          open={updateDialogOpen}
          update={pendingUpdate}
          onClose={() => setUpdateDialogOpen(false)}
        />
      </div>
    </LayoutContext.Provider>
  );
}
