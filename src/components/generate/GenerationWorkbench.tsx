import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  PanelLeftOpen,
  PanelRightClose,
  SlidersHorizontal,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { useDialogFocus } from "../../hooks/useDialogFocus";
import { useLayoutContext } from "../layout/AppLayout";

/** A stable portal host keeps parameter drafts mounted through responsive changes. */
export default function GenerationWorkbench({
  children,
}: {
  children: (
    parametersTarget: HTMLElement | null,
    drawerOpen: boolean,
  ) => ReactNode;
}) {
  const { t } = useTranslation();
  const {
    activeWorkspaceDrawer,
    setWorkspaceDrawer,
    toggleContextSidebar,
    contextSidebarCollapsed,
  } = useLayoutContext();
  const [localDrawer, setLocalDrawer] = useState(false);
  const [docked, setDocked] = useState(
    () => window.matchMedia("(min-width: 1200px)").matches,
  );
  const focusAfterDock = useRef(false);
  const [target, setTarget] = useState<HTMLDivElement | null>(null);
  const open =
    !docked &&
    (setWorkspaceDrawer ? activeWorkspaceDrawer === "parameters" : localDrawer);
  const close = () => {
    setLocalDrawer(false);
    setWorkspaceDrawer?.(null);
  };
  const toggle = () => {
    setLocalDrawer(!open);
    setWorkspaceDrawer?.(open ? null : "parameters");
  };
  const panelRef = useDialogFocus({ open, onClose: close });
  useEffect(() => {
    const media = window.matchMedia("(min-width: 1200px)");
    const update = () => {
      focusAfterDock.current =
        media.matches && !!panelRef.current?.contains(document.activeElement);
      setDocked(media.matches);
    };
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);
  useEffect(() => {
    if (docked) {
      setLocalDrawer(false);
      if (activeWorkspaceDrawer === "parameters") setWorkspaceDrawer?.(null);
    }
  }, [docked, activeWorkspaceDrawer, setWorkspaceDrawer]);
  useEffect(() => {
    if (docked && focusAfterDock.current) {
      focusAfterDock.current = false;
      (
        target?.querySelector<HTMLElement>(
          "select:not(:disabled), input:not(:disabled), summary, button:not(:disabled)",
        ) ?? panelRef.current
      )?.focus();
    }
  }, [docked, target, panelRef]);
  return (
    <div className="generation-workbench relative flex h-full min-h-0 min-w-0">
      <section className="flex min-h-0 min-w-0 flex-1 flex-col">
        <div className="flex h-12 shrink-0 items-center justify-between border-b border-border-subtle bg-surface px-5">
          <div className="flex min-w-0 items-center gap-2">
            {toggleContextSidebar && (
              <button
                type="button"
                aria-label={t("studio.openSidebar", {
                  defaultValue: "Open sidebar",
                })}
                aria-controls="workspace-sidebar"
                aria-expanded={activeWorkspaceDrawer === "context"}
                className={`studio-control-subtle focus-ring rounded-lg p-2 ${contextSidebarCollapsed ? "" : "min-[1440px]:hidden"}`}
                onClick={toggleContextSidebar}
              >
                <PanelLeftOpen size={16} />
              </button>
            )}
            <h1 className="text-base font-semibold tracking-tight">
              {t("nav.generate")}
            </h1>
          </div>
          <button
            type="button"
            className="studio-control-subtle focus-ring inline-flex min-h-9 items-center gap-2 rounded-lg px-2 text-xs min-[1200px]:hidden"
            aria-expanded={open}
            aria-controls="generation-parameters"
            onClick={toggle}
          >
            <SlidersHorizontal size={16} />
            {t("generate.parametersLabel")}
          </button>
        </div>
        {children(target, open || activeWorkspaceDrawer === "context")}
      </section>
      {open && (
        <div
          aria-hidden="true"
          className="absolute inset-0 z-30 bg-black/25"
          onClick={close}
        />
      )}
      <aside
        ref={panelRef}
        id="generation-parameters"
        role={open ? "dialog" : "complementary"}
        aria-modal={open || undefined}
        aria-label={t("generate.parametersLabel")}
        tabIndex={-1}
        className={`${docked || open ? "flex" : "hidden"} ${docked ? "relative" : "absolute inset-y-0 right-0 z-40 shadow-float"} w-[300px] max-w-full shrink-0 flex-col border-l border-border-subtle bg-surface`}
      >
        <div className="flex h-12 shrink-0 items-center justify-between border-b border-border-subtle px-4">
          <h2 className="text-sm font-medium">
            {t("generate.parametersLabel")}
          </h2>
          {open && (
            <button
              type="button"
              className="studio-control-subtle focus-ring rounded-lg p-2"
              onClick={close}
              aria-label={t("studio.closeParameters", {
                defaultValue: "Close parameters",
              })}
            >
              <PanelRightClose size={16} />
            </button>
          )}
        </div>
        <div ref={setTarget} className="min-h-0 flex-1 overflow-y-auto" />
      </aside>
    </div>
  );
}
