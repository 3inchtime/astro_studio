import { describe, expect, it } from "vitest";
import {
  defaultEndpointSettingsForModel,
  normalizeEndpointSettings,
} from "./settingsEndpoints";

describe("settings endpoint helpers", () => {
  it("returns model-specific defaults for OpenAI and Gemini models", () => {
    expect(defaultEndpointSettingsForModel("gpt-image-2")).toEqual({
      mode: "base_url",
      base_url: "https://api.openai.com/v1",
      generation_url: "https://api.openai.com/v1/images/generations",
      edit_url: "https://api.openai.com/v1/images/edits",
    });

    expect(defaultEndpointSettingsForModel("nano-banana")).toEqual({
      mode: "base_url",
      base_url: "https://generativelanguage.googleapis.com/v1beta/models",
      generation_url:
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent",
      edit_url:
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash-image:generateContent",
    });
  });

  it("trims custom endpoints and fills blank fields from the selected model defaults", () => {
    expect(
      normalizeEndpointSettings("gpt-image-2", {
        mode: "full_url",
        base_url: "  ",
        generation_url: " https://example.com/generate ",
        edit_url: "",
      }),
    ).toEqual({
      mode: "full_url",
      base_url: "https://api.openai.com/v1",
      generation_url: "https://example.com/generate",
      edit_url: "https://api.openai.com/v1/images/edits",
    });
  });

  it("uses the generation URL as edit URL for shared-endpoint models", () => {
    expect(
      normalizeEndpointSettings("nano-banana", {
        mode: "full_url",
        base_url: " https://custom.example/v1 ",
        generation_url: " https://custom.example/generate ",
        edit_url: " https://custom.example/ignored-edit ",
      }),
    ).toEqual({
      mode: "full_url",
      base_url: "https://custom.example/v1",
      generation_url: "https://custom.example/generate",
      edit_url: "https://custom.example/generate",
    });
  });
});
