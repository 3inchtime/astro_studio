import type { DateRange } from "react-day-picker";

export interface DateRangeFilterValue {
  from: string;
  to: string;
}

export type DateRangePresetId =
  | "today"
  | "last7Days"
  | "last30Days"
  | "thisMonth";

function startOfLocalDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

function endOfMonth(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth() + 1, 0);
}

function shiftDays(date: Date, days: number): Date {
  const shifted = new Date(date);
  shifted.setDate(shifted.getDate() + days);
  return shifted;
}

export function formatDateInputValue(date: Date): string {
  const year = date.getFullYear();
  const month = `${date.getMonth() + 1}`.padStart(2, "0");
  const day = `${date.getDate()}`.padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function parseDateInputValue(value: string): Date | undefined {
  if (!value) return undefined;

  const [year, month, day] = value.split("-").map(Number);
  if (!year || !month || !day) return undefined;

  return new Date(year, month - 1, day);
}

export function toDayPickerRange(
  value: DateRangeFilterValue,
): DateRange | undefined {
  const from = parseDateInputValue(value.from);
  const to = parseDateInputValue(value.to);

  if (!from && !to) return undefined;
  return { from, to };
}

export function fromDayPickerRange(
  range: DateRange | undefined,
): DateRangeFilterValue {
  return {
    from: range?.from ? formatDateInputValue(range.from) : "",
    to: range?.to ? formatDateInputValue(range.to) : "",
  };
}

export function buildPresetRange(
  preset: DateRangePresetId,
  now = new Date(),
): DateRangeFilterValue {
  const today = startOfLocalDay(now);

  switch (preset) {
    case "today":
      return {
        from: formatDateInputValue(today),
        to: formatDateInputValue(today),
      };
    case "last7Days":
      return {
        from: formatDateInputValue(shiftDays(today, -6)),
        to: formatDateInputValue(today),
      };
    case "last30Days":
      return {
        from: formatDateInputValue(shiftDays(today, -29)),
        to: formatDateInputValue(today),
      };
    case "thisMonth": {
      const monthStart = new Date(today.getFullYear(), today.getMonth(), 1);
      return {
        from: formatDateInputValue(monthStart),
        to: formatDateInputValue(endOfMonth(today)),
      };
    }
  }
}

function formatDisplayDate(
  value: string,
  locale?: string,
): string {
  const parsed = parseDateInputValue(value);
  if (!parsed) return value;

  return parsed.toLocaleDateString(locale, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

export function formatDateRangeDisplay(
  value: DateRangeFilterValue,
  emptyLabel: string,
  locale?: string,
): string {
  const { from, to } = value;

  if (!from && !to) return emptyLabel;
  if (from && to) {
    return `${formatDisplayDate(from, locale)} - ${formatDisplayDate(to, locale)}`;
  }
  if (from) {
    return `${formatDisplayDate(from, locale)} -`;
  }
  return `- ${formatDisplayDate(to, locale)}`;
}
