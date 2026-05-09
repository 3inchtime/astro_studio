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
  t: (key: string) => string;
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
    <div className={cn("grid gap-3", gridClassName)}>
      {THEME_CATALOG.map((theme) => {
        const active = theme.id === selectedThemeId;
        const name = getThemeName(theme, t);
        const description = getThemeDescription(theme, t);

        return (
          <button
            key={theme.id}
            type="button"
            aria-label={`Select ${name} theme`}
            aria-pressed={active}
            onClick={(event) => onSelect(theme.id, event)}
            className={cn(
              "rounded-[12px] border p-3 text-left transition-all",
              active
                ? "border-primary/35 bg-primary/8 shadow-card"
                : "border-border-subtle bg-subtle/20 hover:border-border hover:bg-subtle/45",
            )}
          >
            <div className="flex items-start justify-between gap-3">
              <div className="flex items-center gap-1.5">
                {theme.swatches.map((swatch) => (
                  <span
                    key={`${theme.id}-${swatch}`}
                    className="h-3.5 w-3.5 rounded-full border border-black/5"
                    style={{ backgroundColor: swatch }}
                  />
                ))}
              </div>
              {active && (
                <span className="inline-flex items-center gap-1 rounded-full border border-primary/15 bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary">
                  <Check size={11} strokeWidth={2.5} />
                  {activeLabel}
                </span>
              )}
            </div>
            <div className={compact ? "mt-2.5" : "mt-3"}>
              <div className="text-[12px] font-semibold text-foreground">
                {name}
              </div>
              <p className={cn(
                "mt-1 leading-relaxed text-muted/70",
                compact ? "text-[10.5px]" : "text-[11px]",
              )}
              >
                {description}
              </p>
            </div>
          </button>
        );
      })}
    </div>
  );
}
