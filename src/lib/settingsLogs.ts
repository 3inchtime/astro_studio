import type { LogEntry, RuntimeLogEntry } from "../types";

export function formatStructuredText(value: string): string {
  try {
    return JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    return value;
  }
}

export function formatRuntimeLogEntry(log: RuntimeLogEntry): string {
  return [
    `[${log.timestamp}] [${log.level.toUpperCase()}] ${log.target}`,
    log.message,
  ].join("\n");
}

export function formatRuntimeLogs(logs: RuntimeLogEntry[]): string {
  return logs.map(formatRuntimeLogEntry).join("\n\n");
}

export function formatPersistedLog(log: LogEntry, responseContent: string | null): string {
  const lines = [
    `Time: ${log.timestamp}`,
    `Type: ${log.log_type}`,
    `Level: ${log.level.toUpperCase()}`,
  ];

  if (log.generation_id) {
    lines.push(`Generation ID: ${log.generation_id}`);
  }

  lines.push("Message:", log.message);

  if (log.metadata) {
    lines.push("", "Metadata:", formatStructuredText(log.metadata));
  }

  if (responseContent) {
    lines.push("", "Raw Response:", formatStructuredText(responseContent));
  }

  return lines.join("\n");
}

export async function copyTextToClipboard(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }

  const textArea = document.createElement("textarea");
  textArea.value = text;
  textArea.setAttribute("readonly", "");
  textArea.style.position = "fixed";
  textArea.style.opacity = "0";
  document.body.appendChild(textArea);
  textArea.select();
  const copied = document.execCommand("copy");
  textArea.remove();

  if (!copied) {
    throw new Error("Copy failed");
  }
}

export function mergeRuntimeLogs(
  current: RuntimeLogEntry[],
  incoming: RuntimeLogEntry[],
): RuntimeLogEntry[] {
  const bySequence = new Map<number, RuntimeLogEntry>();

  for (const entry of [...current, ...incoming]) {
    bySequence.set(entry.sequence, entry);
  }

  return Array.from(bySequence.values())
    .sort((a, b) => b.sequence - a.sequence)
    .slice(0, 200);
}
