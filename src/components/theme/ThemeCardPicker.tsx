import type { MouseEvent as ReactMouseEvent } from "react";
import { Check } from "lucide-react";
import { cn } from "../../lib/utils";
import {
  THEME_CATALOG,
  getThemeDescription,
  getThemeName,
  type ThemeId,
} from "../../lib/themes";

interface ThemeCardPickerProps {
  selectedThemeId: ThemeId;
  activeLabel: string;
  t: (key: string, options?: { name: string }) => string;
  gridClassName?: string;
  compact?: boolean;
  onSelect: (themeId: ThemeId, event: ReactMouseEvent<HTMLButtonElement>) => void;
}

export function ThemeCardPicker({
  selectedThemeId,
  activeLabel,
  t,
  gridClassName,
  compact = false,
  onSelect,
}: ThemeCardPickerProps) {
  return (
    <div className={cn("grid gap-2", gridClassName)}>
      {THEME_CATALOG.map((theme) => {
        const active = theme.id === selectedThemeId;
        const name = getThemeName(theme, t);
        const description = getThemeDescription(theme, t);
        return (
          <button
            key={theme.id}
            type="button"
            aria-label={t("theme.select", { name })}
            aria-description={description}
            aria-pressed={active}
            title={`${name}\n${description}`}
            onClick={(event) => onSelect(theme.id, event)}
            className={cn(
              "focus-ring flex min-w-0 items-center gap-2.5 rounded-lg border px-3 text-left transition-colors duration-[140ms]",
              compact ? "min-h-10 py-2" : "min-h-14 py-3",
              active
                ? "border-primary bg-surface"
                : "border-border-subtle bg-background hover:border-border hover:bg-surface",
            )}
          >
            <span aria-hidden="true" className="grid shrink-0 grid-cols-2 gap-0.5">
              {theme.swatches.map((swatch, index) => (
                <span key={index} className="h-1.5 w-1.5 rounded-[2px]" style={{ backgroundColor: swatch }} />
              ))}
            </span>
            <span className="min-w-0 flex-1">
              <span className="block truncate text-xs font-medium text-foreground">{name}</span>
              {!compact && <span className="mt-1 block text-xs leading-relaxed text-muted">{description}</span>}
            </span>
            {active && <><Check size={14} aria-hidden="true" className="shrink-0 text-primary" /><span className="sr-only">{activeLabel}</span></>}
          </button>
        );
      })}
    </div>
  );
}
