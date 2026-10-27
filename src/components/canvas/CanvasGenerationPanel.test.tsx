import { render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import CanvasGenerationPanel from "./CanvasGenerationPanel";
vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
it("shows free resized frames as a localized custom preset", () => {
  render(
    <CanvasGenerationPanel
      prompt=""
      imageModel="gpt-image-2"
      frame={{ x: 0, y: 0, width: 300, height: 200, aspect: "free" }}
      disabled={false}
      isGenerating={false}
      onPromptChange={vi.fn()}
      onGenerate={vi.fn()}
      onFrameAspectChange={vi.fn()}
    />,
  );
  expect(
    screen.getByRole("combobox", { name: "canvas.framePreset" }),
  ).toHaveValue("custom");
  expect(
    screen.getByRole("option", { name: "canvas.customAspect" }),
  ).toHaveValue("custom");
});
