import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import PromptExtractPage from "./PromptExtractPage";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
});

function TestWrapper({ children }: { children: React.ReactNode }) {
  return (
    <QueryClientProvider client={queryClient}>
      {children}
    </QueryClientProvider>
  );
}

const pickSourceImages = vi.fn();
const createPromptFavorite = vi.fn();
const deletePromptFavorite = vi.fn();
const getPromptFavorites = vi.fn();
const getPromptExtractions = vi.fn();
const getLlmConfigs = vi.fn();
const extractPromptFromImage = vi.fn();
const createConversation = vi.fn();
const navigate = vi.fn();
const setActiveConversationId = vi.fn();
const writeText = vi.fn();

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    i18n: {
      language: "zh-CN",
      resolvedLanguage: "zh-CN",
    },
    t: (key: string) =>
      ({
        "extract.title": "Image Prompt Extract",
        "extract.subtitle": "Upload one image and extract a prompt.",
        "extract.uploadTitle": "Upload image",
        "extract.uploadHint": "Choose one local image",
        "extract.selectImage": "Select image",
        "extract.changeImage": "Change image",
        "extract.extractPrompt": "Extract prompt",
        "extract.resultTitle": "Prompt",
        "extract.copy": "Copy",
        "extract.favorite": "Favorite",
        "extract.unfavorite": "Remove favorite",
        "extract.usePrompt": "Use prompt",
        "extract.historyTitle": "History",
        "extract.historyHint": "Recent extractions",
        "extract.historyEmpty": "No extraction history yet",
        "extract.noImageSelected": "Select an image first",
        "extract.noMultimodalConfig": "Enable a multimodal LLM in settings first",
        "extract.copied": "Copied",
        "extract.extracting": "Extracting...",
      })[key] ?? key,
  }),
}));

vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>("react-router-dom");
  return {
    ...actual,
    useNavigate: () => navigate,
  };
});

vi.mock("../lib/api", () => ({
  pickSourceImages: (...args: unknown[]) => pickSourceImages(...args),
  createPromptFavorite: (...args: unknown[]) => createPromptFavorite(...args),
  deletePromptFavorite: (...args: unknown[]) => deletePromptFavorite(...args),
  getPromptFavorites: (...args: unknown[]) => getPromptFavorites(...args),
  getPromptExtractions: (...args: unknown[]) => getPromptExtractions(...args),
  getLlmConfigs: (...args: unknown[]) => getLlmConfigs(...args),
  extractPromptFromImage: (...args: unknown[]) => extractPromptFromImage(...args),
  createConversation: (...args: unknown[]) => createConversation(...args),
  toAssetUrl: (path: string) => path,
}));

vi.mock("../components/layout/AppLayout", () => ({
  useLayoutContext: () => ({
    setActiveConversationId,
  }),
}));

describe("PromptExtractPage", () => {
  beforeEach(() => {
    queryClient.clear();
    pickSourceImages.mockReset();
    createPromptFavorite.mockReset();
    deletePromptFavorite.mockReset();
    getPromptFavorites.mockReset();
    getPromptExtractions.mockReset();
    getLlmConfigs.mockReset();
    extractPromptFromImage.mockReset();
    createConversation.mockReset();
    navigate.mockReset();
    setActiveConversationId.mockReset();
    writeText.mockReset();

    Object.assign(navigator, {
      clipboard: {
        writeText,
      },
    });

    getPromptFavorites.mockResolvedValue([]);
    getPromptExtractions.mockResolvedValue([
      {
        id: "extract-history-1",
        image_path: "/tmp/history-1.png",
        prompt: "soft daylight portrait",
        llm_config_id: "vision-1",
        created_at: "2026-05-09T00:00:00Z",
        updated_at: "2026-05-09T00:00:00Z",
      },
      {
        id: "extract-history-2",
        image_path: "/tmp/history-2.png",
        prompt: "editorial street fashion",
        llm_config_id: "vision-1",
        created_at: "2026-05-08T00:00:00Z",
        updated_at: "2026-05-08T00:00:00Z",
      },
    ]);
    getLlmConfigs.mockResolvedValue([
      {
        id: "vision-1",
        name: "Vision",
        protocol: "openai",
        model: "gpt-4.1",
        api_key: "key",
        base_url: "https://api.openai.com/v1",
        capability: "multimodal",
        enabled: true,
      },
    ]);
    extractPromptFromImage.mockResolvedValue({
      id: "extract-1",
      image_path: "/tmp/reference.png",
      prompt: "cinematic portrait",
      llm_config_id: "vision-1",
      created_at: "2026-05-09T00:00:00Z",
      updated_at: "2026-05-09T00:00:00Z",
    });
    createPromptFavorite.mockResolvedValue({
      id: "favorite-1",
      prompt: "cinematic portrait",
      created_at: "2026-05-09T00:00:00Z",
      updated_at: "2026-05-09T00:00:00Z",
    });
    deletePromptFavorite.mockResolvedValue(undefined);
    createConversation.mockResolvedValue({
      id: "conversation-new-1",
      title: "",
      project_id: "default",
      created_at: "2026-05-09T00:00:00Z",
      updated_at: "2026-05-09T00:00:00Z",
      archived_at: null,
      pinned_at: null,
      deleted_at: null,
      generation_count: 0,
      latest_generation_at: null,
      latest_thumbnail: null,
    });
    writeText.mockResolvedValue(undefined);
  });

  it("picks the first image and extracts a prompt into the textarea", async () => {
    pickSourceImages.mockResolvedValue(["/tmp/reference.png", "/tmp/other.png"]);

    render(<PromptExtractPage />, { wrapper: TestWrapper });

    fireEvent.click(screen.getByRole("button", { name: "Select image" }));

    await waitFor(() => {
      expect(pickSourceImages).toHaveBeenCalled();
    });

    fireEvent.click(screen.getByRole("button", { name: "Extract prompt" }));

    await waitFor(() => {
      expect(extractPromptFromImage).toHaveBeenCalledWith("/tmp/reference.png", "vision-1", "zh-CN");
    });
    expect(screen.getByDisplayValue("cinematic portrait")).toBeInTheDocument();
  });

  it("copies, favorites, and creates a new conversation before sending the extracted prompt to generate", async () => {
    pickSourceImages.mockResolvedValue(["/tmp/reference.png"]);

    render(<PromptExtractPage />, { wrapper: TestWrapper });

    fireEvent.click(screen.getByRole("button", { name: "Select image" }));
    await waitFor(() => {
      expect(pickSourceImages).toHaveBeenCalled();
    });
    fireEvent.click(screen.getByRole("button", { name: "Extract prompt" }));

    await waitFor(() => {
      expect(screen.getByDisplayValue("cinematic portrait")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    await waitFor(() => {
      expect(writeText).toHaveBeenCalledWith("cinematic portrait");
    });

    fireEvent.click(screen.getByRole("button", { name: "Favorite" }));
    await waitFor(() => {
      expect(createPromptFavorite).toHaveBeenCalledWith("cinematic portrait");
    });

    fireEvent.click(screen.getByRole("button", { name: "Use prompt" }));

    await waitFor(() => {
      expect(createConversation).toHaveBeenCalled();
    });
    expect(setActiveConversationId).not.toHaveBeenCalled();
    expect(navigate).toHaveBeenCalledWith("/generate", {
      state: {
        pendingPrompt: "cinematic portrait",
        activateConversationId: "conversation-new-1",
      },
    });
  });

  it("renders extraction history in its own card and restores a selected record into the editor", async () => {
    const { container } = render(<PromptExtractPage />, { wrapper: TestWrapper });

    expect(await screen.findByText("History")).toBeInTheDocument();
    expect(await screen.findByText("soft daylight portrait")).toBeInTheDocument();
    expect(await screen.findByText("editorial street fashion")).toBeInTheDocument();

    const promptSection = screen.getByText("Prompt").closest("section");
    const historySection = screen.getByText("History").closest("section");

    expect(promptSection).not.toBeNull();
    expect(historySection).not.toBeNull();
    expect(historySection).not.toBe(promptSection);
    expect(container.querySelectorAll("section")).toHaveLength(3);

    fireEvent.click(screen.getByRole("button", { name: /editorial street fashion/i }));

    expect(screen.getByDisplayValue("editorial street fashion")).toBeInTheDocument();
    expect(screen.getAllByText("/tmp/history-2.png")).toHaveLength(2);
  });
});
