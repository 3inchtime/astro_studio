import { invoke } from "@tauri-apps/api/core";
import { listenDesktopEvent } from "../../lib/desktop";
import type { LogSearchResult, LogEntry, RuntimeLogEntry } from "../../types";

export function onRuntimeLog(handler: (data: RuntimeLogEntry) => void) {
  return listenDesktopEvent("runtime-log:new", handler);
}

export async function getLogs(
  logType?: string,
  level?: string,
  page?: number,
  pageSize?: number,
): Promise<LogSearchResult> {
  return invoke("get_logs", {
    logType: logType || null,
    level: level || null,
    page,
    pageSize,
  });
}

export async function getRuntimeLogs(
  limit?: number,
): Promise<RuntimeLogEntry[]> {
  return invoke("get_runtime_logs", { limit });
}

export async function getLogDetail(id: string): Promise<LogEntry> {
  return invoke("get_log_detail", { id });
}

export async function readLogResponseFile(path: string): Promise<string> {
  return invoke("read_log_response_file", { path });
}

export async function clearLogs(beforeDays?: number): Promise<number> {
  return invoke("clear_logs", { beforeDays: beforeDays ?? null });
}
