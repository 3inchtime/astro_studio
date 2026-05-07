import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import GeneratePage from "./GeneratePage";

const getConversationGenerations = vi.fn();
const getImageModel = vi.fn();
const saveImageModel = vi.fn();
const generateImage = vi.fn();
const editImage = vi.fn();
const pickSourceImages = vi.fn();
const createPromptFavorite = vi.fn();
const getPromptFavorites = vi.fn();
const deletePromptFavorite = vi.fn();

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
        "generate.favoritePrompt": "Favorite prompt",
        "generate.removePromptFavorite": "Remove prompt favorite",
        "generate.promptFavorites": "Prompt favorites",
        "generate.promptFavoritesCount": `${options?.count} saved`,
        "generate.noPromptFavorites": "No prompt favorites yet",
        "generate.deletePromptFavorite": "Delete prompt favorite",
        "generate.closePromptFavorites": "Close prompt favorites",
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
  createPromptFavorite: (...args: unknown[]) => createPromptFavorite(...args),
  deleteGeneration: vi.fn(),
  deletePromptFavorite: (...args: unknown[]) => deletePromptFavorite(...args),
  editImage: (...args: unknown[]) => editImage(...args),
  generateImage: (...args: unknown[]) => generateImage(...args),
  getConversationGenerations: (...args: unknown[]) =>
    getConversationGenerations(...args),
  getImageModel: (...args: unknown[]) => getImageModel(...args),
  getPromptFavorites: (...args: unknown[]) => getPromptFavorites(...args),
  saveImageModel: (...args: unknown[]) => saveImageModel(...args),
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
    activeProjectId: "project-1",
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
    onEditImage,
    onEditPrompt,
    onFavoritePrompt,
    isPromptFavorited,
  }: {
    message: {
      id: string;
      role: string;
      content: string;
      images?: Array<{
        path: string;
        imageId: string;
        generationId: string;
      }>;
    };
    onEditImage?: (image: {
      path: string;
      imageId: string;
      generationId: string;
    }) => void;
    onEditPrompt?: (message: { id: string; role: string; content: string }) => void;
    onFavoritePrompt?: (message: {
      id: string;
      role: string;
      content: string;
    }) => void;
    isPromptFavorited?: boolean;
  }) =>
    message.role === "user" ? (
      <div>
        <button onClick={() => onEditPrompt?.(message)}>Edit prompt</button>
        <button onClick={() => onFavoritePrompt?.(message)}>
          {isPromptFavorited ? "Remove prompt favorite" : "Favorite prompt"}
        </button>
      </div>
    ) : message.images?.[0] ? (
      <div>
        <span>{message.images[0].path}</span>
        <button onClick={() => onEditImage?.(message.images![0])}>Edit image</button>
      </div>
    ) : null,
}));

vi.mock("../components/lightbox/Lightbox", () => ({
  default: () => null,
}));

vi.mock("../components/favorites/FolderSelector", () => ({
  default: () => null,
}));

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });

  return { promise, resolve, reject };
}

describe("GeneratePage", () => {
  beforeEach(() => {
    getConversationGenerations.mockReset();
    getImageModel.mockReset();
    saveImageModel.mockReset();
    generateImage.mockReset();
    editImage.mockReset();
    pickSourceImages.mockReset();
    createPromptFavorite.mockReset();
    getPromptFavorites.mockReset();
    deletePromptFavorite.mockReset();

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
    getPromptFavorites.mockResolvedValue([]);
    createPromptFavorite.mockImplementation(async (prompt: string) => ({
      id: "favorite-1",
      prompt,
      created_at: "2026-04-28T00:00:00Z",
      updated_at: "2026-04-28T00:00:00Z",
    }));
    deletePromptFavorite.mockResolvedValue(undefined);
    saveImageModel.mockResolvedValue(undefined);
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

  it("renders every registered model from the shared catalog in the model selector", async () => {
    render(<GeneratePage />);

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    const { IMAGE_MODEL_CATALOG } = await import("../lib/modelCatalog");
    const modelSelect = await screen.findByLabelText("Model");
    const optionNames = within(modelSelect)
      .getAllByRole("option")
      .map((option) => option.textContent);

    expect(optionNames).toEqual(
      IMAGE_MODEL_CATALOG.map((entry) => entry.label),
    );
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

  it("saves a sent prompt as a prompt favorite from the message actions", async () => {
    render(<GeneratePage />);

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "Favorite prompt" }));

    await waitFor(() => {
      expect(createPromptFavorite).toHaveBeenCalledWith(
        "A dramatic mountain temple at sunrise",
      );
    });
    expect(
      screen.getByRole("button", { name: "Remove prompt favorite" }),
    ).toBeInTheDocument();
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

  it("uses the selected UI model for the next generate request even while model persistence is still in flight", async () => {
    const saveModelDeferred = createDeferred<void>();
    saveImageModel.mockReturnValue(saveModelDeferred.promise);

    render(<GeneratePage />);

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.change(screen.getByLabelText("Model"), {
      target: { value: "nano-banana" },
    });
    fireEvent.change(
      screen.getByPlaceholderText("Describe the image you want to generate..."),
      { target: { value: "A neon paper crane" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(generateImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "nano-banana",
          prompt: "A neon paper crane",
        }),
      );
    });
  });

  it("keeps the returned generated image visible when conversation reload data is stale", async () => {
    generateImage.mockResolvedValue({
      generation_id: "generation-new",
      conversation_id: "conversation-1",
      images: [
        {
          id: "image-new",
          generation_id: "generation-new",
          file_path: "/tmp/generated-nano-banana.png",
          thumbnail_path: "/tmp/generated-nano-banana-thumb.png",
        },
      ],
    });

    render(<GeneratePage />);

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.change(screen.getByLabelText("Model"), {
      target: { value: "nano-banana" },
    });
    fireEvent.change(
      screen.getByPlaceholderText("Describe the image you want to generate..."),
      { target: { value: "A luminous banana nebula" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(generateImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "nano-banana",
          prompt: "A luminous banana nebula",
        }),
      );
    });
    await waitFor(() => {
      expect(screen.getByText("/tmp/generated-nano-banana.png")).toBeInTheDocument();
    });
  });

  it("resets parameters to selected model defaults and hides unsupported controls when switching models", async () => {
    pickSourceImages.mockResolvedValue(["/tmp/source-edit.png"]);
    const { getImageModelCatalogEntry } = await import("../lib/modelCatalog");
    const geminiEntry = getImageModelCatalogEntry("nano-banana");

    render(<GeneratePage />);

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.change(screen.getByLabelText("Size"), {
      target: { value: "1536x1024" },
    });
    fireEvent.change(screen.getByLabelText("Quality"), {
      target: { value: "high" },
    });
    fireEvent.change(screen.getByLabelText("Background"), {
      target: { value: "transparent" },
    });
    fireEvent.change(screen.getByLabelText("Format"), {
      target: { value: "webp" },
    });
    fireEvent.change(screen.getByLabelText("Moderation"), {
      target: { value: "low" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Upload Source" }));

    await waitFor(() => {
      expect(screen.getByText("Editing 1 source image")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByLabelText("Input fidelity"), {
      target: { value: "low" },
    });
    fireEvent.change(screen.getByLabelText("Model"), {
      target: { value: "nano-banana" },
    });

    await waitFor(() => {
      expect(saveImageModel).toHaveBeenCalledWith("nano-banana");
    });

    expect(screen.getByLabelText("Size")).toHaveValue(
      geminiEntry.parameterDefaults.size,
    );
    expect(screen.getByLabelText("Count")).toHaveValue(
      String(geminiEntry.parameterDefaults.imageCount),
    );
    expect(screen.queryByLabelText("Quality")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Background")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Format")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Moderation")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Input fidelity")).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Upload Source" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Editing 1 source image")).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText("Describe how you want to edit these source images..."),
    ).toBeInTheDocument();

    fireEvent.change(
      screen.getByPlaceholderText("Describe how you want to edit these source images..."),
      { target: { value: "A polished chrome crane" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(editImage).toHaveBeenCalledWith(
        expect.objectContaining({
          prompt: "A polished chrome crane",
          model: "nano-banana",
          sourceImagePaths: ["/tmp/source-edit.png"],
          size: geminiEntry.parameterDefaults.size,
          imageCount: geminiEntry.parameterDefaults.imageCount,
        }),
      );
    });
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("quality");
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("background");
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("outputFormat");
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("moderation");
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("inputFidelity");
    expect(generateImage).not.toHaveBeenCalled();
  });

  it("reconciles invalid draft state when late hydration switches to a narrower Gemini model", async () => {
    const initialModelDeferred = createDeferred<"nano-banana">();
    getImageModel.mockReturnValue(initialModelDeferred.promise);
    pickSourceImages.mockResolvedValue(["/tmp/hydration-source.png"]);

    render(<GeneratePage />);

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.change(screen.getByLabelText("Size"), {
      target: { value: "1536x1024" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Upload Source" }));

    await waitFor(() => {
      expect(screen.getByText("Editing 1 source image")).toBeInTheDocument();
    });
    fireEvent.change(screen.getByLabelText("Quality"), {
      target: { value: "high" },
    });
    fireEvent.change(screen.getByLabelText("Background"), {
      target: { value: "transparent" },
    });
    fireEvent.change(
      screen.getByPlaceholderText(
        "Describe how you want to edit these source images...",
      ),
      { target: { value: "Turn it into etched silver" } },
    );

    initialModelDeferred.resolve("nano-banana");

    await waitFor(() => {
      expect(getImageModel).toHaveBeenCalled();
    });

    await waitFor(() => {
      expect(screen.getByLabelText("Model")).toHaveValue(
        "nano-banana",
      );
    });
    expect(
      screen.getByRole("button", { name: "Upload Source" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Editing 1 source image")).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText(
        "Describe how you want to edit these source images...",
      ),
    ).toBeInTheDocument();
    expect(screen.getByDisplayValue("Turn it into etched silver")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(editImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "nano-banana",
          prompt: "Turn it into etched silver",
          size: "1536x1024",
          imageCount: 1,
          sourceImagePaths: ["/tmp/hydration-source.png"],
        }),
      );
    });
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("quality");
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("background");
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("outputFormat");
    expect(editImage.mock.calls[0][0]).not.toHaveProperty("moderation");
    expect(generateImage).not.toHaveBeenCalled();
  });

  it("keeps picked edit sources when source picking resolves after hydration switches to Gemini", async () => {
    const initialModelDeferred = createDeferred<"nano-banana">();
    const pickedSourcesDeferred = createDeferred<string[]>();
    getImageModel.mockReturnValue(initialModelDeferred.promise);
    pickSourceImages.mockReturnValue(pickedSourcesDeferred.promise);

    render(<GeneratePage />);

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.click(screen.getByRole("button", { name: "Upload Source" }));

    initialModelDeferred.resolve("nano-banana");
    pickedSourcesDeferred.resolve(["/tmp/picked-after-hydration.png"]);

    await waitFor(() => {
      expect(screen.getByLabelText("Model")).toHaveValue(
        "nano-banana",
      );
    });

    await waitFor(() => {
      expect(pickSourceImages).toHaveBeenCalled();
    });

    expect(screen.getByText("Editing 1 source image")).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText("Describe how you want to edit these source images..."),
    ).toBeInTheDocument();

    fireEvent.change(
      screen.getByPlaceholderText("Describe how you want to edit these source images..."),
      { target: { value: "A brushed steel heron" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(editImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "nano-banana",
          prompt: "A brushed steel heron",
          sourceImagePaths: ["/tmp/picked-after-hydration.png"],
        }),
      );
    });
    expect(generateImage).not.toHaveBeenCalled();
  });

  it("uses image-to-edit entry points under a Gemini model", async () => {
    render(<GeneratePage />);

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    fireEvent.change(screen.getByLabelText("Model"), {
      target: { value: "nano-banana" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Edit image" }));

    expect(screen.getByText("Editing 1 source image")).toBeInTheDocument();
    expect(
      screen.getByPlaceholderText("Describe how you want to edit these source images..."),
    ).toBeInTheDocument();

    fireEvent.change(
      screen.getByPlaceholderText("Describe how you want to edit these source images..."),
      { target: { value: "A paper lantern koi" } },
    );
    fireEvent.click(screen.getByRole("button", { name: "Generate" }));

    await waitFor(() => {
      expect(editImage).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "nano-banana",
          prompt: "A paper lantern koi",
          sourceImagePaths: ["/tmp/source.png"],
        }),
      );
    });
    expect(generateImage).not.toHaveBeenCalled();
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

  it("lets the generate surface stretch across the available main panel", async () => {
    render(<GeneratePage />);

    await waitFor(() => {
      expect(getConversationGenerations).toHaveBeenCalledWith("conversation-1");
    });

    const toolbar = screen.getByRole("toolbar", {
      name: "Generation parameters",
    });
    const feedSurface = screen
      .getByRole("button", { name: "Edit prompt" })
      .closest(".space-y-7");
    const composerSurface = screen
      .getByPlaceholderText("Describe the image you want to generate...")
      .parentElement?.parentElement;

    expect(toolbar).toHaveClass("w-full");
    expect(toolbar.className).not.toContain("max-w-[900px]");
    expect(feedSurface).toHaveClass("w-full");
    expect(feedSurface?.className).not.toContain("max-w-[900px]");
    expect(composerSurface).toHaveClass("w-full");
    expect(composerSurface?.className).not.toContain("max-w-[900px]");
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
