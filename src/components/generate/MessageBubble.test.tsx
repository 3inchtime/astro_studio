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
        size: "auto",
        quality: "auto",
        outputFormat: "png",
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
  });
});
