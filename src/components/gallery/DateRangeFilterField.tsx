import { useEffect, useMemo, useRef, useState } from "react";
import { CalendarDays, ChevronDown } from "lucide-react";
import { DayPicker, type DayPickerLocale } from "react-day-picker";
import "react-day-picker/style.css";
import {
  buildPresetRange,
  fromDayPickerRange,
  toDayPickerRange,
  type DateRangeFilterValue,
  type DateRangePresetId,
} from "../../lib/dateRangeFilters";
import { cn } from "../../lib/utils";
import { useDialogFocus } from "../../hooks/useDialogFocus";

interface DateRangeFilterFieldProps {
  label: string;
  value: DateRangeFilterValue;
  displayValue: string;
  locale: DayPickerLocale;
  presets: {
    today: string;
    last7Days: string;
    last30Days: string;
    thisMonth: string;
    clear: string;
    done: string;
  };
  onChange: (value: DateRangeFilterValue) => void;
}

const presetOrder: DateRangePresetId[] = [
  "today",
  "last7Days",
  "last30Days",
  "thisMonth",
];

function getPreviousMonthStart(now = new Date()): Date {
  return new Date(now.getFullYear(), now.getMonth() - 1, 1);
}

export default function DateRangeFilterField({
  label,
  value,
  displayValue,
  locale,
  presets,
  onChange,
}: DateRangeFilterFieldProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const dialogRef = useDialogFocus({ open, onClose: () => setOpen(false) });
  const selectedRange = useMemo(() => toDayPickerRange(value), [value]);
  const displayStartMonth = useMemo(() => getPreviousMonthStart(), [open]);
  const hasValue = Boolean(value.from || value.to);

  useEffect(() => {
    if (!open) return;

    const handlePointerDown = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    };

    document.addEventListener("mousedown", handlePointerDown);
    return () => {
      document.removeEventListener("mousedown", handlePointerDown);
    };
  }, [open]);

  const presetLabels: Record<DateRangePresetId, string> = {
    today: presets.today,
    last7Days: presets.last7Days,
    last30Days: presets.last30Days,
    thisMonth: presets.thisMonth,
  };

  return (
    <div
      ref={rootRef}
      className="relative min-w-[184px] flex-[0_1_228px] xl:max-w-[228px]"
    >
      <button
        type="button"
        aria-label={`${label}: ${displayValue}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={(event) => {
          // Safari does not focus pointer-activated buttons automatically.
          if (!open) event.currentTarget.focus();
          setOpen((current) => !current);
        }}
        className={cn(
          "studio-control focus-ring group flex h-9 w-full items-center gap-2 rounded-[8px] px-2.5 text-left",
          hasValue
            ? "border-border bg-surface text-foreground"
            : "border-border-subtle bg-subtle/45",
          "hover:studio-control-hover",
          open
            ? "border-primary bg-surface"
            : "",
        )}
      >
        <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-[8px] bg-primary/[0.08] text-primary transition-colors group-hover:bg-primary/[0.12]">
          <CalendarDays size={14} strokeWidth={1.9} />
        </span>

        <span className="min-w-0 flex-1">
          <span className="block truncate text-xs font-medium text-foreground">
            {displayValue}
          </span>
        </span>

        <ChevronDown
          size={14}
          strokeWidth={1.9}
          className={cn(
            "shrink-0 text-muted transition-transform duration-150",
            open ? "rotate-180" : "",
          )}
        />
      </button>

      {open && (
        <div
          ref={dialogRef}
          role="dialog"
          aria-modal="true"
          aria-label={label}
          tabIndex={-1}
          className="studio-floating-panel absolute right-0 top-[calc(100%+8px)] z-30 w-[min(456px,calc(100vw-132px))] overflow-hidden rounded-[12px] p-2.5"
        >
          <div className="mb-2 flex flex-wrap gap-1">
            {presetOrder.map((presetId) => (
              <button
                key={presetId}
                type="button"
                onClick={() => onChange(buildPresetRange(presetId))}
                aria-label={presetLabels[presetId]}
                className="focus-ring min-w-0 rounded-[8px] border border-border bg-subtle px-2 py-1 text-xs font-medium text-foreground transition-colors hover:bg-surface"
              >
                {presetLabels[presetId]}
              </button>
            ))}
          </div>

          <div className="overflow-hidden rounded-[12px] border border-border-subtle bg-surface p-1.5">
            <div className="astro-day-picker-scale">
              <DayPicker
                mode="range"
                selected={selectedRange}
                onSelect={(range) => onChange(fromDayPickerRange(range))}
                showOutsideDays={false}
                numberOfMonths={2}
                defaultMonth={displayStartMonth}
                locale={locale}
                className="astro-day-picker"
                classNames={{
                  months: "rdp-months astro-day-picker-months",
                }}
              />
            </div>
          </div>

          <div className="mt-2 flex items-center justify-between gap-2">
            <div className="min-w-0">
              <p className="truncate text-xs font-medium text-foreground">
                {displayValue}
              </p>
              <p className="text-xs text-muted">
                {value.from && value.to
                  ? `${value.from} → ${value.to}`
                  : label}
              </p>
            </div>

            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={() => onChange({ from: "", to: "" })}
                className="studio-control focus-ring rounded-[8px] px-2 py-1 text-xs font-medium hover:studio-control-hover"
              >
                {presets.clear}
              </button>
              <button
                type="button"
                onClick={() => setOpen(false)}
                className="studio-control-primary focus-ring rounded-[8px] px-2.5 py-1 text-xs font-semibold"
              >
                {presets.done}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
