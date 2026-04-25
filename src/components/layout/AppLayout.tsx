import { createContext, useContext, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import { Image, Settings, Sparkles, Sun, Moon } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useTheme } from "../../hooks/useTheme";
import { useResizable } from "../../hooks/useResizable";
import { ResizeHandle } from "./ResizeHandle";
import ConversationList from "../sidebar/ConversationList";

interface LayoutContextType {
  activeConversationId: string | null;
  setActiveConversationId: (id: string | null) => void;
}

export const LayoutContext = createContext<LayoutContextType>({
  activeConversationId: null,
  setActiveConversationId: () => {},
});

export function useLayoutContext() {
  return useContext(LayoutContext);
}

const navItems = [
  { to: "/generate", icon: Sparkles, labelKey: "nav.generate" },
  { to: "/gallery", icon: Image, labelKey: "nav.gallery" },
  { to: "/settings", icon: Settings, labelKey: "nav.settings" },
];

const PANEL_CONFIGS = [
  { min: 48, default: 64, max: 80 },
  { min: 180, default: 260, max: 400 },
  { min: 400, default: 600, max: null },
];

export default function AppLayout() {
  const location = useLocation();
  const navigate = useNavigate();
  const { theme, toggleTheme } = useTheme();
  const { t } = useTranslation();
  const { widths, onHandleDown } = useResizable(PANEL_CONFIGS);
  const [activeConversationId, setActiveConversationId] = useState<string | null>(null);

  return (
    <LayoutContext.Provider value={{ activeConversationId, setActiveConversationId }}>
      <div className="flex h-screen overflow-hidden bg-background gradient-mesh">
        {/* Nav Rail */}
        <aside
          className="flex shrink-0 flex-col items-center border-r border-border-subtle py-6"
          style={{ width: widths[0] }}
        >
          <NavLink to="/generate" className="mb-8 group">
            <div className="relative flex h-9 w-9 items-center justify-center rounded-[10px] gradient-primary shadow-button transition-transform duration-200 group-hover:scale-105">
              <Sparkles size={15} className="text-white" strokeWidth={2.5} />
            </div>
          </NavLink>

          <nav className="flex flex-1 flex-col items-center gap-1">
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

          <div className="mt-auto">
            <button
              onClick={toggleTheme}
              className="flex h-10 w-10 items-center justify-center rounded-[10px] text-muted transition-colors hover:text-foreground hover:bg-subtle"
            >
              <AnimatePresence mode="wait" initial={false}>
                {theme === "dark" ? (
                  <motion.div
                    key="moon"
                    initial={{ rotate: -90, scale: 0, opacity: 0 }}
                    animate={{ rotate: 0, scale: 1, opacity: 1 }}
                    exit={{ rotate: 90, scale: 0, opacity: 0 }}
                    transition={{ type: "spring", stiffness: 400, damping: 20 }}
                  >
                    <Moon size={18} strokeWidth={1.8} />
                  </motion.div>
                ) : (
                  <motion.div
                    key="sun"
                    initial={{ rotate: 90, scale: 0, opacity: 0 }}
                    animate={{ rotate: 0, scale: 1, opacity: 1 }}
                    exit={{ rotate: -90, scale: 0, opacity: 0 }}
                    transition={{ type: "spring", stiffness: 400, damping: 20 }}
                  >
                    <Sun size={18} strokeWidth={1.8} />
                  </motion.div>
                )}
              </AnimatePresence>
            </button>
          </div>
        </aside>

        <ResizeHandle onMouseDown={onHandleDown(0)} />

        {/* Conversation Sidebar */}
        <aside
          className="flex shrink-0 flex-col border-r border-border-subtle"
          style={{ width: widths[1] }}
        >
          <ConversationList
            activeConversationId={activeConversationId}
            onSelectConversation={(id) => {
              setActiveConversationId(id);
              navigate("/generate");
            }}
            onNewConversation={() => {
              setActiveConversationId(null);
              navigate("/generate");
            }}
          />
        </aside>

        <ResizeHandle onMouseDown={onHandleDown(1)} />

        {/* Main Content */}
        <main className="relative flex-1 overflow-hidden" style={{ minWidth: widths[2] }}>
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
      </div>
    </LayoutContext.Provider>
  );
}
