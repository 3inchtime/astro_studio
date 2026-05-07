import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ModelSettingsPanel } from "./ModelSettingsPanel";
import type { ModelProviderProfilesState } from "../../types";

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

function renderPanel(overrides = {}) {
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
    ...overrides,
  };

  render(<ModelSettingsPanel {...props} />);
  return props;
}

describe("ModelSettingsPanel provider profiles", () => {
  it("renders provider rows and the selected provider editor", () => {
    renderPanel();

    expect(screen.getByText("settings.providers")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Select OpenAI Official provider" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Select Company Gateway provider" })).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByDisplayValue("OpenAI Official")).toBeInTheDocument();
    expect(screen.getByDisplayValue("https://api.openai.com/v1")).toBeInTheDocument();
  });

  it("routes provider actions through callbacks", () => {
    const props = renderPanel({ selectedProviderId: "provider-b" });

    fireEvent.click(screen.getByRole("button", { name: "settings.newProvider" }));
    fireEvent.click(screen.getByRole("button", { name: "Use Company Gateway provider" }));
    fireEvent.click(screen.getByRole("button", { name: "Delete Company Gateway provider" }));
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

  it("disables provider deletion when only one provider remains", () => {
    renderPanel({
      providerState: {
        active_provider_id: "provider-a",
        profiles: [providerState.profiles[0]],
      },
    });

    expect(
      screen.getByRole("button", { name: "Delete OpenAI Official provider" }),
    ).toBeDisabled();
  });
});
