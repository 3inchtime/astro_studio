import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { LlmConfigSection } from "./LlmConfigSection";
import type { LlmConfig } from "../../types";

const apiMocks = vi.hoisted(() => ({
  getLlmConfigs: vi.fn(),
  saveLlmConfigs: vi.fn(),
}));

vi.mock("../../lib/api", () => ({
  getLlmConfigs: apiMocks.getLlmConfigs,
  saveLlmConfigs: apiMocks.saveLlmConfigs,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

function renderSection(configs: LlmConfig[] = []) {
  apiMocks.getLlmConfigs.mockResolvedValue(configs);
  apiMocks.saveLlmConfigs.mockResolvedValue(undefined);

  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });

  render(
    <QueryClientProvider client={queryClient}>
      <LlmConfigSection />
    </QueryClientProvider>,
  );
}

describe("LlmConfigSection", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it("shows no saved secret and sends a distinct clear action", async () => {
    renderSection([{ id: "vault-llm", name: "Vault helper", protocol: "openai", model: "gpt-4o", api_key: "", api_key_configured: true, api_key_masked: "••••••••", base_url: "https://example.test", capability: "text", enabled: false }]);
    const input = await screen.findByPlaceholderText("credentials.keepPlaceholder");
    expect(input).toHaveValue("");
    fireEvent.click(screen.getByRole("button", { name: "credentials.clear" }));
    expect(screen.getByRole("button", { name: "credentials.clearPending" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "settings.llm.saveConfig" }));
    await waitFor(() => expect(apiMocks.saveLlmConfigs).toHaveBeenCalledWith([expect.objectContaining({ api_key: "", credential_update: { action: "clear" } })]));
  });

  it("creates disabled configs by default", async () => {
    renderSection();

    fireEvent.click(
      screen.getByRole("button", { name: "settings.addOptimizationService" }),
    );

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "settings.llm.create" }),
      ).toBeInTheDocument();
    });
  });

  it("preserves failed edits and allows saving them again", async () => {
    renderSection([
      {
        id: "llm-a",
        name: "Prompt Helper",
        protocol: "openai",
        model: "gpt-4o",
        api_key: "sk-llm",
        base_url: "https://api.openai.com/v1",
        capability: "text",
        enabled: false,
      },
    ]);
    await screen.findByDisplayValue("Prompt Helper");
    apiMocks.saveLlmConfigs.mockRejectedValueOnce(new Error("Offline"));
    fireEvent.change(screen.getByDisplayValue("Prompt Helper"), {
      target: { value: "Retained edits" },
    });
    fireEvent.click(
      screen.getByRole("button", { name: "settings.llm.saveConfig" }),
    );
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "settings.llm.saveError",
    );
    expect(screen.getByDisplayValue("Retained edits")).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", { name: "settings.llm.saveConfig" }),
    );
    await waitFor(() =>
      expect(apiMocks.saveLlmConfigs).toHaveBeenLastCalledWith([
        expect.objectContaining({ name: "Retained edits" }),
      ]),
    );
  });

  it.each([true, false])(
    "retains edits when toggling enabled=%s fails",
    async (enabled) => {
      renderSection([
        {
          id: "llm-a",
          name: "Prompt Helper",
          protocol: "openai",
          model: "gpt-4o",
          api_key: "sk-llm",
          base_url: "https://api.openai.com/v1",
          capability: "text",
          enabled,
        },
      ]);
      await screen.findByDisplayValue("Prompt Helper");
      fireEvent.change(screen.getByDisplayValue("Prompt Helper"), {
        target: { value: "Unsaved name" },
      });
      apiMocks.saveLlmConfigs.mockRejectedValueOnce(new Error("Offline"));
      fireEvent.click(
        screen.getByRole("button", {
          name: enabled ? "settings.llm.deactivate" : "settings.llm.activate",
        }),
      );
      expect(await screen.findByRole("alert")).toHaveTextContent(
        "settings.llm.saveError",
      );
      expect(screen.getByDisplayValue("Unsaved name")).toBeInTheDocument();
      expect(
        screen.getByRole("button", {
          name: enabled ? "settings.llm.deactivate" : "settings.llm.activate",
        }),
      ).toBeEnabled();
    },
  );

  it("blocks switching configurations while saving and retains the failed draft", async () => {
    renderSection(
      ["A", "B"].map((name) => ({
        id: name,
        name: `Helper ${name}`,
        protocol: "openai",
        model: "gpt-4o",
        api_key: "key",
        base_url: "https://api.openai.com/v1",
        capability: "text",
        enabled: false,
      })),
    );
    await screen.findByDisplayValue("Helper A");
    let rejectSave!: (error: Error) => void;
    apiMocks.saveLlmConfigs.mockReturnValueOnce(
      new Promise<void>((_, reject) => {
        rejectSave = reject;
      }),
    );
    fireEvent.change(screen.getByDisplayValue("Helper A"), {
      target: { value: "Retained pending draft" },
    });
    fireEvent.click(
      screen.getByRole("button", { name: "settings.llm.saveConfig" }),
    );
    await screen.findByRole("status");
    const otherConfig = screen.getByRole("button", {
      name: "Select Helper B optimization service",
    });
    fireEvent.click(otherConfig);
    fireEvent.keyDown(otherConfig, { key: "Enter" });
    fireEvent.keyDown(otherConfig, { key: " " });
    expect(
      screen.getByDisplayValue("Retained pending draft"),
    ).toBeInTheDocument();
    expect(otherConfig).toBeDisabled();
    await act(async () => rejectSave(new Error("Offline")));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "settings.llm.saveError",
    );
    expect(
      screen.getByDisplayValue("Retained pending draft"),
    ).toBeInTheDocument();
    expect(otherConfig).toBeEnabled();
  });

  it("shows the delete button clearly", async () => {
    renderSection([
      {
        id: "llm-a",
        name: "Prompt Helper",
        protocol: "openai",
        model: "gpt-4o",
        api_key: "sk-llm",
        base_url: "https://api.openai.com/v1",
        capability: "text",
        enabled: false,
      },
    ]);

    await waitFor(() => {
      expect(
        screen.getByRole("button", {
          name: "Select Prompt Helper optimization service",
        }),
      ).toBeInTheDocument();
    });

    expect(
      screen.getByRole("button", { name: "settings.llm.deleteTitle" }),
    ).toBeInTheDocument();
  });

  it("deletes an llm config", async () => {
    renderSection([
      {
        id: "llm-a",
        name: "Prompt Helper",
        protocol: "openai",
        model: "gpt-4o",
        api_key: "sk-llm",
        base_url: "https://api.openai.com/v1",
        capability: "text",
        enabled: false,
      },
    ]);

    await waitFor(() => {
      expect(
        screen.getByRole("button", {
          name: "Select Prompt Helper optimization service",
        }),
      ).toBeInTheDocument();
    });

    fireEvent.click(
      screen.getByRole("button", { name: "settings.llm.deleteTitle" }),
    );
    fireEvent.click(
      screen.getAllByRole("button", { name: "settings.llm.deleteTitle" })[1],
    );

    await waitFor(() => {
      expect(apiMocks.saveLlmConfigs).toHaveBeenCalledWith([]);
    });
  });

  it("can disable an enabled llm config", async () => {
    renderSection([
      {
        id: "llm-a",
        name: "Prompt Helper",
        protocol: "openai",
        model: "gpt-4o",
        api_key: "sk-llm",
        base_url: "https://api.openai.com/v1",
        capability: "text",
        enabled: true,
      },
    ]);

    await waitFor(() => {
      expect(
        screen.getByRole("button", {
          name: "Select Prompt Helper optimization service",
        }),
      ).toBeInTheDocument();
    });

    fireEvent.click(
      screen.getByRole("button", { name: "settings.llm.deactivate" }),
    );

    await waitFor(() => {
      expect(apiMocks.saveLlmConfigs).toHaveBeenCalledWith([
        expect.objectContaining({
          id: "llm-a",
          enabled: false,
        }),
      ]);
    });
  });

  it("blocks enabling a second text config", async () => {
    renderSection([
      {
        id: "text-a",
        name: "Text A",
        protocol: "openai",
        model: "gpt-4o",
        api_key: "sk-a",
        base_url: "https://api.openai.com/v1",
        capability: "text",
        enabled: true,
      },
      {
        id: "text-b",
        name: "Text B",
        protocol: "openai",
        model: "gpt-4o-mini",
        api_key: "sk-b",
        base_url: "https://api.openai.com/v1",
        capability: "text",
        enabled: false,
      },
    ]);

    await waitFor(() => {
      expect(
        screen.getByRole("button", {
          name: "Select Text A optimization service",
        }),
      ).toBeInTheDocument();
    });

    fireEvent.click(
      screen.getByRole("button", {
        name: "Select Text B optimization service",
      }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "settings.llm.activate" }),
    );

    await waitFor(() => {
      expect(
        screen.getByText("settings.llm.enableTextLimit"),
      ).toBeInTheDocument();
    });
    expect(apiMocks.saveLlmConfigs).not.toHaveBeenCalled();
  });

  it("blocks enabling a multimodal config when the total enabled limit is reached", async () => {
    renderSection([
      {
        id: "text-a",
        name: "Text A",
        protocol: "openai",
        model: "gpt-4o",
        api_key: "sk-a",
        base_url: "https://api.openai.com/v1",
        capability: "text",
        enabled: true,
      },
      {
        id: "multi-a",
        name: "Vision A",
        protocol: "openai",
        model: "gpt-4.1",
        api_key: "sk-b",
        base_url: "https://api.openai.com/v1",
        capability: "multimodal",
        enabled: true,
      },
      {
        id: "multi-b",
        name: "Vision B",
        protocol: "openai",
        model: "gpt-4.1-mini",
        api_key: "sk-c",
        base_url: "https://api.openai.com/v1",
        capability: "multimodal",
        enabled: false,
      },
    ]);

    await waitFor(() => {
      expect(
        screen.getByRole("button", {
          name: "Select Vision B optimization service",
        }),
      ).toBeInTheDocument();
    });

    fireEvent.click(
      screen.getByRole("button", {
        name: "Select Vision B optimization service",
      }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "settings.llm.activate" }),
    );

    await waitFor(() => {
      expect(
        screen.getByText("settings.llm.enableCombinationLimit"),
      ).toBeInTheDocument();
    });
    expect(apiMocks.saveLlmConfigs).not.toHaveBeenCalled();
  });
});
