import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import MessageBubble from "./MessageBubble";
import type { Message } from "../../types";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) =>
      ({
        "generate.generationFailed": "Generation failed",
        "generate.retry": "Retry",
        "generate.editPrompt": "Edit prompt",
        "generate.agent.finalPrompt": "Final prompt",
        "generate.agent.acceptAndGenerate": "Accept and generate",
        "generate.agent.continueRefining": "Continue refining",
        "generate.agent.editManually": "Edit manually",
      })[key] ?? key,
  }),
}));

vi.mock("../../lib/api", () => ({
  toAssetUrl: (path: string) => path,
}));

vi.mock("./ImageGrid", () => ({
  default: () => <div data-testid="image-grid" />,
}));

vi.mock("./GenerationLoadingScene", () => ({
  default: () => <div data-testid="loading-scene" />,
}));

describe("MessageBubble", () => {
  it("expands and collapses a long prompt without losing its content", () => {
    const content = "Detailed lighting and composition. ".repeat(20);
    render(
      <MessageBubble
        message={{
          id: "long",
          role: "user",
          content,
          status: "complete",
          createdAt: "2026-05-01",
        }}
      />,
    );
    const toggle = screen.getByRole("button", { name: "studio.expandPrompt" });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(toggle);
    expect(
      screen.getByRole("button", { name: "studio.collapsePrompt" }),
    ).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText(content.trim())).not.toHaveClass("line-clamp-3");
  });

  it("limits source image max height to 80% of the chat viewport height", () => {
    const message: Message = {
      id: "user-2",
      role: "user",
      content: "Use this as a source image",
      sourceImages: [
        {
          imageId: "image-1",
          generationId: "generation-1",
          path: "/tmp/source-image.png",
          thumbnailPath: "/tmp/source-image-thumb.png",
        },
      ],
      status: "complete",
      createdAt: "2026-04-26T00:00:00Z",
    };

    const { container } = render(
      <MessageBubble
        message={message}
        onImageClick={vi.fn()}
        chatViewportHeight={500}
      />,
    );

    expect(container.querySelector("img")).toHaveStyle({
      maxHeight: "400px",
    });
  });

  it("shows an edit prompt button for user messages", () => {
    const onEditPrompt = vi.fn();
    const message: Message = {
      id: "user-1",
      role: "user",
      content: "A cinematic portrait of a fox astronaut",
      status: "complete",
      createdAt: "2026-04-26T00:00:00Z",
    };

    render(
      <MessageBubble
        message={message}
        onImageClick={vi.fn()}
        onEditPrompt={onEditPrompt}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Edit prompt" }));

    expect(onEditPrompt).toHaveBeenCalledWith(message);
    expect(
      screen
        .getByText("A cinematic portrait of a fox astronaut")
        .closest("[data-message-role='user']"),
    ).toHaveClass("justify-start");
    expect(
      screen
        .getByText("A cinematic portrait of a fox astronaut")
        .closest("div"),
    ).toHaveClass("bg-transparent");
  });

  it("shows a retry button for failed messages with retry data", () => {
    const onRetry = vi.fn();
    const message: Message = {
      id: "assistant-1",
      role: "assistant",
      content: "",
      generationId: "generation-1",
      status: "failed",
      error: "Network error",
      createdAt: "2026-04-26T00:00:00Z",
      retryRequest: {
        prompt: "retry me",
        model: "gpt-image-2",
        size: "auto",
        quality: "auto",
        background: "auto",
        outputFormat: "png",
        moderation: "auto",
        inputFidelity: "high",
        imageCount: 1,
        conversationId: "conversation-1",
        editSources: [],
      },
    };

    render(
      <MessageBubble
        message={message}
        onImageClick={vi.fn()}
        onRetry={onRetry}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Retry" }));

    expect(onRetry).toHaveBeenCalledWith(message);
    expect(screen.getByText("Network error").parentElement).toHaveClass(
      "rounded-xl",
      "border-error/20",
    );
  });

  it("leaves individual image rounding to the image grid", () => {
    const message: Message = {
      id: "assistant-2",
      role: "assistant",
      content: "",
      generationId: "generation-2",
      status: "complete",
      createdAt: "2026-04-26T00:00:00Z",
      images: [
        {
          imageId: "image-2",
          generationId: "generation-2",
          path: "/tmp/generated-image.png",
          thumbnailPath: "/tmp/generated-thumb.png",
        },
      ],
    };

    render(<MessageBubble message={message} onImageClick={vi.fn()} />);

    expect(
      screen
        .getByTestId("image-grid")
        .closest("[data-message-role='assistant']"),
    ).toHaveClass("justify-start");
    expect(screen.getByTestId("image-grid").parentElement).toHaveClass(
      "bg-canvas",
    );
    expect(screen.getByTestId("image-grid").parentElement).not.toHaveClass(
      "overflow-hidden",
    );
  });

  it("renders accept controls for ready prompt agent drafts", () => {
    const onAccept = vi.fn();

    render(
      <MessageBubble
        agentMessage={{
          id: "agent-msg-1",
          session_id: "agent-session-1",
          role: "assistant",
          content: "I have a final prompt ready.",
          draft_prompt: "A cinematic glass observatory over a moonlit forest.",
          selected_skill_ids: ["photography"],
          suggested_params: {},
          ready_to_generate: true,
          created_at: "2026-05-16T00:00:00Z",
        }}
        onAcceptAgentDraft={onAccept}
      />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "Accept and generate" }),
    );

    expect(onAccept).toHaveBeenCalled();
    expect(
      screen.getByText("A cinematic glass observatory over a moonlit forest."),
    ).toBeInTheDocument();
  });
});

it("offers full parameter reuse and regeneration on completed history", () => {
  const reuse = vi.fn();
  const message: Message = {
    id: "history",
    role: "assistant",
    content: "",
    generationId: "saved",
    status: "complete",
    createdAt: "2026-11-13",
  };
  render(<MessageBubble message={message} onReuseRecipe={reuse} />);
  fireEvent.click(screen.getByRole("button", { name: "recipe.useParameters" }));
  expect(reuse).toHaveBeenCalledWith("saved", "edit");
  fireEvent.click(screen.getByRole("button", { name: "recipe.regenerate" }));
  expect(reuse).toHaveBeenCalledWith("saved", "generate");
});

it("shows persisted visual observations in an optional user-message disclosure", () => {
 render(<MessageBubble agentMessage={{id:"visual",session_id:"s",role:"user",content:"Keep the composition",selected_skill_ids:[],suggested_params:{},ready_to_generate:false,created_at:"2026-11-13",visual_context:{version:1,analysis_model:"vision-model",analysis_protocol:"openai",image_hashes:["private-hash"],observations:["A red vase on a stone table"],preserve:["Vase placement"],changes:["Warm afternoon light"],user_request:"Keep the composition",text_only:false}}}/>);
 const disclosure=screen.getByText("promptVision.summary").closest("details");
 expect(disclosure).not.toHaveAttribute("open");
 fireEvent.click(screen.getByText("promptVision.summary"));
 expect(screen.getByText("A red vase on a stone table")).toBeInTheDocument();
 expect(screen.getByText("Vase placement")).toBeInTheDocument();
 expect(screen.queryByText("private-hash")).not.toBeInTheDocument();
});
