import { createRef, type ComponentProps } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import GenerationComposer from "./GenerationComposer";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) =>
      options?.defaultValue ?? key,
  }),
}));
vi.mock("../../lib/api", () => ({ toAssetUrl: (path: string) => path }));
vi.mock("../../lib/queries/llm", () => ({
  useLlmConfigsQuery: () => ({ data: [] }),
  useOptimizePromptMutation: () => ({ isPending: false }),
}));

function props(
  overrides: Partial<ComponentProps<typeof GenerationComposer>> = {},
): ComponentProps<typeof GenerationComposer> {
  return {
    textareaRef: createRef(),
    prompt: "A quiet mountain lake",
    imageModel: "gpt-image-2",
    quality: "auto",
    background: "auto",
    outputFormat: "png",
    moderation: "auto",
    inputFidelity: "high",
    imageCount: 1,
    editSources: [],
    editingPromptMessageId: null,
    composerMode: "direct",
    onPromptChange: vi.fn(),
    onModelChange: vi.fn(),
    onQualityChange: vi.fn(),
    onBackgroundChange: vi.fn(),
    onOutputFormatChange: vi.fn(),
    onModerationChange: vi.fn(),
    onInputFidelityChange: vi.fn(),
    onImageCountChange: vi.fn(),
    onAddUploadedSources: vi.fn(),
    onClearEditSources: vi.fn(),
    onRemoveEditSource: vi.fn(),
    onCancelPromptEdit: vi.fn(),
    onGenerate: vi.fn(),
    onComposerModeChange: vi.fn(),
    onDeepThinkingSend: vi.fn(),
    ...overrides,
  };
}

describe("GenerationComposer workbench", () => {
  it("submits with Cmd/Ctrl+Enter while preserving ordinary Enter and IME composition", () => {
    const onGenerate = vi.fn();
    render(<GenerationComposer {...props({ onGenerate })} />);
    const input = screen.getByRole("textbox");
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.keyDown(input, {
      key: "Enter",
      metaKey: true,
      isComposing: true,
    });
    fireEvent.keyDown(input, { key: "Enter", ctrlKey: true, keyCode: 229 });
    expect(onGenerate).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: "Enter", metaKey: true });
    fireEvent.keyDown(input, { key: "Enter", ctrlKey: true });
    expect(onGenerate).toHaveBeenCalledTimes(2);
  });

  it("uses the same disabled and deep-thinking rules for keyboard submission", () => {
    const onDeepThinkingSend = vi.fn();
    const config = props({ composerMode: "deep-thinking", onDeepThinkingSend });
    const { rerender } = render(<GenerationComposer {...config} />);
    fireEvent.keyDown(screen.getByRole("textbox"), {
      key: "Enter",
      ctrlKey: true,
    });
    expect(onDeepThinkingSend).toHaveBeenCalledTimes(1);
    rerender(<GenerationComposer {...config} isGenerating />);
    fireEvent.keyDown(screen.getByRole("textbox"), {
      key: "Enter",
      ctrlKey: true,
    });
    expect(onDeepThinkingSend).toHaveBeenCalledTimes(1);
  });

  it("identifies the selected model provider accurately", () => {
    const { rerender } = render(
      <GenerationComposer
        {...props({ imageModel: "grok-imagine-image-2.0" })}
      />,
    );
    expect(screen.getByText("xAI")).toBeInTheDocument();
    rerender(
      <GenerationComposer
        {...props({ imageModel: "doubao-seedream-5-0-pro-260628" })}
      />,
    );
    expect(screen.getByText("Volcengine")).toBeInTheDocument();
  });

  it("keeps common controls visible and secondary controls in an advanced disclosure", () => {
    render(<GenerationComposer {...props()} />);
    expect(
      screen.getByLabelText("generate.modelLabel").closest("details"),
    ).toBeNull();
    expect(
      screen.getByLabelText("generate.countLabel").closest("details"),
    ).toBeNull();
    const details = screen
      .getByLabelText("generate.qualityLabel")
      .closest("details");
    expect(details).not.toHaveAttribute("open");
    expect(details?.querySelector("summary")).toHaveTextContent(
      "Advanced settings",
    );
    expect(screen.getByRole("textbox")).not.toHaveClass("pr-[190px]");
    expect(
      screen.getByRole("button", { name: "generate.submit" }).parentElement,
    ).not.toHaveClass("absolute");
  });
  it("keeps parameter disclosures mounted outside the prompt surface", () => {
    const target = document.createElement("div");
    document.body.append(target);
    const config = props({ parametersTarget: target });
    const { rerender, unmount } = render(<GenerationComposer {...config} />);
    const model = screen.getByLabelText("generate.modelLabel");
    expect(target).toContainElement(model);
    const details = screen.getByText("Advanced settings").closest("details")!;
    details.open = true;
    rerender(<GenerationComposer {...config} prompt="Changed draft" />);
    expect(screen.getByRole("textbox")).toHaveValue("Changed draft");
    expect(screen.getByText("Advanced settings").closest("details")).toBe(details);
    expect(details.open).toBe(true);
    expect(screen.getByTestId("generation-command-surface")).not.toContainElement(model);
    unmount(); target.remove();
  });

});

it("shows the actual saved image count outside current composer presets", () => {
  const view = render(<GenerationComposer {...props({ imageCount: 7 })} />);
  const select = screen.getByLabelText("generate.countLabel") as HTMLSelectElement;
  expect(select).toHaveValue("7");
  expect(select.selectedOptions[0]).toHaveTextContent("7");
  expect(select.selectedOptions[0]).toBeDisabled();
  view.rerender(<GenerationComposer {...props({ imageCount: 1 })} />);
  expect(select).toHaveValue("1");
  expect(Array.from(select.options).map((option) => option.value)).toEqual(["1", "2", "3", "4"]);
});
