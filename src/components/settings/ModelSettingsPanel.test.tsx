import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ModelSettingsPanel } from "./ModelSettingsPanel";
import type { LlmConfig, ModelProviderProfilesState } from "../../types";

const apiMocks = vi.hoisted(() => ({
  getLlmConfigs: vi.fn(),
  saveLlmConfigs: vi.fn(),
}));

vi.mock("../../lib/api", () => ({
  getLlmConfigs: apiMocks.getLlmConfigs,
  saveLlmConfigs: apiMocks.saveLlmConfigs,
}));

const baseEndpoint = {
  mode: "base_url" as const,
  base_url: "https://api.openai.com/v1",
  generation_url: "https://api.openai.com/v1/images/generations",
  edit_url: "https://api.openai.com/v1/images/edits",
};

const providerState: ModelProviderProfilesState = {
  active_provider_id: "provider-a",
  profiles: [
    {
      id: "provider-a",
      name: "OpenAI Official",
      api_key: "sk-openai",
      endpoint_settings: baseEndpoint,
    },
    {
      id: "provider-b",
      name: "Company Gateway",
      api_key: "sk-gateway",
      endpoint_settings: {
        ...baseEndpoint,
        mode: "full_url",
        generation_url: "https://gateway.example/generate",
        edit_url: "https://gateway.example/edit",
      },
    },
  ],
};

const disabledLlmConfig: LlmConfig = {
  id: "llm-a",
  name: "Prompt Helper",
  protocol: "openai",
  model: "gpt-4o",
  api_key: "sk-llm",
  base_url: "https://api.openai.com/v1",
  capability: "text",
  enabled: false,
};

const enabledLlmConfig: LlmConfig = {
  ...disabledLlmConfig,
  enabled: true,
};

function renderPanel({
  llmConfigs = [disabledLlmConfig],
  ...overrides
}: { llmConfigs?: LlmConfig[] } & Record<string, unknown> = {}) {
  apiMocks.saveLlmConfigs.mockResolvedValue([]);
  apiMocks.getLlmConfigs.mockResolvedValue(llmConfigs);

  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  const props = {
    t: ((key: string) => key) as never,
    imageModel: "gpt-image-2" as const,
    modelSaved: false,
    providerState,
    selectedProviderId: "provider-a",
    showKey: false,
    providerSaved: false,
    onSelectImageModel: vi.fn(),
    onSaveModel: vi.fn(),
    onSelectProvider: vi.fn(),
    onProviderNameChange: vi.fn(),
    onProviderApiKeyChange: vi.fn(),
    onShowKeyChange: vi.fn(),
    onProviderEndpointModeChange: vi.fn(),
    onProviderBaseUrlChange: vi.fn(),
    onProviderGenerationUrlChange: vi.fn(),
    onProviderEditUrlChange: vi.fn(),
    onCreateProvider: vi.fn(),
    onDeleteProvider: vi.fn(),
    onSetActiveProvider: vi.fn(),
    onSaveProvider: vi.fn(),
    onCancelProviderEdit: vi.fn(),
    ...overrides,
  };

  render(
    <QueryClientProvider client={queryClient}>
      <ModelSettingsPanel {...props} />
    </QueryClientProvider>,
  );
  return props;
}

describe("ModelSettingsPanel provider profiles", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it("renders provider rows and the selected provider editor", () => {
    renderPanel();

    expect(screen.getByText("settings.providers")).toBeInTheDocument();
    expect(screen.getByText("settings.imageGenerationConfig")).toBeInTheDocument();
    expect(screen.getAllByText("settings.providerName").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Select OpenAI Official provider" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Select Company Gateway provider" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.queryByRole("button", { name: "Use Company Gateway provider" })).not.toBeInTheDocument();
    expect(screen.getByDisplayValue("OpenAI Official")).toBeInTheDocument();
    expect(screen.getByDisplayValue("https://api.openai.com/v1")).toBeInTheDocument();
  });

  it("shows image generation and prompt optimization as aligned configuration blocks", async () => {
    renderPanel();

    expect(screen.getByText("settings.imageGenerationConfig")).toBeInTheDocument();
    expect(screen.getByText("settings.promptOptimizationConfig")).toBeInTheDocument();
    expect(screen.queryByText("settings.currentModel")).not.toBeInTheDocument();
    expect(screen.queryByText("settings.providerWorkspace")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "settings.newProvider" })).toBeInTheDocument();
    expect(screen.getByText("settings.optimizationService")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "settings.addOptimizationService" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "settings.cancelEdit" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "settings.saveProvider" })).toHaveClass("bg-primary/10");
    expect(screen.getByDisplayValue("OpenAI Official")).toBeInTheDocument();
    expect(screen.getAllByText("settings.apiKey").length).toBeGreaterThan(0);
    expect(screen.getAllByText("settings.endpoint").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "settings.deleteProvider" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "settings.saveProvider" })).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getAllByText("Prompt Helper").length).toBeGreaterThan(0);
    });
    expect(screen.getByText("settings.promptOptimizationUsageTitle")).toBeInTheDocument();
    expect(screen.getByText("settings.promptOptimizationUsageHint")).toBeInTheDocument();
    expect(screen.getByText("settings.llm.disabled")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "settings.llm.activate" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "settings.llm.saveConfig" })).toHaveClass("bg-accent/10");
    expect(screen.queryByRole("button", { name: "settings.llm.save" })).not.toBeInTheDocument();
  });

  it("activates a prompt optimization service by saving current edits and enabling it", async () => {
    renderPanel();

    await waitFor(() => {
      expect(screen.getAllByText("Prompt Helper").length).toBeGreaterThan(0);
    });

    fireEvent.change(screen.getByDisplayValue("Prompt Helper"), {
      target: { value: "Prompt Helper Active" },
    });

    fireEvent.click(screen.getByRole("button", { name: "settings.llm.activate" }));
    await waitFor(() => {
      expect(apiMocks.saveLlmConfigs).toHaveBeenCalledWith([
        expect.objectContaining({
          id: "llm-a",
          name: "Prompt Helper Active",
          enabled: true,
        }),
      ]);
    });
  });

  it("keeps the activate action visible for enabled prompt optimization services", async () => {
    renderPanel({ llmConfigs: [enabledLlmConfig] });

    await waitFor(() => {
      expect(screen.getAllByText("Prompt Helper").length).toBeGreaterThan(0);
    });

    expect(screen.getByRole("button", { name: "settings.llm.saveConfig" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "settings.llm.deactivate" })).toBeInTheDocument();
    expect(screen.getByText("settings.llm.enabled")).toBeInTheDocument();
  });

  it("routes provider actions through callbacks", () => {
    const props = renderPanel({ selectedProviderId: "provider-b" });

    fireEvent.click(screen.getByRole("button", { name: "settings.newProvider" }));
    fireEvent.click(screen.getByRole("button", { name: "settings.activateProvider" }));
    fireEvent.click(screen.getByRole("button", { name: "settings.deleteProvider" }));
    fireEvent.change(screen.getByDisplayValue("Company Gateway"), {
      target: { value: "Renamed Gateway" },
    });
    fireEvent.click(screen.getByRole("button", { name: "settings.saveProvider" }));

    expect(props.onCreateProvider).toHaveBeenCalled();
    expect(props.onSetActiveProvider).toHaveBeenCalledWith("provider-b");
    expect(props.onDeleteProvider).toHaveBeenCalledWith("provider-b");
    expect(props.onProviderNameChange).toHaveBeenCalledWith("Renamed Gateway");
    expect(props.onSaveProvider).toHaveBeenCalled();
  });

  it("allows deleting the last remaining provider", () => {
    renderPanel({
      providerState: {
        active_provider_id: "provider-a",
        profiles: [providerState.profiles[0]],
      },
    });

    expect(screen.getByRole("button", { name: "settings.deleteProvider" })).toBeEnabled();
  });

  it("shows a plain delete button label for the selected provider", () => {
    renderPanel();

    expect(
      screen.getByRole("button", { name: "settings.deleteProvider" }),
    ).toBeInTheDocument();
  });

  it("keeps prompt optimization configs disabled by default", async () => {
    renderPanel({ llmConfigs: [] });

    await waitFor(() => {
      expect(screen.getByText("settings.llm.emptyTitle")).toBeInTheDocument();
    });
  });
});
