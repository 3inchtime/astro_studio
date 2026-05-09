import { useCallback, useEffect, useSyncExternalStore } from "react";
import type { MouseEvent as ReactMouseEvent } from "react";
import {
  DEFAULT_THEME_ID,
  THEME_CATALOG,
  getThemeCatalogEntry,
  resolveThemeId,
  type ThemeId,
} from "../lib/themes";

const STORAGE_KEY = "astro-theme";
const THEME_EVENT = "astro-theme-change";

function getStoredThemeId(): ThemeId {
  if (typeof window === "undefined") {
    return DEFAULT_THEME_ID;
  }

  return resolveThemeId(window.localStorage.getItem(STORAGE_KEY));
}

function getTransitionOrigin(
  event: ReactMouseEvent<HTMLElement> | null | undefined,
): { x: number; y: number } | null {
  if (!event) {
    return null;
  }

  return { x: event.clientX, y: event.clientY };
}

function applyThemeVariables(themeId: ThemeId) {
  if (typeof document === "undefined") {
    return;
  }

  const root = document.documentElement;
  const theme = getThemeCatalogEntry(themeId);

  root.dataset.theme = theme.id;
  root.classList.toggle("dark", theme.appearance === "dark");

  for (const [name, value] of Object.entries(theme.variables)) {
    root.style.setProperty(name, value);
  }
}

function notifyThemeListeners() {
  if (typeof window === "undefined") {
    return;
  }

  window.dispatchEvent(new CustomEvent(THEME_EVENT));
}

function persistTheme(themeId: ThemeId) {
  if (typeof window === "undefined") {
    return;
  }

  window.localStorage.setItem(STORAGE_KEY, themeId);
}

function applyThemeSelection(
  themeId: ThemeId,
  origin: { x: number; y: number } | null = null,
  emit = true,
) {
  if (typeof document === "undefined" || typeof window === "undefined") {
    return;
  }

  const root = document.documentElement;

  root.classList.add("theme-transition");

  const apply = () => {
    applyThemeVariables(themeId);
    persistTheme(themeId);
  };

  if (
    origin &&
    "startViewTransition" in document
  ) {
    const transitionDocument = document as Document & {
      startViewTransition: (
        callback: () => void,
      ) => { ready: Promise<void> };
    };

    const transition = transitionDocument.startViewTransition(() => {
      apply();
    });

    transition.ready.then(() => {
      const maxRadius = Math.hypot(
        Math.max(origin.x, window.innerWidth - origin.x),
        Math.max(origin.y, window.innerHeight - origin.y),
      );

      root.animate(
        {
          clipPath: [
            `circle(0px at ${origin.x}px ${origin.y}px)`,
            `circle(${maxRadius}px at ${origin.x}px ${origin.y}px)`,
          ],
        },
        {
          duration: 500,
          easing: "ease-out",
        },
      );
    }).catch(() => {});
  } else {
    apply();
  }

  window.setTimeout(() => {
    root.classList.remove("theme-transition");
  }, 500);

  if (emit) {
    notifyThemeListeners();
  }
}

function subscribe(listener: () => void) {
  if (typeof window === "undefined") {
    return () => {};
  }

  const handleThemeEvent = () => listener();
  const handleStorage = (event: StorageEvent) => {
    if (event.key === STORAGE_KEY) {
      listener();
    }
  };

  window.addEventListener(THEME_EVENT, handleThemeEvent);
  window.addEventListener("storage", handleStorage);

  return () => {
    window.removeEventListener(THEME_EVENT, handleThemeEvent);
    window.removeEventListener("storage", handleStorage);
  };
}

export function useTheme() {
  const theme = useSyncExternalStore(
    subscribe,
    getStoredThemeId,
    () => DEFAULT_THEME_ID,
  );

  useEffect(() => {
    applyThemeVariables(theme);
    persistTheme(theme);
  }, [theme]);

  const setTheme = useCallback((nextTheme: ThemeId) => {
    applyThemeSelection(nextTheme);
  }, []);

  const setThemeWithEvent = useCallback(
    (nextTheme: ThemeId, event?: ReactMouseEvent<HTMLElement>) => {
      applyThemeSelection(nextTheme, getTransitionOrigin(event));
    },
    [],
  );

  const cycleTheme = useCallback(() => {
    const currentIndex = THEME_CATALOG.findIndex((entry) => entry.id === theme);
    const nextIndex = currentIndex === -1
      ? 0
      : (currentIndex + 1) % THEME_CATALOG.length;

    applyThemeSelection(THEME_CATALOG[nextIndex].id);
  }, [theme]);

  return {
    theme,
    themeMeta: getThemeCatalogEntry(theme),
    themes: THEME_CATALOG,
    setTheme,
    setThemeWithEvent,
    cycleTheme,
  };
}
