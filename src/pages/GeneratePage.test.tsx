import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import GeneratePage from "./GeneratePage";

const getConversationGenerations = vi.fn();
const getImageModel = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number }) =>
      ({
        "generate.placeholder": "Describe the image you want to generate...",
        "generate.editPlaceholder":
          "Describe how you want to edit these source images...",
        "generate.modelLabel": "Model",
        "generate.sizeLabel": "Size",
        "generate.qualityLabel": "Quality",
        "generate.countLabel": "Count",
        "generate.formatLabel": "Format",
        "generate.uploadSource": "Upload Source",
        "generate.clearSources": "Clear sources",
        "generate.editingPrompt": "Editing previous prompt",
        "generate.cancelEditPrompt": "Cancel edit",
        "generate.editingSources":
          options?.count === 1
            ? "Editing 1 source image"
            : `Editing ${options?.count} source images`,
        "generate.editPrompt": "Edit prompt",
        "generate.auto": "Auto",
        "generate.square": "Square",
        "generate.landscape": "Landscape",
        "generate.portrait": "Portrait",
        "generate.quality.auto": "Auto",
        "generate.quality.high": "High",
        "generate.quality.medium": "Medium",
        "generate.quality.low": "Low",
        "generate.countValue": `${options?.count} images`,
        "generate.format.png": "PNG",
        "generate.format.jpeg": "JPEG",
        "generate.format.webp": "WEBP",
      })[key] ?? key,
  }),
}));

vi.mock("../lib/api", () => ({
  deleteGeneration: vi.fn(),
  editImage: vi.fn(),
  generateImage: vi.fn(),
  getConversationGenerations: (...args: unknown[]) =>
    getConversationGenerations(...args),
  getImageModel: (...args: unknown[]) => getImageModel(...args),
  messageImageToEditSource: (image: {
    path: string;
    imageId?: string;
    generationId?: string;
  }) => ({
    id: `${image.imageId ?? image.generationId ?? "source"}:${image.path}`,
    path: image.path,
    label: image.path.split("/").pop() ?? "source-image",
    imageId: image.imageId,
    generationId: image.generationId,
  }),
  pickSourceImages: vi.fn(),
  toAssetUrl: (path: string) => path,
}));

vi.mock("../lib/editSources", () => ({
  consumePendingEditSources: () => [],
}));

vi.mock("../components/layout/AppLayout", () => ({
  useLayoutContext: () => ({
    activeConversationId: "conversation-1",
    setActiveConversationId: vi.fn(),
    refreshConversations: vi.fn(),
  }),
}));

vi.mock("../components/common/ConfirmDialog", () => ({
  default: () => null,
}));

vi.mock("../components/generate/MessageBubble", () => ({
  default: ({
    message,
    onEditPrompt,
  }: {
    message: { id: string; role: string; content: string };
    onEditPrompt?: (message: { id: string; role: string; content: string }) => void;
  }) =>
    message.role === "user" ? (
      <button onClick={() => onEditPrompt?.(message)}>Edit prompt</button>
    ) : null,
}));

vi.mock("../components/lightbox/Lightbox", () => ({
  default: () => null,
}));

vi.mock("../components/favorites/FolderSelector", () => ({
  default: () => null,
}));

describe("GeneratePage", () => {
  beforeEach(() => {
    getConversationGenerations.mockReset();
    getImageModel.mockReset();

    getConversationGenerations.mockResolvedValue([
      {
        generation: {
          id: "generation-1",
          prompt: "A dramatic mountain temple at sunrise",
          created_at: "2026-04-26T00:00:00Z",
          status: "completed",
        },
        images: [
          {
            id: "image-1",
            generation_id: "generation-1",
            file_path: "/tmp/source.png",
            thumbnail_path: "/tmp/source-thumb.png",
          },
        ],
      },
    ]);
    getImageModel.mockResolvedValue("gpt-image-2");
  });

  it("loads a sent prompt back into the composer when editing", async () => {
    render(<GeneratePage />);

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "Edit prompt" }));

    expect(screen.getByDisplayValue("A dramatic mountain temple at sunrise")).toBeInTheDocument();
    expect(screen.getByText("Editing previous prompt")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel edit" })).toBeInTheDocument();
  });
});
