import { createRef } from "react";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { LogsPanel } from "./LogsPanel";
import type { LogEntry } from "../../types";

const selectedLog: LogEntry = {
  id: "log-1",
  timestamp: "2026-05-17T13:00:00Z",
  log_type: "api_request",
  level: "info",
  message: "Request started",
  generation_id: "generation-1",
  metadata: null,
  response_file: null,
};

const translations: Record<string, string> = {
  "log.liveTitle": "实时运行日志",
  "log.liveConnected": "正在流式传输",
  "log.liveDesc": "显示运行中应用的最新日志输出",
  "log.autoScroll": "自动滚动",
  "log.copyRuntimeLogs": "复制日志",
  "log.clearView": "清空视图",
  "log.liveEmpty": "等待运行日志...",
  "log.liveHint": "保持此页面打开时，新日志会显示在这里。",
  "log.liveRecent": "最新 {{count}} 条",
  "log.allTypes": "全部类型",
  "log.allLevels": "全部级别",
  "log.retentionDays": "保留时间",
  "log.days": "天",
  "log.clearLogs": "清除日志",
  "log.filterType": "类型",
  "log.filterLevel": "级别",
  "log.totalCount": "{{count}} 条记录",
  "log.detail": "详情",
  "log.copyLog": "复制日志",
  "log.time": "时间",
  "log.type": "类型",
  "log.level": "级别",
  "log.message": "消息",
  "log.generationId": "生成 ID",
};

describe("LogsPanel", () => {
  it("uses localized labels for log table and detail metadata", () => {
    render(
      <LogsPanel
        t={((key: string, options?: { count?: number }) =>
          (translations[key] ?? key).replace(
            "{{count}}",
            String(options?.count ?? ""),
          )) as never}
        logs={[selectedLog]}
        totalLogs={1}
        logPage={1}
        totalPages={1}
        logType=""
        logLevel=""
        logSettings={{ enabled: true, retention_days: 7 }}
        selectedLog={selectedLog}
        responseContent={null}
        runtimeLogs={[]}
        runtimeLogsRef={createRef<HTMLDivElement>()}
        autoScrollRuntimeLogs
        copiedLogTarget={null}
        onAutoScrollRuntimeLogsChange={vi.fn()}
        onCopyText={vi.fn()}
        onClearRuntimeLogs={vi.fn()}
        onLogTypeChange={vi.fn()}
        onLogLevelChange={vi.fn()}
        onSaveRetention={vi.fn()}
        onOpenClearLogs={vi.fn()}
        onSelectLog={vi.fn()}
        onLogPageChange={vi.fn()}
        onCloseSelectedLog={vi.fn()}
      />,
    );

    expect(screen.getAllByText("时间")).toHaveLength(2);
    expect(screen.getAllByText("消息")).toHaveLength(2);
    expect(screen.getAllByText("类型").length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("级别").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("生成 ID")).toBeInTheDocument();
  });
});
