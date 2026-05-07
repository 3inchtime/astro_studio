import { describe, expect, it } from "vitest";
import {
  formatPersistedLog,
  formatRuntimeLogs,
  mergeRuntimeLogs,
} from "./settingsLogs";
import type { LogEntry, RuntimeLogEntry } from "../types";

describe("settings log helpers", () => {
  it("formats runtime logs newest-first exactly as copied from Settings", () => {
    const logs: RuntimeLogEntry[] = [
      {
        sequence: 2,
        timestamp: "2026-04-28T09:01:00.000Z",
        level: "warn",
        target: "astro_studio",
        message: "fresh log",
      },
      {
        sequence: 1,
        timestamp: "2026-04-28T09:00:00.000Z",
        level: "info",
        target: "astro_studio",
        message: "older log",
      },
    ];

    expect(formatRuntimeLogs(logs)).toBe(
      [
        "[2026-04-28T09:01:00.000Z] [WARN] astro_studio",
        "fresh log",
        "",
        "[2026-04-28T09:00:00.000Z] [INFO] astro_studio",
        "older log",
      ].join("\n"),
    );
  });

  it("formats persisted logs with pretty JSON metadata and response content", () => {
    const log: LogEntry = {
      id: "log-1",
      timestamp: "2026-04-28T09:02:00.000Z",
      log_type: "generation",
      level: "error",
      message: "persisted failure",
      generation_id: "generation-1",
      metadata: "{\"reason\":\"bad request\"}",
      response_file: null,
    };

    expect(formatPersistedLog(log, "{\"raw\":true}")).toBe(
      [
        "Time: 2026-04-28T09:02:00.000Z",
        "Type: generation",
        "Level: ERROR",
        "Generation ID: generation-1",
        "Message:",
        "persisted failure",
        "",
        "Metadata:",
        "{",
        "  \"reason\": \"bad request\"",
        "}",
        "",
        "Raw Response:",
        "{",
        "  \"raw\": true",
        "}",
      ].join("\n"),
    );
  });

  it("deduplicates runtime logs by sequence, sorts descending, and keeps the newest 200", () => {
    const current = Array.from({ length: 198 }, (_, index) => ({
      sequence: index + 1,
      timestamp: `t-${index + 1}`,
      level: "info",
      target: "astro_studio",
      message: `log-${index + 1}`,
    }));
    const incoming: RuntimeLogEntry[] = [
      {
        sequence: 1,
        timestamp: "replacement",
        level: "warn",
        target: "astro_studio",
        message: "replacement log",
      },
      {
        sequence: 250,
        timestamp: "newest",
        level: "error",
        target: "astro_studio",
        message: "newest log",
      },
      {
        sequence: 249,
        timestamp: "second newest",
        level: "debug",
        target: "astro_studio",
        message: "second newest log",
      },
    ];

    const merged = mergeRuntimeLogs(current, incoming);

    expect(merged).toHaveLength(200);
    expect(merged[0]?.sequence).toBe(250);
    expect(merged[1]?.sequence).toBe(249);
    expect(merged[merged.length - 1]?.sequence).toBe(1);
    expect(merged[merged.length - 1]?.message).toBe("replacement log");
  });
});
