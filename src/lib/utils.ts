import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

function parseTimestamp(dateStr: string): Date {
  const legacyUtcPattern = /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/;
  const normalized = legacyUtcPattern.test(dateStr)
    ? `${dateStr.replace(" ", "T")}Z`
    : dateStr;

  return new Date(normalized);
}

export function formatTimeAgo(dateStr: string, t: (key: string, options?: Record<string, unknown>) => string): string {
  const date = parseTimestamp(dateStr);
  if (Number.isNaN(date.getTime())) return dateStr;

  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  if (diffMin < 1) return t("time.justNow");
  if (diffMin < 60) return t("time.minutesAgo", { count: diffMin });
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return t("time.hoursAgo", { count: diffHr });
  const diffDay = Math.floor(diffHr / 24);
  if (diffDay < 7) return t("time.daysAgo", { count: diffDay });
  return date.toLocaleDateString();
}

export function formatConversationTime(dateStr: string): string {
  const date = parseTimestamp(dateStr);
  if (Number.isNaN(date.getTime())) return dateStr;

  const now = new Date();
  const isSameYear = date.getFullYear() === now.getFullYear();
  const isToday = isSameYear && date.getMonth() === now.getMonth() && date.getDate() === now.getDate();

  if (isToday) {
    return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }

  return date.toLocaleString([], {
    month: "2-digit",
    day: "2-digit",
    ...(isSameYear ? {} : { year: "numeric" }),
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function formatLocalDateTime(dateStr: string): string {
  const date = parseTimestamp(dateStr);
  if (Number.isNaN(date.getTime())) return dateStr;

  return date.toLocaleString([], {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function toLocalDate(dateStr: string): Date {
  return parseTimestamp(dateStr);
}
