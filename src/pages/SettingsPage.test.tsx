import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { MemoryRouter } from "react-router-dom";
import type { ReactElement } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsPage from "./SettingsPage";

const getFontSize = vi.fn();
const getImageModel = vi.fn();
const getLogSettings = vi.fn();
const getLlmConfigs = vi.fn();
const getLogs = vi.fn();
const getModelProviderProfiles = vi.fn();
const getRuntimeLogs = vi.fn();
const getTrashSettings = vi.fn();
const onRuntimeLog = vi.fn();
const clearLogs = vi.fn();
const saveModelProviderProfiles = vi.fn();
const createModelProviderProfile = vi.fn();
const deleteModelProviderProfile = vi.fn();
const setActiveModelProvider = vi.fn();
const saveImageModel = vi.fn();
const saveLlmConfigs = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    i18n: {
      changeLanguage: vi.fn(),
      language: "en",
      resolvedLanguage: "en",
    },
    t: (key: string, options?: { count?: number }) =>
      options?.count === undefined ? key : `${key} ${options.count}`,
  }),
}));

vi.mock("../lib/api", () => ({
  clearLogs: (...args: unknown[]) => clearLogs(...args),
  getFontSize: (...args: unknown[]) => getFontSize(...args),
  getImageModel: (...args: unknown[]) => getImageModel(...args),
  getLlmConfigs: (...args: unknown[]) => getLlmConfigs(...args),
  getLogs: (...args: unknown[]) => getLogs(...args),
  getLogSettings: (...args: unknown[]) => getLogSettings(...args),
  getModelProviderProfiles: (...args: unknown[]) => getModelProviderProfiles(...args),
  getRuntimeLogs: (...args: unknown[]) => getRuntimeLogs(...args),
  getTrashSettings: (...args: unknown[]) => getTrashSettings(...args),
  onRuntimeLog: (...args: unknown[]) => onRuntimeLog(...args),
  readLogResponseFile: vi.fn(),
  saveImageModel: (...args: unknown[]) => saveImageModel(...args),
  saveFontSize: vi.fn(),
  saveLlmConfigs: (...args: unknown[]) => saveLlmConfigs(...args),
  saveModelProviderProfiles: (...args: unknown[]) => saveModelProviderProfiles(...args),
  createModelProviderProfile: (...args: unknown[]) => createModelProviderProfile(...args),
  deleteModelProviderProfile: (...args: unknown[]) => deleteModelProviderProfile(...args),
  setActiveModelProvider: (...args: unknown[]) => setActiveModelProvider(...args),
  saveLogSettings: vi.fn(),
  saveTrashSettings: vi.fn(),
}));

const openAiProviderState = {
  active_provider_id: "openai-official",
  profiles: [
    {
      id: "openai-official",
      name: "OpenAI Official",
      api_key: "openai-key",
      endpoint_settings: {
        mode: "base_url",
        base_url: "https://api.openai.com/v1",
        generation_url: "https://api.openai.com/v1/images/generations",
        edit_url: "https://api.openai.com/v1/images/edits",
      },
    },
    {
      id: "company-gateway",
      name: "Company Gateway",
      api_key: "gateway-key",
      endpoint_settings: {
        mode: "full_url",
        base_url: "https://gateway.example/v1",
        generation_url: "https://gateway.example/generate",
        edit_url: "https://gateway.example/edit",
      },
    },
  ],
} as const;

const geminiProviderState = {
  active_provider_id: "gemini-official",
  profiles: [
    {
      id: "gemini-official",
      name: "Gemini Official",
      api_key: "gemini-key",
      endpoint_settings: {
        mode: "base_url",
        base_url: "https://generativelanguage.googleapis.com/v1beta/models",
        generation_url:
          "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent",
        edit_url:
          "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent",
      },
    },
  ],
} as const;

function renderWithProviders(ui: ReactElement) {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });

  render(
    <QueryClientProvider client={queryClient}>
      <MemoryRouter>{ui}</MemoryRouter>
    </QueryClientProvider>,
  );
}

function renderSettingsPage() {
  renderWithProviders(<SettingsPage />);
}

async function clickModelCard(name: string) {
  const card = await screen.findByRole("button", { name: `Select ${name} model` });
  await act(async () => {
    fireEvent.click(card);
  });
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
    getLlmConfigs.mockReset();
    getLogs.mockReset();
    getModelProviderProfiles.mockReset();
    getRuntimeLogs.mockReset();
    getTrashSettings.mockReset();
    onRuntimeLog.mockReset();
    saveImageModel.mockReset();
    saveLlmConfigs.mockReset();
    saveModelProviderProfiles.mockReset();
    createModelProviderProfile.mockReset();
    deleteModelProviderProfile.mockReset();
    setActiveModelProvider.mockReset();
    writeText.mockReset();

    clearLogs.mockResolvedValue(1);
    getModelProviderProfiles.mockImplementation(async (model: string) =>
      model === "nano-banana" ? geminiProviderState : openAiProviderState,
    );
    saveModelProviderProfiles.mockImplementation(async (_model: string, state: unknown) => state);
    createModelProviderProfile.mockResolvedValue({
      active_provider_id: "openai-official",
      profiles: [
        ...openAiProviderState.profiles,
        {
          id: "new-provider",
          name: "New Provider",
          api_key: "",
          endpoint_settings: {
            mode: "base_url",
            base_url: "https://api.openai.com/v1",
            generation_url: "https://api.openai.com/v1/images/generations",
            edit_url: "https://api.openai.com/v1/images/edits",
          },
        },
      ],
    });
    deleteModelProviderProfile.mockResolvedValue({
      active_provider_id: "openai-official",
      profiles: [openAiProviderState.profiles[0]],
    });
    setActiveModelProvider.mockResolvedValue({
      ...openAiProviderState,
      active_provider_id: "company-gateway",
    });
    getFontSize.mockResolvedValue("medium");
    getImageModel.mockResolvedValue("gpt-image-2");
    getLlmConfigs.mockResolvedValue([]);
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

    fireEvent.click(screen.getByRole("button", { name: "log.title" }));

    const freshLog = await screen.findByText("fresh log");
    const olderLog = screen.getByText("older log");

    expect(
      freshLog.compareDocumentPosition(olderLog) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("renders a top settings navigation and switches sections from it", async () => {
    renderSettingsPage();

    const settingsNav = screen.getByRole("navigation", { name: "settings.sections" });

    expect(
      within(settingsNav).getByRole("button", { name: "settings.general" }),
    ).toHaveAttribute("aria-current", "page");

    await act(async () => {
      fireEvent.click(
        within(settingsNav).getByRole("button", { name: "settings.modelConfig" }),
      );
    });

    expect(
      await screen.findByRole("heading", { name: "settings.modelConfig" }),
    ).toBeInTheDocument();
  });

  it("copies the visible runtime logs", async () => {
    renderSettingsPage();

    fireEvent.click(screen.getByRole("button", { name: "log.title" }));

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

    fireEvent.click(screen.getByRole("button", { name: "log.title" }));

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

    fireEvent.click(screen.getByRole("button", { name: "log.title" }));

    await screen.findByText("persisted failure");
    fireEvent.click(screen.getByRole("button", { name: "log.clearLogs" }));
    fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "log.clearLogs" }));

    await waitFor(() => {
      expect(clearLogs).toHaveBeenCalledWith(0);
      expect(screen.queryByText("persisted failure")).not.toBeInTheDocument();
      expect(screen.getByText("log.noLogs")).toBeInTheDocument();
    });
  });

  it("shows Nano Banana models as direct selection cards", async () => {
    renderSettingsPage();

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    expect(
      await screen.findByRole("button", { name: "Select Nano Banana model" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Select Nano Banana 2 model" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Select Nano Banana Pro model" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: "settings.model" })).not.toBeInTheDocument();
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

    renderWithProviders(<CatalogSettingsPage />);

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    expect(await screen.findByRole("button", { name: "Select GPT Image 2 model" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Select Catalog Test Model model" }),
    ).toBeInTheDocument();

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

  it("loads provider profiles for the selected model", async () => {
    renderSettingsPage();

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    expect(
      await screen.findByRole("button", { name: "Select OpenAI Official provider" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Select Company Gateway provider" }),
    ).toBeInTheDocument();
    expect(getModelProviderProfiles).toHaveBeenCalledWith("gpt-image-2");
  });

  it("ignores stale async provider profiles from a previous model selection", async () => {
    const gptProfiles = createDeferred<typeof openAiProviderState>();
    const geminiProfiles = createDeferred<typeof geminiProviderState>();

    getModelProviderProfiles.mockImplementation((model: string) =>
      model === "gpt-image-2" ? gptProfiles.promise : geminiProfiles.promise,
    );

    renderSettingsPage();

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    await clickModelCard("Nano Banana");

    await act(async () => {
      geminiProfiles.resolve(geminiProviderState);
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
      gptProfiles.resolve(openAiProviderState);
    });

    expect(screen.getByDisplayValue("gemini-key")).toBeInTheDocument();
    expect(screen.queryByText("OpenAI Official")).not.toBeInTheDocument();
    expect(
      screen.getByDisplayValue(
        "https://generativelanguage.googleapis.com/v1beta/models",
      ),
    ).toBeInTheDocument();
  });

  it("resets provider fields to the selected model defaults while loading", async () => {
    const geminiProfiles = createDeferred<typeof geminiProviderState>();

    getModelProviderProfiles.mockImplementation((model: string) =>
      model === "nano-banana" ? geminiProfiles.promise : Promise.resolve(openAiProviderState),
    );

    renderSettingsPage();

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    await screen.findByDisplayValue("https://api.openai.com/v1");
    fireEvent.click(screen.getByRole("button", { name: "settings.showKey" }));
    expect(screen.getByDisplayValue("openai-key")).toBeInTheDocument();

    await clickModelCard("Nano Banana");

    expect(screen.getByDisplayValue("Default")).toBeInTheDocument();
    expect(
      screen.getByDisplayValue(
        "https://generativelanguage.googleapis.com/v1beta/models",
      ),
    ).toBeInTheDocument();

    await act(async () => {
      geminiProfiles.resolve(geminiProviderState);
    });

    expect(await screen.findByDisplayValue("Gemini Official")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "settings.showKey" }));
    expect(screen.getByDisplayValue("gemini-key")).toBeInTheDocument();
  });

  it("saves edits to the selected provider profile", async () => {
    renderSettingsPage();

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    await screen.findByRole("button", { name: "Select OpenAI Official provider" });
    fireEvent.click(screen.getByRole("button", { name: "Select Company Gateway provider" }));
    fireEvent.change(screen.getByDisplayValue("Company Gateway"), {
      target: { value: "Renamed Gateway" },
    });
    fireEvent.click(screen.getByRole("button", { name: "settings.saveProvider" }));

    await waitFor(() => {
      expect(saveModelProviderProfiles).toHaveBeenCalledWith(
        "gpt-image-2",
        expect.objectContaining({
          active_provider_id: "openai-official",
          profiles: expect.arrayContaining([
            expect.objectContaining({ id: "company-gateway", name: "Renamed Gateway" }),
          ]),
        }),
      );
    });
  });

  it("creates a provider for editing without activating it, then activates and deletes providers", async () => {
    renderSettingsPage();

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    await screen.findByRole("button", { name: "Select OpenAI Official provider" });
    fireEvent.click(screen.getByRole("button", { name: "settings.newProvider" }));
    await waitFor(() => {
      expect(createModelProviderProfile).toHaveBeenCalledWith("gpt-image-2", "New Provider");
      expect(screen.getByDisplayValue("New Provider")).toBeInTheDocument();
    });
    expect(screen.getByRole("button", { name: "Select OpenAI Official provider" })).toHaveTextContent(
      "settings.activeProvider",
    );
    expect(screen.getByRole("button", { name: "Select New Provider provider" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("button", { name: "settings.activateProvider" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Select Company Gateway provider" }));
    fireEvent.click(screen.getByRole("button", { name: "settings.activateProvider" }));
    await waitFor(() =>
      expect(setActiveModelProvider).toHaveBeenCalledWith("gpt-image-2", "company-gateway"),
    );

    fireEvent.click(screen.getByRole("button", { name: "Delete Company Gateway provider" }));
    await waitFor(() =>
      expect(deleteModelProviderProfile).toHaveBeenCalledWith("gpt-image-2", "company-gateway"),
    );
  });

  it("does not let a late persisted model overwrite a manual selection", async () => {
    const persistedModel = createDeferred<string>();

    getImageModel.mockReturnValue(persistedModel.promise);

    renderSettingsPage();

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    await clickModelCard("Nano Banana");

    await waitFor(() => {
      expect(getModelProviderProfiles).toHaveBeenCalledWith("nano-banana");
    });

    await act(async () => {
      persistedModel.resolve("nano-banana-pro");
    });

    expect(screen.getByRole("button", { name: "Select Nano Banana model" })).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.getByRole("button", { name: "Select Nano Banana Pro model" }),
    ).toHaveAttribute("aria-pressed", "false");
    expect(
      screen.getByDisplayValue(
        "https://generativelanguage.googleapis.com/v1beta/models",
      ),
    ).toBeInTheDocument();
  });

  it("ignores late provider save completions after switching to another model", async () => {
    const providerSave = createDeferred<typeof openAiProviderState>();

    saveModelProviderProfiles.mockReturnValue(providerSave.promise);

    renderSettingsPage();

    fireEvent.click(screen.getByRole("button", { name: "settings.modelConfig" }));

    await screen.findByRole("button", { name: "Select OpenAI Official provider" });
    fireEvent.click(screen.getByRole("button", { name: "settings.showKey" }));
    fireEvent.change(screen.getByDisplayValue("openai-key"), {
      target: { value: "pending-openai-key" },
    });
    fireEvent.click(screen.getByRole("button", { name: "settings.saveProvider" }));

    await clickModelCard("Nano Banana");

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Select Nano Banana model" })).toHaveAttribute("aria-pressed", "true");
      expect(
        screen.getByDisplayValue(
          "https://generativelanguage.googleapis.com/v1beta/models",
        ),
      ).toBeInTheDocument();
    });

    await act(async () => {
      providerSave.resolve(openAiProviderState);
    });

    expect(screen.queryAllByText("settings.saved")).toHaveLength(0);
    expect(
      screen.getByDisplayValue(
        "https://generativelanguage.googleapis.com/v1beta/models",
      ),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "settings.showKey" }));
    expect(screen.getByDisplayValue("gemini-key")).toBeInTheDocument();
    expect(
      screen.queryByDisplayValue("pending-openai-key"),
    ).not.toBeInTheDocument();
  });
});
