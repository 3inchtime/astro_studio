import { NavLink, Outlet, useLocation } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import { useEffect, useState, useCallback } from "react";
import { Image, Settings, Sparkles, Clock, Search, Sun, Moon } from "lucide-react";
import { searchGenerations, toAssetUrl } from "../../lib/api";
import { formatTimeAgo } from "../../lib/utils";
import { useTheme } from "../../hooks/useTheme";
import { useResizable } from "../../hooks/useResizable";
import { ResizeHandle } from "./ResizeHandle";
import type { GenerationResult } from "../../types";

const navItems = [
  { to: "/generate", icon: Sparkles, label: "Generate" },
  { to: "/gallery", icon: Image, label: "Gallery" },
  { to: "/settings", icon: Settings, label: "Settings" },
];

const PANEL_CONFIGS = [
  { min: 48, default: 64, max: 80 },
  { min: 180, default: 260, max: 400 },
  { min: 400, default: 600, max: null },
];

export default function AppLayout() {
  const location = useLocation();
  const [history, setHistory] = useState<GenerationResult[]>([]);
  const [historyQuery, setHistoryQuery] = useState("");
  const { theme, toggleTheme } = useTheme();
  const { widths, onHandleDown } = useResizable(PANEL_CONFIGS);

  const loadHistory = useCallback((q?: string) => {
    searchGenerations(q || undefined, 1).then((res) => {
      setHistory(res.generations.slice(0, 20));
    }).catch(() => {});
  }, []);

  useEffect(() => { loadHistory(); }, [loadHistory]);

  useEffect(() => {
    if (historyQuery) {
      const timer = setTimeout(() => loadHistory(historyQuery), 300);
      return () => clearTimeout(timer);
    } else {
      loadHistory();
    }
  }, [historyQuery, loadHistory]);

  return (
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
          {navItems.map(({ to, icon: Icon, label }) => (
            <NavLink
              key={to}
              to={to}
              title={label}
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

      {/* History Sidebar */}
      <aside
        className="flex shrink-0 flex-col border-r border-border-subtle"
        style={{ width: widths[1] }}
      >
        <div className="px-4 pt-5 pb-3">
          <div className="flex items-center gap-2 mb-3">
            <Clock size={13} className="text-muted" strokeWidth={1.8} />
            <span className="text-[13px] font-semibold text-foreground tracking-tight">History</span>
          </div>
          <div className="relative">
            <Search size={12} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-muted" strokeWidth={2} />
            <input
              value={historyQuery}
              onChange={(e) => setHistoryQuery(e.target.value)}
              placeholder="Search..."
              className="h-[28px] w-full rounded-[8px] border border-border-subtle bg-subtle/50 pl-7 pr-2 text-[12px] text-foreground placeholder:text-muted/60 focus:outline-none focus:border-border focus:bg-surface transition-colors"
            />
          </div>
        </div>
        <div className="flex-1 overflow-y-auto px-2.5 pb-4">
          {history.length === 0 ? (
            <div className="px-2 pt-6 text-center">
              <p className="text-[12px] text-muted/50">{historyQuery ? "No results" : "No history yet"}</p>
            </div>
          ) : (
            <div className="flex flex-col gap-0.5">
              {history.map((item, i) => {
                const img = item.images[0];
                return (
                  <motion.button
                    key={item.generation.id}
                    initial={{ opacity: 0, x: -6 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ delay: i * 0.03, duration: 0.25 }}
                    className="group flex items-center gap-2.5 rounded-[10px] px-2 py-2 text-left transition-colors hover:bg-subtle"
                  >
                    <div className="h-9 w-9 shrink-0 overflow-hidden rounded-[8px] bg-subtle border border-border-subtle">
                      {img ? (
                        <img src={toAssetUrl(img.thumbnail_path)} alt="" className="h-full w-full object-cover" loading="lazy" />
                      ) : (
                        <div className="flex h-full w-full items-center justify-center">
                          <Image size={14} className="text-muted/30" />
                        </div>
                      )}
                    </div>
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-[12px] leading-snug text-foreground/80 group-hover:text-foreground transition-colors">
                        {item.generation.prompt}
                      </p>
                      <p className="mt-0.5 text-[10px] text-muted/60">{formatTimeAgo(item.generation.created_at)}</p>
                    </div>
                  </motion.button>
                );
              })}
            </div>
          )}
        </div>
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
  );
}
