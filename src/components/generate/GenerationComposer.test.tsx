import { createRef, type ComponentProps } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import GenerationComposer from "./GenerationComposer";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { defaultValue?: string }) =>
      options?.defaultValue ?? key,
  }),
}));
vi.mock("../../lib/api", () => ({ toAssetUrl: (path: string) => path }));
const llmState = vi.hoisted(() => ({ optimize: vi.fn(), configs: [] as { id: string; name: string; enabled: boolean; capability: string }[] }));
beforeEach(() => { llmState.configs = []; llmState.optimize.mockReset().mockResolvedValue("Optimized"); });
vi.mock("../../lib/queries/llm", () => ({
  useLlmConfigsQuery: () => ({ data: llmState.configs, isSuccess: true }),
  useOptimizePromptMutation: () => ({ isPending: false, mutateAsync: llmState.optimize }),
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

it("requires an explicit subset when more than three references are attached", () => {
 const sources = Array.from({length:4}, (_,i) => ({id:`ref-${i}`,path:`/ref-${i}.png`,label:`Reference ${i}`}));
 const onChange = vi.fn();
 const base = props({composerMode:"deep-thinking",editSources:sources,agentVisualSourcePaths:[],onAgentVisualSourcePathsChange:onChange});
 const view = render(<GenerationComposer {...base}/>);
 expect(screen.getByRole("button",{name:"generate.submit"})).toBeDisabled();
 const checks = screen.getAllByRole("checkbox");
 expect(checks).toHaveLength(4);
 fireEvent.click(checks[0]);
 expect(onChange).toHaveBeenCalledWith(["/ref-0.png"]);
 view.rerender(<GenerationComposer {...base} agentVisualSourcePaths={sources.slice(0,3).map(s=>s.path)}/>);
 expect(screen.getAllByRole("checkbox")[3]).toBeDisabled();
 expect(screen.getByRole("button",{name:"generate.submit"})).toBeEnabled();
 expect(screen.getByText("promptVision.editReferencesKept")).toBeInTheDocument();
});
it("offers text configurations for explicit text-only deep thinking with image references", () => {
 llmState.configs=[{id:"text-config",name:"Writer",enabled:true,capability:"text"}];
 const changed=vi.fn(); const toggled=vi.fn();
 const base=props({composerMode:"deep-thinking",editSources:[{id:"ref",path:"/ref.png",label:"Reference"}],onPromptAgentConfigChange:changed,onAgentTextOnlyChange:toggled});
 const view=render(<GenerationComposer {...base}/>);
 expect(changed).toHaveBeenLastCalledWith("");
 fireEvent.click(screen.getByRole("switch",{name:"promptVision.textOnly"}));
 expect(toggled).toHaveBeenCalledWith(true);
 view.rerender(<GenerationComposer {...base} agentTextOnly/>);
 expect(changed).toHaveBeenLastCalledWith("text-config");
 expect(screen.queryAllByRole("checkbox")).toHaveLength(0);
});

it("keeps one recoverable error outside the capped composer so the draft remains usable", () => {
 render(<GenerationComposer {...props({promptAgentError:"Analysis could not finish",onRetryPromptAgent:vi.fn()})}/>);
 expect(screen.getAllByRole("alert")).toHaveLength(1);
 expect(screen.getByRole("alert").closest(".generation-composer")).toBeNull();
 expect(screen.getByTestId("generation-command-surface")).not.toContainElement(screen.getByRole("alert"));
});


it("uses the restored Agent dropdown choice for optimization while thinking", async () => {
  llmState.configs = [{id:"first",name:"First",enabled:true,capability:"text"},{id:"second",name:"Second",enabled:true,capability:"text"}];
  render(<GenerationComposer {...props({composerMode:"deep-thinking",promptAgentConfigId:"second"})}/>);
  expect(screen.getByRole("button", {name:"Second"})).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", {name:"generate.llm.optimize"}));
  await waitFor(()=>expect(llmState.optimize).toHaveBeenCalledWith({prompt:"A quiet mountain lake",configId:"second",imagePaths:undefined}));
});
