import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import GeneratePage from "./GeneratePage";

const getConversationGenerations = vi.fn();
const getImageModel = vi.fn();
const generateImage = vi.fn();
const editImage = vi.fn();
const pickSourceImages = vi.fn();

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
        "generate.backgroundLabel": "Background",
        "generate.moderationLabel": "Moderation",
        "generate.compressionLabel": "Compression",
        "generate.inputFidelityLabel": "Input fidelity",
        "generate.parametersLabel": "Generation parameters",
        "generate.submit": "Generate",
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
        "generate.background.auto": "Auto",
        "generate.background.opaque": "Opaque",
        "generate.background.transparent": "Transparent",
        "generate.moderation.auto": "Auto",
        "generate.moderation.low": "Low",
        "generate.inputFidelity.high": "High",
        "generate.inputFidelity.low": "Low",
      })[key] ?? key,
  }),
}));

vi.mock("../lib/api", () => ({
  deleteGeneration: vi.fn(),
  editImage: (...args: unknown[]) => editImage(...args),
  generateImage: (...args: unknown[]) => generateImage(...args),
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
  pickSourceImages: (...args: unknown[]) => pickSourceImages(...args),
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
    generateImage.mockReset();
    editImage.mockReset();
    pickSourceImages.mockReset();

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
    generateImage.mockResolvedValue({
      generation_id: "generation-new",
      conversation_id: "conversation-1",
      images: [],
    });
    editImage.mockResolvedValue({
      generation_id: "generation-edit",
      conversation_id: "conversation-1",
      images: [],
    });
    pickSourceImages.mockResolvedValue([]);
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

  it("submits selected generation parameters from the settings bar", async () => {
    render(<GeneratePage />);

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.change(
      screen.getByPlaceholderText("Describe the image you want to generate..."),
      { target: { value: "A luminous glass observatory" } },
    );
    fireEvent.change(screen.getByLabelText("Background"), {
      target: { value: "transparent" },
    });
    fireEvent.change(screen.getByLabelText("Moderation"), {
      target: { value: "low" },
    });
    fireEvent.change(screen.getByLabelText("Format"), {
      target: { value: "webp" },
    });
    expect(screen.queryByLabelText("Compression")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(generateImage).toHaveBeenCalledWith(
        expect.objectContaining({
          prompt: "A luminous glass observatory",
          background: "transparent",
          moderation: "low",
          outputFormat: "webp",
        }),
      );
    });
    expect(generateImage.mock.calls[0][0]).not.toHaveProperty("outputCompression");
  });

  it("prevents transparent backgrounds with jpeg output", async () => {
    render(<GeneratePage />);

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.change(screen.getByLabelText("Background"), {
      target: { value: "transparent" },
    });
    expect(screen.getByRole("option", { name: "JPEG" })).toBeDisabled();

    fireEvent.change(screen.getByLabelText("Background"), {
      target: { value: "auto" },
    });
    fireEvent.change(screen.getByLabelText("Format"), {
      target: { value: "jpeg" },
    });
    expect(screen.getByRole("option", { name: "Transparent" })).toBeDisabled();
  });

  it("keeps generation parameters in a single row inside the page boundary", async () => {
    render(<GeneratePage />);

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    expect(
      screen.getByRole("toolbar", { name: "Generation parameters" }),
    ).toHaveClass("overflow-hidden");
    expect(
      screen.getByRole("toolbar", { name: "Generation parameters" }),
    ).not.toHaveClass("overflow-x-auto");
    expect(screen.getByTestId("generation-parameter-row")).toHaveClass(
      "grid",
      "min-w-0",
      "whitespace-nowrap",
    );
    expect(screen.getByTestId("generation-parameter-row")).not.toHaveClass(
      "w-max",
    );
  });

  it("submits edit-only input fidelity with selected source images", async () => {
    pickSourceImages.mockResolvedValue(["/tmp/source-edit.png"]);

    render(<GeneratePage />);

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "Upload Source" }));

    await waitFor(() => {
      expect(screen.getByText("Editing 1 source image")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByLabelText("Input fidelity"), {
      target: { value: "low" },
    });
    fireEvent.change(
      screen.getByPlaceholderText("Describe how you want to edit these source images..."),
      { target: { value: "Make the source image look like a lithograph" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(editImage).toHaveBeenCalledWith(
        expect.objectContaining({
          prompt: "Make the source image look like a lithograph",
          sourceImagePaths: ["/tmp/source-edit.png"],
          inputFidelity: "low",
        }),
      );
    });
  });
});
