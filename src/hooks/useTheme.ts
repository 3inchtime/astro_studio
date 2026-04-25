import { useState, useEffect, useCallback, useRef } from "react";

export type Theme = "light" | "dark";

const STORAGE_KEY = "astro-theme";

function getInitialTheme(): Theme {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === "dark" || stored === "light") return stored;
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

export function useTheme() {
  const [theme, setThemeState] = useState<Theme>(getInitialTheme);
  const pendingTransition = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    // Apply pending view transition (deferred from toggleTheme)
    if (pendingTransition.current) {
      const { x, y } = pendingTransition.current;
      pendingTransition.current = null;
      applyWithTransition(x, y);
      return;
    }

    document.documentElement.classList.toggle("dark", theme === "dark");
    localStorage.setItem(STORAGE_KEY, theme);
  }, [theme]);

  function applyWithTransition(x: number, y: number) {
    const root = document.documentElement;
    const isDark = theme === "dark";

    // Enable CSS transition class
    root.classList.add("theme-transition");

    // Use View Transition API if available (Chrome/Edge 111+)
    if ("startViewTransition" in document) {
      const vt = (document as unknown as { startViewTransition: (cb: () => void) => { ready: Promise<void> } }).startViewTransition(() => {
        root.classList.toggle("dark", isDark);
        localStorage.setItem(STORAGE_KEY, theme);
      });

      vt.ready.then(() => {
        const maxRadius = Math.hypot(
          Math.max(x, window.innerWidth - x),
          Math.max(y, window.innerHeight - y),
        );
        root.animate(
          {
            clipPath: [
              `circle(0px at ${x}px ${y}px)`,
              `circle(${maxRadius}px at ${x}px ${y}px)`,
            ],
          },
          {
            duration: 500,
            easing: "ease-out",
          },
        );
      }).catch(() => {});
    } else {
      // Fallback: just toggle with CSS transition
      root.classList.toggle("dark", isDark);
      localStorage.setItem(STORAGE_KEY, theme);
    }

    // Remove transition class after animation
    setTimeout(() => root.classList.remove("theme-transition"), 500);
  }

  const toggleTheme = useCallback(() => {
    // Set state first, then trigger transition in useEffect
    setThemeState((t) => (t === "dark" ? "light" : "dark"));
  }, []);

  // Call this from an onClick handler that has access to MouseEvent
  const toggleThemeWithEvent = useCallback((e: React.MouseEvent) => {
    pendingTransition.current = { x: e.clientX, y: e.clientY };
    setThemeState((t) => (t === "dark" ? "light" : "dark"));
  }, []);

  return { theme, toggleTheme, toggleThemeWithEvent };
}
