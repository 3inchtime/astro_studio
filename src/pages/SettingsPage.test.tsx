import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsPage from "./SettingsPage";

const getApiKey = vi.fn();
const getEndpointSettings = vi.fn();
const getFontSize = vi.fn();
const getImageModel = vi.fn();
const getLogSettings = vi.fn();
const getLogs = vi.fn();
const getRuntimeLogs = vi.fn();
const getTrashSettings = vi.fn();
const onRuntimeLog = vi.fn();
const clearLogs = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    i18n: {
      changeLanguage: vi.fn(),
      language: "en",
    },
    t: (key: string, options?: { count?: number }) =>
      options?.count === undefined ? key : `${key} ${options.count}`,
  }),
}));

vi.mock("../lib/api", () => ({
  clearLogs: (...args: unknown[]) => clearLogs(...args),
  getApiKey: (...args: unknown[]) => getApiKey(...args),
  getEndpointSettings: (...args: unknown[]) => getEndpointSettings(...args),
  getFontSize: (...args: unknown[]) => getFontSize(...args),
  getImageModel: (...args: unknown[]) => getImageModel(...args),
  getLogs: (...args: unknown[]) => getLogs(...args),
  getLogSettings: (...args: unknown[]) => getLogSettings(...args),
  getRuntimeLogs: (...args: unknown[]) => getRuntimeLogs(...args),
  getTrashSettings: (...args: unknown[]) => getTrashSettings(...args),
  onRuntimeLog: (...args: unknown[]) => onRuntimeLog(...args),
  readLogResponseFile: vi.fn(),
  saveApiKey: vi.fn(),
  saveEndpointSettings: vi.fn(),
  saveFontSize: vi.fn(),
  saveImageModel: vi.fn(),
  saveLogSettings: vi.fn(),
  saveTrashSettings: vi.fn(),
}));

function renderSettingsPage() {
  render(
    <MemoryRouter>
      <SettingsPage />
    </MemoryRouter>,
  );

  fireEvent.click(screen.getByRole("button", { name: "log.title" }));
}

describe("SettingsPage logs", () => {
  const writeText = vi.fn();

  beforeEach(() => {
    clearLogs.mockReset();
    getApiKey.mockReset();
    getEndpointSettings.mockReset();
    getFontSize.mockReset();
    getImageModel.mockReset();
    getLogSettings.mockReset();
    getLogs.mockReset();
    getRuntimeLogs.mockReset();
    getTrashSettings.mockReset();
    onRuntimeLog.mockReset();
    writeText.mockReset();

    clearLogs.mockResolvedValue(1);
    getApiKey.mockResolvedValue("");
    getEndpointSettings.mockResolvedValue({
      mode: "base_url",
      base_url: "https://api.openai.com/v1",
      generation_url: "https://api.openai.com/v1/images/generations",
      edit_url: "https://api.openai.com/v1/images/edits",
    });
    getFontSize.mockResolvedValue("medium");
    getImageModel.mockResolvedValue("gpt-image-2");
    getLogSettings.mockResolvedValue({ enabled: true, retention_days: 7 });
    getLogs.mockResolvedValue({ logs: [], total: 0, page: 1, page_size: 20 });
    getTrashSettings.mockResolvedValue({ retention_days: 30 });
    getRuntimeLogs.mockResolvedValue([
      {
        sequence: 1,
        timestamp: "2026-04-28T09:00:00.000Z",
        level: "info",
        target: "astro_studio",
        message: "older log",
      },
      {
        sequence: 2,
        timestamp: "2026-04-28T09:01:00.000Z",
        level: "warn",
        target: "astro_studio",
        message: "fresh log",
      },
    ]);
    onRuntimeLog.mockResolvedValue(vi.fn());
    writeText.mockResolvedValue(undefined);
    Object.assign(navigator, {
      clipboard: { writeText },
    });
  });

  it("shows the newest runtime logs at the top", async () => {
    renderSettingsPage();

    const freshLog = await screen.findByText("fresh log");
    const olderLog = screen.getByText("older log");

    expect(
      freshLog.compareDocumentPosition(olderLog) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("copies the visible runtime logs", async () => {
    renderSettingsPage();

    await screen.findByText("fresh log");
    fireEvent.click(screen.getByRole("button", { name: "log.copyRuntimeLogs" }));

    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith(
        [
          "[2026-04-28T09:01:00.000Z] [WARN] astro_studio",
          "fresh log",
          "",
          "[2026-04-28T09:00:00.000Z] [INFO] astro_studio",
          "older log",
        ].join("\n"),
      );
    });
  });

  it("copies the selected persisted log detail", async () => {
    getLogs.mockResolvedValue({
      logs: [
        {
          id: "log-1",
          timestamp: "2026-04-28T09:02:00.000Z",
          log_type: "generation",
          level: "error",
          message: "persisted failure",
          generation_id: "generation-1",
          metadata: "{\"reason\":\"bad request\"}",
          response_file: null,
        },
      ],
      total: 1,
      page: 1,
      page_size: 20,
    });

    renderSettingsPage();

    fireEvent.click(await screen.findByText("persisted failure"));
    fireEvent.click(screen.getByRole("button", { name: "log.copyLog" }));

    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith(
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
        ].join("\n"),
      );
    });
  });

  it("clears all persisted logs immediately when confirmed", async () => {
    getLogs.mockResolvedValue({
      logs: [
        {
          id: "log-1",
          timestamp: "2026-04-28T09:02:00.000Z",
          log_type: "generation",
          level: "error",
          message: "persisted failure",
          generation_id: null,
          metadata: null,
          response_file: null,
        },
      ],
      total: 1,
      page: 1,
      page_size: 20,
    });

    renderSettingsPage();

    await screen.findByText("persisted failure");
    fireEvent.click(screen.getByRole("button", { name: "log.clearLogs" }));
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "log.clearLogs" }));

    await waitFor(() => {
      expect(clearLogs).toHaveBeenCalledWith(0);
      expect(screen.queryByText("persisted failure")).not.toBeInTheDocument();
      expect(screen.getByText("log.noLogs")).toBeInTheDocument();
    });
  });
});
