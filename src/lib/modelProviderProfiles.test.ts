import { describe, expect, it } from "vitest";
import {
  DEFAULT_PROVIDER_ID,
  activeProviderForState,
  defaultProviderProfilesStateForModel,
  removeProviderFromState,
  updateProviderInState,
} from "./modelProviderProfiles";

describe("model provider profile helpers", () => {
  it("builds a default provider state from model defaults", () => {
    expect(defaultProviderProfilesStateForModel("gpt-image-2")).toEqual({
      active_provider_id: DEFAULT_PROVIDER_ID,
      profiles: [
        {
          id: DEFAULT_PROVIDER_ID,
          name: "Default",
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
  });

  it("updates one provider without changing the active provider", () => {
    const state = defaultProviderProfilesStateForModel("gpt-image-2");

    expect(
      updateProviderInState(state, DEFAULT_PROVIDER_ID, (profile) => ({
        ...profile,
        name: "OpenAI Official",
      })),
    ).toMatchObject({
      active_provider_id: DEFAULT_PROVIDER_ID,
      profiles: [{ name: "OpenAI Official" }],
    });
  });

  it("removes an active provider and activates the first remaining provider", () => {
    const state = {
      active_provider_id: "provider-b",
      profiles: [
        {
          ...defaultProviderProfilesStateForModel("gpt-image-2").profiles[0],
          id: "provider-a",
          name: "Provider A",
        },
        {
          ...defaultProviderProfilesStateForModel("gpt-image-2").profiles[0],
          id: "provider-b",
          name: "Provider B",
        },
      ],
    };

    expect(removeProviderFromState(state, "provider-b")).toMatchObject({
      active_provider_id: "provider-a",
      profiles: [{ id: "provider-a" }],
    });
  });

  it("returns the active provider or the first provider", () => {
    const state = {
      active_provider_id: "missing",
      profiles: defaultProviderProfilesStateForModel("gpt-image-2").profiles,
    };

    expect(activeProviderForState(state)?.id).toBe(DEFAULT_PROVIDER_ID);
  });
});
