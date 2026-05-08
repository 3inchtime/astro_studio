import { useRef } from "react";
import { CalendarDays, X } from "lucide-react";
import { cn } from "../../lib/utils";

interface DateFilterFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
}

function formatDate(dateStr: string): string {
  const date = new Date(dateStr + "T00:00:00");
  return date.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export default function DateFilterField({
  label,
  value,
  onChange,
}: DateFilterFieldProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const inputId = `date-filter-${label.toLowerCase().replace(/\s+/g, "-")}`;

  return (
    <div className="flex min-w-[150px] flex-[1_1_160px] flex-col gap-1.5 xl:max-w-[185px]">
      <label
        htmlFor={inputId}
        className="text-[10px] font-medium uppercase tracking-[0.08em] text-muted/60"
      >
        {label}
      </label>

      <div className="relative">
        <input
          ref={inputRef}
          id={inputId}
          type="date"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className="absolute inset-0 z-10 h-full w-full cursor-pointer opacity-0"
          style={{ colorScheme: "light" }}
        />

        <div
          onClick={() => inputRef.current?.showPicker()}
          className={cn(
            "group flex h-[36px] w-full cursor-pointer items-center gap-2 rounded-[10px] border px-2.5 transition-all duration-150",
            "shadow-[inset_0_1px_0_rgba(255,255,255,0.65)]",
            value
              ? "border-border bg-surface"
              : "border-border-subtle bg-subtle/35",
            "hover:border-border hover:bg-surface",
            "focus-within:border-primary/30 focus-within:bg-surface focus-within:shadow-[0_0_0_4px_rgba(79,106,255,0.12),inset_0_1px_0_rgba(255,255,255,0.65)]",
          )}
        >
          <CalendarDays
            size={14}
            strokeWidth={1.8}
            className={cn(
              "shrink-0 transition-colors",
              value ? "text-primary/60" : "text-muted/40 group-hover:text-muted/55",
            )}
          />

          <span
            className={cn(
              "min-w-0 flex-1 truncate text-[13px] transition-colors",
              value
                ? "font-medium text-foreground"
                : "text-muted/45",
            )}
          >
            {value ? formatDate(value) : "—"}
          </span>

          {value && (
            <button
              type="button"
              aria-label="Clear date"
              onClick={(e) => {
                e.stopPropagation();
                onChange("");
              }}
              className="flex h-[22px] w-[22px] shrink-0 items-center justify-center rounded-[6px] -mr-0.5 text-muted/40 transition-colors hover:bg-subtle hover:text-muted"
            >
              <X size={12} strokeWidth={2} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
