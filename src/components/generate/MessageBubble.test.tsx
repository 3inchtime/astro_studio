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
      screen.getByText("A cinematic portrait of a fox astronaut").closest("div"),
    ).toHaveClass("rounded-[16px]", "rounded-br-[5px]");
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
      "rounded-[16px]",
      "rounded-bl-[5px]",
    );
  });

  it("uses a less-rounded container for assistant image bubbles", () => {
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

    expect(screen.getByTestId("image-grid").parentElement).toHaveClass(
      "rounded-[14px]",
    );
  });
});
