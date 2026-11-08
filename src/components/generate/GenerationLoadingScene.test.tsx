import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import GenerationLoadingScene from "./GenerationLoadingScene";

const translations: Record<string, string> = {
  "studio.generationWaiting": "Waiting for image provider",
  "studio.generationWaitingHint":
    "Your request is processing. Images will appear when ready.",
};

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => translations[key] ?? key,
  }),
}));

describe("GenerationLoadingScene", () => {
  it("renders localized loading copy and status role", () => {
    render(<GenerationLoadingScene />);

    expect(screen.getByRole("status")).toBeTruthy();
    expect(screen.getByText("Waiting for image provider")).toBeTruthy();
    expect(
      screen.getByText(
        "Your request is processing. Images will appear when ready.",
      ),
    ).toBeTruthy();
    expect(document.querySelector(".generation-loading-progress")).toBeNull();
  });
});
