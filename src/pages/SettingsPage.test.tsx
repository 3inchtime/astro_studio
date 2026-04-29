import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsPage from "./SettingsPage";

const getFontSize = vi.fn();
const getImageModel = vi.fn();
const getLogSettings = vi.fn();
const getLogs = vi.fn();
const getModelApiKey = vi.fn();
const getModelEndpointSettings = vi.fn();
const getRuntimeLogs = vi.fn();
const getTrashSettings = vi.fn();
const onRuntimeLog = vi.fn();
const clearLogs = vi.fn();
const saveModelApiKey = vi.fn();
const saveModelEndpointSettings = vi.fn();
const saveImageModel = vi.fn();

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
  getFontSize: (...args: unknown[]) => getFontSize(...args),
  getImageModel: (...args: unknown[]) => getImageModel(...args),
  getLogs: (...args: unknown[]) => getLogs(...args),
  getLogSettings: (...args: unknown[]) => getLogSettings(...args),
  getModelApiKey: (...args: unknown[]) => getModelApiKey(...args),
  getModelEndpointSettings: (...args: unknown[]) =>
    getModelEndpointSettings(...args),
  getRuntimeLogs: (...args: unknown[]) => getRuntimeLogs(...args),
  getTrashSettings: (...args: unknown[]) => getTrashSettings(...args),
  onRuntimeLog: (...args: unknown[]) => onRuntimeLog(...args),
  readLogResponseFile: vi.fn(),
  saveImageModel: (...args: unknown[]) => saveImageModel(...args),
  saveFontSize: vi.fn(),
  saveModelApiKey: (...args: unknown[]) => saveModelApiKey(...args),
  saveModelEndpointSettings: (...args: unknown[]) =>
    saveModelEndpointSettings(...args),
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

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });

  return { promise, resolve, reject };
}

describe("SettingsPage logs", () => {
  const writeText = vi.fn();

  beforeEach(() => {
    clearLogs.mockReset();
    getFontSize.mockReset();
    getImageModel.mockReset();
    getLogSettings.mockReset();
    getLogs.mockReset();
    getModelApiKey.mockReset();
    getModelEndpointSettings.mockReset();
    getRuntimeLogs.mockReset();
    getTrashSettings.mockReset();
    onRuntimeLog.mockReset();
    saveImageModel.mockReset();
    saveModelApiKey.mockReset();
    saveModelEndpointSettings.mockReset();
    writeText.mockReset();

    clearLogs.mockResolvedValue(1);
    getModelApiKey.mockImplementation(async (model: string) =>
      model === "nano-banana" ? "gemini-key" : "",
    );
    getModelEndpointSettings.mockImplementation(async (model: string) => ({
      mode: "base_url",
      base_url:
        model === "nano-banana"
          ? "https://generativelanguage.googleapis.com/v1beta/models"
          : "https://api.openai.com/v1",
      generation_url:
        model === "nano-banana"
          ? "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent"
          : "https://api.openai.com/v1/images/generations",
      edit_url:
        model === "nano-banana"
          ? "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent"
          : "https://api.openai.com/v1/images/edits",
    }));
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

  it("shows Nano Banana models in the model selector", async () => {
    render(
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    expect(
      await screen.findByRole("option", { name: "Nano Banana" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("option", { name: "Nano Banana 2" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("option", { name: "Nano Banana Pro" }),
    ).toBeInTheDocument();
  });

  it("renders every registered model from the shared catalog", async () => {
    vi.resetModules();
    vi.doMock("../lib/modelCatalog", () => {
      const IMAGE_MODEL_CATALOG = [
        {
          id: "gpt-image-2",
          label: "GPT Image 2",
          provider: "openai",
          providerModelId: "gpt-image-2",
          supportsEdit: true,
          connectionDefaults: {
            baseUrl: "https://api.openai.com/v1",
            generationUrl: "https://api.openai.com/v1/images/generations",
            editUrl: "https://api.openai.com/v1/images/edits",
          },
          parameterDefaults: {},
          parameterCapabilities: {},
        },
        {
          id: "catalog-test-model",
          label: "Catalog Test Model",
          provider: "test-provider",
          providerModelId: "provider-test-model",
          supportsEdit: false,
          connectionDefaults: {
            baseUrl: "https://example.com/v1",
            generationUrl: "https://example.com/v1/images/generations",
            editUrl: "https://example.com/v1/images/edits",
          },
          parameterDefaults: {},
          parameterCapabilities: {},
        },
      ];

      return {
        IMAGE_MODEL_CATALOG,
        getImageModelCatalogEntry: (model: string) =>
          IMAGE_MODEL_CATALOG.find((entry) => entry.id === model) ??
          IMAGE_MODEL_CATALOG[0],
      };
    });

    const { default: CatalogSettingsPage } = await import("./SettingsPage");

    render(
      <MemoryRouter>
        <CatalogSettingsPage />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    const modelSelect = await screen.findByDisplayValue("GPT Image 2");
    const optionNames = within(modelSelect)
      .getAllByRole("option")
      .map((option) => option.textContent);

    expect(optionNames).toEqual(["GPT Image 2", "Catalog Test Model"]);

    vi.doUnmock("../lib/modelCatalog");
  });

  it("defines label, provider, and edit support metadata for every registered model", async () => {
    const { IMAGE_MODEL_CATALOG, getImageModelCatalogEntry } = await import("../lib/modelCatalog");
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    expect(IMAGE_MODEL_CATALOG).not.toHaveLength(0);
    for (const entry of IMAGE_MODEL_CATALOG) {
      expect(entry.label).toBeTruthy();
      expect(typeof entry.label).toBe("string");
      expect(entry.provider).toBeTruthy();
      expect(typeof entry.provider).toBe("string");
      expect(typeof entry.supportsEdit).toBe("boolean");
    }

    expect(getImageModelCatalogEntry("missing-model" as never).id).toBe("gpt-image-2");
    expect(consoleError).toHaveBeenCalledWith(
      expect.objectContaining({
        message: expect.stringContaining("Unknown image model"),
      }),
    );

    consoleError.mockRestore();
  });

  it("loads model-specific credentials when switching models", async () => {
    render(
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    const modelSelect = await screen.findByDisplayValue("GPT Image 2");
    fireEvent.change(modelSelect, {
      target: { value: "nano-banana" },
    });

    await waitFor(() => {
      expect(getModelApiKey).toHaveBeenCalledWith("nano-banana");
      expect(getModelEndpointSettings).toHaveBeenCalledWith(
        "nano-banana",
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "settings.showKey" }));
    expect(screen.getByDisplayValue("gemini-key")).toBeInTheDocument();
    expect(
      screen.getByDisplayValue(
        "https://generativelanguage.googleapis.com/v1beta/models",
      ),
    ).toBeInTheDocument();
  });

  it("ignores stale async model settings from a previous model selection", async () => {
    const gptKey = createDeferred<string | null>();
    const geminiKey = createDeferred<string | null>();
    const gptEndpointSettings = createDeferred<{
      mode: "base_url";
      base_url: string;
      generation_url: string;
      edit_url: string;
    }>();
    const geminiEndpointSettings = createDeferred<{
      mode: "base_url";
      base_url: string;
      generation_url: string;
      edit_url: string;
    }>();

    getModelApiKey.mockImplementation((model: string) => {
      if (model === "gpt-image-2") {
        return gptKey.promise;
      }

      return geminiKey.promise;
    });
    getModelEndpointSettings.mockImplementation((model: string) => {
      if (model === "gpt-image-2") {
        return gptEndpointSettings.promise;
      }

      return geminiEndpointSettings.promise;
    });

    render(
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    const modelSelect = await screen.findByDisplayValue("GPT Image 2");
    fireEvent.change(modelSelect, {
      target: { value: "nano-banana" },
    });

    await act(async () => {
      geminiKey.resolve("gemini-key");
      geminiEndpointSettings.resolve({
        mode: "base_url",
        base_url: "https://generativelanguage.googleapis.com/v1beta/models",
        generation_url:
          "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent",
        edit_url:
          "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent",
      });
    });

    fireEvent.click(screen.getByRole("button", { name: "settings.showKey" }));

    await waitFor(() => {
      expect(screen.getByDisplayValue("gemini-key")).toBeInTheDocument();
      expect(
        screen.getByDisplayValue(
          "https://generativelanguage.googleapis.com/v1beta/models",
        ),
      ).toBeInTheDocument();
    });

    await act(async () => {
      gptKey.resolve("stale-openai-key");
      gptEndpointSettings.resolve({
        mode: "base_url",
        base_url: "https://api.openai.com/v1",
        generation_url: "https://api.openai.com/v1/images/generations",
        edit_url: "https://api.openai.com/v1/images/edits",
      });
    });

    expect(screen.getByDisplayValue("gemini-key")).toBeInTheDocument();
    expect(screen.queryByDisplayValue("stale-openai-key")).not.toBeInTheDocument();
    expect(
      screen.getByDisplayValue(
        "https://generativelanguage.googleapis.com/v1beta/models",
      ),
    ).toBeInTheDocument();
  });

  it("resets connection fields to the selected model before saving", async () => {
    const geminiKey = createDeferred<string | null>();
    const geminiEndpointSettings = createDeferred<{
      mode: "base_url";
      base_url: string;
      generation_url: string;
      edit_url: string;
    }>();

    getModelApiKey.mockImplementation((model: string) => {
      if (model === "nano-banana") {
        return geminiKey.promise;
      }

      return Promise.resolve("openai-key");
    });
    getModelEndpointSettings.mockImplementation((model: string) => {
      if (model === "nano-banana") {
        return geminiEndpointSettings.promise;
      }

      return Promise.resolve({
        mode: "full_url",
        base_url: "https://api.openai.com/v1",
        generation_url: "https://api.openai.com/v1/images/generations",
        edit_url: "https://api.openai.com/v1/images/edits",
      });
    });

    render(
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    await screen.findByDisplayValue("https://api.openai.com/v1/images/generations");
    fireEvent.click(screen.getByRole("button", { name: "settings.showKey" }));
    expect(screen.getByDisplayValue("openai-key")).toBeInTheDocument();

    fireEvent.change(screen.getByDisplayValue("GPT Image 2"), {
      target: { value: "nano-banana" },
    });

    await waitFor(() => {
      expect(
        screen.getByDisplayValue(
          "https://generativelanguage.googleapis.com/v1beta/models",
        ),
      ).toBeInTheDocument();
    });

    expect(screen.getByRole("button", { name: "settings.saveKey" })).toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: "settings.saveUrl" }));

    await waitFor(() => {
      expect(saveModelEndpointSettings).toHaveBeenCalledWith(
        "nano-banana",
        {
          mode: "base_url",
          base_url: "https://generativelanguage.googleapis.com/v1beta/models",
          generation_url:
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent",
          edit_url:
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent",
        },
      );
    });

    expect(saveModelApiKey).not.toHaveBeenCalled();
  });

  it("saves the API key for the currently selected model", async () => {
    render(
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    const modelSelect = await screen.findByDisplayValue("GPT Image 2");
    fireEvent.change(modelSelect, {
      target: { value: "nano-banana" },
    });

    await waitFor(() => {
      expect(
        screen.getByDisplayValue(
          "https://generativelanguage.googleapis.com/v1beta/models",
        ),
      ).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "settings.showKey" }));
    fireEvent.change(screen.getByDisplayValue("gemini-key"), {
      target: { value: "fresh-gemini-key" },
    });
    fireEvent.click(screen.getByRole("button", { name: "settings.saveKey" }));

    await waitFor(() => {
      expect(saveModelApiKey).toHaveBeenCalledWith(
        "nano-banana",
        "fresh-gemini-key",
      );
    });
  });

  it("does not let a late persisted model overwrite a manual selection", async () => {
    const persistedModel = createDeferred<string>();

    getImageModel.mockReturnValue(persistedModel.promise);

    render(
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    const modelSelect = await screen.findByDisplayValue("GPT Image 2");
    fireEvent.change(modelSelect, {
      target: { value: "nano-banana" },
    });

    await waitFor(() => {
      expect(getModelApiKey).toHaveBeenCalledWith("nano-banana");
    });

    await act(async () => {
      persistedModel.resolve("nano-banana-pro");
    });

    expect(screen.getByDisplayValue("Nano Banana")).toBeInTheDocument();
    expect(
      screen.queryByDisplayValue("Nano Banana Pro"),
    ).not.toBeInTheDocument();
    expect(
      screen.getByDisplayValue(
        "https://generativelanguage.googleapis.com/v1beta/models",
      ),
    ).toBeInTheDocument();
  });

  it("ignores late save completions after switching to another model", async () => {
    const keySave = createDeferred<void>();
    const endpointSave = createDeferred<void>();

    saveModelApiKey.mockReturnValue(keySave.promise);
    saveModelEndpointSettings.mockReturnValue(endpointSave.promise);

    render(
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>,
    );

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    await screen.findByDisplayValue("GPT Image 2");
    fireEvent.click(screen.getByRole("button", { name: "settings.showKey" }));
    fireEvent.change(screen.getByDisplayValue(""), {
      target: { value: "pending-openai-key" },
    });
    fireEvent.click(screen.getByRole("button", { name: "settings.saveKey" }));

    fireEvent.click(
      screen.getByRole("button", { name: "settings.endpointFullUrlMode" }),
    );
    fireEvent.change(
      screen.getByDisplayValue("https://api.openai.com/v1/images/generations"),
      {
        target: { value: "https://example.com/custom-generation" },
      },
    );
    fireEvent.click(screen.getByRole("button", { name: "settings.saveUrl" }));

    fireEvent.change(screen.getByDisplayValue("GPT Image 2"), {
      target: { value: "nano-banana" },
    });

    await waitFor(() => {
      expect(screen.getByDisplayValue("Nano Banana")).toBeInTheDocument();
      expect(
        screen.getByDisplayValue(
          "https://generativelanguage.googleapis.com/v1beta/models",
        ),
      ).toBeInTheDocument();
    });

    await act(async () => {
      keySave.resolve();
      endpointSave.resolve();
    });

    expect(screen.queryAllByText("settings.saved")).toHaveLength(0);
    expect(
      screen.getByDisplayValue(
        "https://generativelanguage.googleapis.com/v1beta/models",
      ),
    ).toBeInTheDocument();
    expect(
      screen.queryByDisplayValue("https://example.com/custom-generation"),
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "settings.showKey" }));
    expect(screen.getByDisplayValue("gemini-key")).toBeInTheDocument();
    expect(
      screen.queryByDisplayValue("pending-openai-key"),
    ).not.toBeInTheDocument();
  });
});
