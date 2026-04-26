import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import GenerationLoadingScene from "./GenerationLoadingScene";

const translations: Record<string, string> = {
  "generate.loading.title": "画面正在穿过雾光",
  "generate.loading.subtitle": "星云正在校准层次，最终镜头正在靠近。",
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
    expect(screen.getByText("画面正在穿过雾光", { selector: ".generation-loading-title" })).toBeTruthy();
    expect(screen.getByText("星云正在校准层次，最终镜头正在靠近。")).toBeTruthy();
  });
});
